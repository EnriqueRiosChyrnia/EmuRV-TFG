//! Comandos Tauri y sesión de emulación.
//!
//! La emulación vive SIEMPRE en su propio hilo (riesgo R4 de
//! ARQUITECTURA.md): los comandos le hablan por canales y los resultados
//! vuelven a la UI como eventos Tauri. Durante una ejecución larga, el
//! hilo corre la máquina en tramos acotados por count (~2 ms, TEC1: así
//! «Detener» responde incluso en bucles apretados sin syscalls, que el
//! timeout de Unicorn no corta) y emite `emu://registros` como máximo
//! cada 100 ms (~10 Hz, RNF-03); las vistas nunca se refrescan por
//! instrucción.

use crate::core::assembler::{self, AsmError, Program};
use crate::core::keyboard::KeyboardShared;
use crate::core::machine::{Machine, StopReason};
use crate::core::syscalls::{ConsoleEvent, InputMsg};
use crate::core::video::VideoShared;
use crate::core::{DATA_BASE, SP_INIT, TEXT_BASE};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

/// Tope de tiempo por tramo. Quedó como red de seguridad: el corte real es
/// por count (el timeout de Unicorn no corta bucles apretados sin syscalls).
const RUN_SLICE_MS: u64 = 100;

/// Instrucciones por tramo de ejecución (TEC1). El spike
/// (tests/spike_count.rs, release) midió ~430 M instr/s con slicing y
/// ~4,2 µs de overhead por re-entrada: 1 M instr ≈ 2,3 ms por tramo, así
/// «Detener» se atiende con esa latencia incluso en bucles apretados.
const RUN_SLICE_COUNT: u64 = 1_000_000;

/// Cadencia mínima entre refrescos de registros a la UI (~10 Hz, RNF-03).
/// Va desacoplada del tramo: con tramos de ~2 ms, emitir por tramo
/// inundaría la UI a ~500 Hz.
const UI_REFRESH: Duration = Duration::from_millis(100);

// ---------------------------------------------------------------------------
// Tipos de la sesión
// ---------------------------------------------------------------------------

/// Órdenes que la UI envía al hilo de emulación.
enum EmuCmd {
    Run,
    Step,
    Reset,
    SetBreakpoints(HashSet<u32>),
    ReadMem {
        addr: u32,
        len: usize,
        reply: Sender<Result<Vec<u8>, String>>,
    },
    GetState {
        reply: Sender<Result<MachineState, String>>,
    },
}

/// Sesión activa: canales hacia el hilo de emulación.
struct Session {
    cmd_tx: Sender<EmuCmd>,
    input_tx: Sender<InputMsg>,
    stop_flag: Arc<AtomicBool>,
    /// La UI marca si el programa espera entrada (para Cancel en stop).
    waiting_input: Arc<AtomicBool>,
    /// Copia del programa ensamblado (para el export ELF, RF-08).
    program: Program,
    /// Estado de video compartido: `read_framebuffer` lo consume sin pasar
    /// por el hilo de emulación (que puede estar bloqueado en una syscall
    /// de entrada).
    video: Arc<Mutex<VideoShared>>,
    /// Cola de teclas del dispositivo MMIO (TEC2): `push_key` empuja acá
    /// sin pasar por el hilo de emulación (mismo criterio que `video`).
    keyboard: Arc<Mutex<KeyboardShared>>,
}

/// Estado global gestionado por Tauri.
#[derive(Default)]
pub struct EmuManager(Mutex<Option<Session>>);

// ---------------------------------------------------------------------------
// Payloads de eventos y respuestas (serializados a la UI)
// ---------------------------------------------------------------------------

/// Respuesta de `assemble`: lo que la UI necesita del programa ensamblado.
#[derive(Serialize, Clone)]
pub struct AssembleOk {
    pub text_base: u32,
    pub data_base: u32,
    pub sp_init: u32,
    pub text_len: u32,
    pub data_len: u32,
    pub symbols: BTreeMap<String, u32>,
    /// línea (1-based) → dirección de su primera instrucción
    pub line_to_addr: BTreeMap<u32, u32>,
    /// dirección → línea (para resaltar la línea actual)
    pub addr_to_line: HashMap<u32, u32>,
}

/// Estado de la máquina para las vistas (RF-05).
#[derive(Serialize, Clone)]
pub struct MachineState {
    pub registers: [u32; 32],
    pub pc: u32,
    pub line: Option<u32>,
}

/// Evento `emu://detenido`: la ejecución devolvió el control.
#[derive(Serialize, Clone)]
struct StoppedEvent {
    reason: StopReason,
    state: MachineState,
}

/// Evento `emu://consola`.
#[derive(Serialize, Clone)]
struct ConsoleOut {
    text: String,
}

// ---------------------------------------------------------------------------
// Hilo de emulación
// ---------------------------------------------------------------------------

/// Arma el estado para las vistas; propaga el error si los registros o el
/// pc no se pudieron leer (no se inventan valores en 0).
fn machine_state(m: &Machine) -> Result<MachineState, String> {
    let pc = m.pc()?;
    Ok(MachineState {
        registers: m.registers()?,
        pc,
        line: m.program().source_map.line_for_addr(pc),
    })
}

fn emit_stopped(app: &AppHandle, m: &Machine, reason: StopReason) {
    match machine_state(m) {
        Ok(state) => {
            let _ = app.emit("emu://detenido", StoppedEvent { reason, state });
        }
        Err(message) => {
            // No se pudo leer el estado real: el motivo pasa a ser ese
            // fallo explícito, con un estado neutro que la UI no debe
            // interpretar como valores reales.
            let _ = app.emit(
                "emu://detenido",
                StoppedEvent {
                    reason: StopReason::RuntimeError { message },
                    state: MachineState {
                        registers: [0u32; 32],
                        pc: 0,
                        line: None,
                    },
                },
            );
        }
    }
}

/// Bucle del hilo de emulación: atiende órdenes hasta que la sesión muere
/// (el canal se cierra cuando la UI ensambla de nuevo).
fn emu_thread(
    app: AppHandle,
    mut machine: Machine<'static>,
    cmd_rx: Receiver<EmuCmd>,
    stop_flag: Arc<AtomicBool>,
) {
    // Aviso único a la UI cuando el programa usa la pantalla gráfica (RF4):
    // el panel de video se muestra solo si hay uso real de la VRAM.
    let video = machine.video_shared();
    let mut video_notificado = false;
    let check_video = |notificado: &mut bool| {
        if !*notificado && video.lock().map(|v| v.in_use).unwrap_or(false) {
            *notificado = true;
            let _ = app.emit("emu://video-activo", ());
        }
    };
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            EmuCmd::Step => {
                let reason = machine.step();
                machine.publish_frame();
                check_video(&mut video_notificado);
                emit_stopped(&app, &machine, reason);
            }
            EmuCmd::Run => {
                stop_flag.store(false, Ordering::SeqCst);
                // El refresco de la UI va desacoplado del tramo: el tramo
                // es corto para la latencia de Detener; la cadencia de
                // eventos a la UI la marca este Instant.
                let mut ultimo_refresco = Instant::now();
                loop {
                    let reason = machine.run_slice(RUN_SLICE_MS, RUN_SLICE_COUNT);
                    // publish_frame queda por tramo a propósito: el flag
                    // sucio corta enseguida si la VRAM no cambió, y así el
                    // último frame nunca queda sin publicar al detenerse
                    // (un Instant extra no paga su complejidad acá).
                    machine.publish_frame();
                    check_video(&mut video_notificado);
                    match reason {
                        StopReason::Paused => {
                            // Count/timeout agotado: NO es pausa de usuario.
                            // Solo se reporta a la UI si Detener fue pedido.
                            if stop_flag.load(Ordering::SeqCst) {
                                emit_stopped(&app, &machine, StopReason::Paused);
                                break;
                            }
                            // Refresco periódico (RNF-03): como máximo cada
                            // UI_REFRESH aunque haya ~500 tramos/s.
                            if ultimo_refresco.elapsed() >= UI_REFRESH {
                                match machine_state(&machine) {
                                    Ok(st) => {
                                        let _ = app.emit("emu://registros", st);
                                        ultimo_refresco = Instant::now();
                                    }
                                    Err(message) => {
                                        // No se pudieron leer los registros:
                                        // cortar la corrida reportando el
                                        // fallo.
                                        emit_stopped(
                                            &app,
                                            &machine,
                                            StopReason::RuntimeError { message },
                                        );
                                        break;
                                    }
                                }
                            }
                            // Las órdenes auxiliares y el stop_flag se
                            // atienden en CADA tramo: esa es la gracia del
                            // slicing (latencia de Detener ~2 ms).
                            match cmd_rx.try_recv() {
                                Ok(cmd) => {
                                    if let Err(e) = handle_aux(&mut machine, cmd) {
                                        emit_stopped(
                                            &app,
                                            &machine,
                                            StopReason::RuntimeError { message: e },
                                        );
                                        break;
                                    }
                                }
                                Err(TryRecvError::Empty) => {}
                                Err(TryRecvError::Disconnected) => return,
                            }
                        }
                        other => {
                            emit_stopped(&app, &machine, other);
                            break;
                        }
                    }
                }
            }
            EmuCmd::Reset => match machine.reset() {
                Ok(()) => emit_stopped(&app, &machine, StopReason::Paused),
                Err(e) => emit_stopped(&app, &machine, StopReason::RuntimeError { message: e }),
            },
            aux => {
                if let Err(e) = handle_aux(&mut machine, aux) {
                    emit_stopped(&app, &machine, StopReason::RuntimeError { message: e });
                }
            }
        }
    }
}

/// Órdenes que se pueden atender en cualquier momento, incluso entre
/// tramos de una ejecución larga. Run/Step/Reset llegados a destiempo se
/// descartan (la UI deshabilita esos botones mientras corre).
fn handle_aux(machine: &mut Machine<'static>, cmd: EmuCmd) -> Result<(), String> {
    match cmd {
        EmuCmd::SetBreakpoints(bps) => machine.set_breakpoints(bps),
        EmuCmd::ReadMem { addr, len, reply } => {
            let _ = reply.send(machine.read_mem(addr, len));
            Ok(())
        }
        EmuCmd::GetState { reply } => {
            // El Result viaja entero: el solicitante decide cómo reportarlo.
            let _ = reply.send(machine_state(machine));
            Ok(())
        }
        EmuCmd::Run | EmuCmd::Step | EmuCmd::Reset => Ok(()),
    }
}

/// Puente consola→UI: reenvía la salida del programa como eventos Tauri.
/// Hilo aparte para no acoplar la emulación al event loop de la ventana.
/// También avisa del uso de video al pedir entrada: si el programa dibujó
/// y se bloqueó esperando una línea, el hilo de emulación no va a poder
/// emitir el aviso hasta desbloquearse (FB2).
fn console_thread(
    app: AppHandle,
    out_rx: Receiver<ConsoleEvent>,
    waiting: Arc<AtomicBool>,
    video: Arc<Mutex<VideoShared>>,
) {
    let mut video_notificado = false;
    while let Ok(ev) = out_rx.recv() {
        match ev {
            ConsoleEvent::Output(text) => {
                let _ = app.emit("emu://consola", ConsoleOut { text });
            }
            ConsoleEvent::InputRequest => {
                if !video_notificado && video.lock().map(|v| v.in_use).unwrap_or(false) {
                    video_notificado = true;
                    let _ = app.emit("emu://video-activo", ());
                }
                waiting.store(true, Ordering::SeqCst);
                let _ = app.emit("emu://pedir-entrada", ());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Comandos Tauri
// ---------------------------------------------------------------------------

/// Ensambla el fuente. Si compila, levanta una sesión de emulación nueva
/// (la anterior muere al cerrarse sus canales). Errores → Err(Vec<AsmError>)
/// para que la UI los ancle al editor.
#[tauri::command]
pub fn assemble(
    source: String,
    app: AppHandle,
    manager: State<'_, EmuManager>,
) -> Result<AssembleOk, Vec<AsmError>> {
    let program: Program = assembler::assemble(&source)?;

    let ok = AssembleOk {
        text_base: TEXT_BASE,
        data_base: DATA_BASE,
        sp_init: SP_INIT,
        text_len: program.text.len() as u32,
        data_len: program.data.len() as u32,
        symbols: program.symbols.clone(),
        line_to_addr: program.source_map.line_to_addr.clone(),
        addr_to_line: program.source_map.addr_to_line.clone(),
    };

    let (out_tx, out_rx) = channel();
    let (in_tx, in_rx) = channel();
    let (cmd_tx, cmd_rx) = channel();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let waiting_input = Arc::new(AtomicBool::new(false));
    // El estado de video de la sesión nueva continúa el contador de la
    // anterior (y limpia la pantalla): los consumidores ya abiertos —panel
    // embebido y ventana desacoplada— siguen recibiendo frames sin
    // reiniciarse (ver VideoShared::para_sesion_nueva).
    let video_anterior = manager
        .0
        .lock()
        .map_err(|_| asm_internal_error("estado del emulador corrupto"))?
        .as_ref()
        .map(|s| Arc::clone(&s.video));
    let video = Arc::new(Mutex::new(VideoShared::para_sesion_nueva(
        video_anterior.as_deref(),
    )));
    let program_copy = program.clone();

    // La máquina se construye DENTRO del hilo: Unicorn no es Send (usa Rc).
    // El canal init confirma la creación (devolviendo la cola de teclado,
    // que la sesión necesita para `push_key`) o devuelve el error.
    let (init_tx, init_rx) = channel::<Result<Arc<Mutex<KeyboardShared>>, String>>();
    std::thread::spawn({
        let app = app.clone();
        let stop = Arc::clone(&stop_flag);
        let video = Arc::clone(&video);
        move || match Machine::new_with_video(program, out_tx, in_rx, video) {
            Ok(mut machine) => {
                // Espejo de la señal de stop de la sesión dentro de la
                // máquina (mismo Arc): la fase del sleep interrumpible la
                // consultará desde el hook de syscalls.
                machine.set_stop_request(Arc::clone(&stop));
                let _ = init_tx.send(Ok(machine.keyboard_shared()));
                emu_thread(app, machine, cmd_rx, stop);
            }
            Err(e) => {
                let _ = init_tx.send(Err(e));
            }
        }
    });
    let keyboard = init_rx
        .recv()
        .unwrap_or_else(|_| Err("el hilo de emulación murió al iniciar".into()))
        .map_err(|e| {
            vec![AsmError {
                line: 1,
                col: 1,
                msg: e,
            }]
        })?;

    std::thread::spawn({
        let waiting = Arc::clone(&waiting_input);
        let video = Arc::clone(&video);
        move || console_thread(app, out_rx, waiting, video)
    });

    // Reemplaza la sesión anterior: sus canales se cierran y sus hilos
    // terminan solos.
    *manager
        .0
        .lock()
        .map_err(|_| asm_internal_error("estado del emulador corrupto"))? = Some(Session {
        cmd_tx,
        input_tx: in_tx,
        stop_flag,
        waiting_input,
        program: program_copy,
        video,
        keyboard,
    });
    Ok(ok)
}

/// Error interno de `assemble` con la forma que la UI espera (Vec<AsmError>).
fn asm_internal_error(msg: &str) -> Vec<AsmError> {
    vec![AsmError {
        line: 1,
        col: 1,
        msg: msg.into(),
    }]
}

/// Exporta el programa ensamblado como ELF32 RISC-V (RF-08).
#[tauri::command]
pub fn export_elf(path: String, manager: State<'_, EmuManager>) -> Result<(), String> {
    let guard = manager
        .0
        .lock()
        .map_err(|_| String::from("estado del emulador corrupto"))?;
    let s = guard
        .as_ref()
        .ok_or("no hay programa ensamblado: usá «Ensamblar» primero")?;
    let bytes = crate::core::elf_export::export_elf(&s.program)?;
    std::fs::write(&path, bytes).map_err(|e| format!("no se pudo escribir «{path}»: {e}"))
}

/// Lee un archivo de texto del disco (botón «Abrir»: cargar un .asm del
/// usuario en el editor). No depende de la sesión de emulación.
#[tauri::command]
pub fn leer_archivo_texto(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("no se pudo leer el archivo: {e}"))
}

/// Guarda contenido de texto en disco (botón «Guardar»: persistir el .asm
/// del editor). No depende de la sesión de emulación.
#[tauri::command]
pub fn guardar_archivo_texto(path: String, contenido: String) -> Result<(), String> {
    std::fs::write(&path, contenido).map_err(|e| format!("no se pudo guardar el archivo: {e}"))
}

/// Helper: ejecuta `f` con la sesión activa o devuelve error en español.
fn with_session<T>(
    manager: &State<'_, EmuManager>,
    f: impl FnOnce(&Session) -> Result<T, String>,
) -> Result<T, String> {
    let guard = manager
        .0
        .lock()
        .map_err(|_| String::from("estado del emulador corrupto"))?;
    match guard.as_ref() {
        Some(s) => f(s),
        None => Err("no hay programa ensamblado: usá «Ensamblar» primero".into()),
    }
}

#[tauri::command]
pub fn run_program(manager: State<'_, EmuManager>) -> Result<(), String> {
    with_session(&manager, |s| {
        s.cmd_tx
            .send(EmuCmd::Run)
            .map_err(|_| "la sesión de emulación terminó".into())
    })
}

#[tauri::command]
pub fn step(manager: State<'_, EmuManager>) -> Result<(), String> {
    with_session(&manager, |s| {
        s.cmd_tx
            .send(EmuCmd::Step)
            .map_err(|_| "la sesión de emulación terminó".into())
    })
}

#[tauri::command]
pub fn reset(manager: State<'_, EmuManager>) -> Result<(), String> {
    with_session(&manager, |s| {
        // Si estaba esperando entrada, hay que despertarla primero.
        s.stop_flag.store(true, Ordering::SeqCst);
        if s.waiting_input.swap(false, Ordering::SeqCst) {
            let _ = s.input_tx.send(InputMsg::Cancel);
        }
        s.cmd_tx
            .send(EmuCmd::Reset)
            .map_err(|_| "la sesión de emulación terminó".into())
    })
}

/// Detiene una ejecución larga (o despierta al programa si espera entrada).
#[tauri::command]
pub fn stop(manager: State<'_, EmuManager>) -> Result<(), String> {
    with_session(&manager, |s| {
        s.stop_flag.store(true, Ordering::SeqCst);
        if s.waiting_input.swap(false, Ordering::SeqCst) {
            let _ = s.input_tx.send(InputMsg::Cancel);
        }
        Ok(())
    })
}

/// Breakpoints por dirección (la UI traduce línea→dirección con el
/// source_map que recibió de `assemble`).
#[tauri::command]
pub fn set_breakpoints(addrs: Vec<u32>, manager: State<'_, EmuManager>) -> Result<(), String> {
    with_session(&manager, |s| {
        s.cmd_tx
            .send(EmuCmd::SetBreakpoints(addrs.into_iter().collect()))
            .map_err(|_| "la sesión de emulación terminó".into())
    })
}

/// Tecla presionada en la pantalla gráfica (TEC2): empuja el código a la
/// cola del dispositivo de teclado MMIO. No pasa por el hilo de emulación
/// (puede estar corriendo un tramo): el hook MEM_READ la consume cuando el
/// programa lee RECEIVER_DATA. Cola llena = se descarta la tecla más vieja,
/// nunca es un error.
#[tauri::command]
pub fn push_key(code: u8, manager: State<'_, EmuManager>) -> Result<(), String> {
    with_session(&manager, |s| {
        // Lock envenenado: se recupera igual (la cola es POD, sin
        // invariantes a medias), mismo criterio que el estado de video.
        s.keyboard
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(code);
        Ok(())
    })
}

/// Línea tipeada en la consola para la syscall 5 (RF-07).
#[tauri::command]
pub fn send_input(text: String, manager: State<'_, EmuManager>) -> Result<(), String> {
    with_session(&manager, |s| {
        s.waiting_input.store(false, Ordering::SeqCst);
        s.input_tx
            .send(InputMsg::Line(text))
            .map_err(|_| "la sesión de emulación terminó".into())
    })
}

/// Hexdump para la vista de memoria (RF-06).
#[tauri::command]
pub fn read_mem(addr: u32, len: u32, manager: State<'_, EmuManager>) -> Result<Vec<u8>, String> {
    let len = len.min(4096) as usize; // tope de cordura para la UI
    let rx = with_session(&manager, |s| {
        let (reply, rx) = channel();
        s.cmd_tx
            .send(EmuCmd::ReadMem { addr, len, reply })
            .map_err(|_| String::from("la sesión de emulación terminó"))?;
        Ok(rx)
    })?;
    rx.recv()
        .map_err(|_| String::from("la sesión de emulación terminó"))?
}

/// Registros + pc + línea actual (para refrescar al pausar).
#[tauri::command]
pub fn get_state(manager: State<'_, EmuManager>) -> Result<MachineState, String> {
    let rx = with_session(&manager, |s| {
        let (reply, rx) = channel();
        s.cmd_tx
            .send(EmuCmd::GetState { reply })
            .map_err(|_| String::from("la sesión de emulación terminó"))?;
        Ok(rx)
    })?;
    rx.recv()
        .map_err(|_| String::from("la sesión de emulación terminó"))?
}

/// Lee el último frame publicado si es más nuevo que `since` (el número de
/// publicación que el consumidor ya pintó). Responde BYTES CRUDOS
/// (`tauri::ipc::Response`, nunca JSON — RF1): 8 bytes de seq little-endian
/// + 307200 bytes RGBA, o 0 bytes si no hay nada más nuevo (flag sucio,
/// RF2). El frame se conserva: el panel embebido y la ventana aparte pueden
/// consumir en paralelo, cada uno con su propio `since`. No pasa por el
/// hilo de emulación: lee el estado compartido, así el tablero se ve
/// incluso con el programa bloqueado pidiendo entrada.
#[tauri::command]
pub fn read_framebuffer(
    since: String,
    manager: State<'_, EmuManager>,
) -> Result<tauri::ipc::Response, String> {
    // `since` llega como string porque JSON (números f64) no representa el
    // rango completo de u64: el front manda el bigint serializado.
    let since: u64 = since
        .parse()
        .map_err(|_| String::from("parámetro since inválido"))?;
    let respuesta = with_session(&manager, |s| {
        let v = s
            .video
            .lock()
            .map_err(|_| String::from("estado de video corrupto"))?;
        if v.seq <= since {
            return Ok(Vec::new());
        }
        match &v.rgba {
            Some(frame) => {
                let mut out = Vec::with_capacity(8 + frame.len());
                out.extend_from_slice(&v.seq.to_le_bytes());
                out.extend_from_slice(frame);
                Ok(out)
            }
            None => Ok(Vec::new()),
        }
    })?;
    Ok(tauri::ipc::Response::new(respuesta))
}
