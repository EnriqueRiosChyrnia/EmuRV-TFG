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
use crate::core::cond::parse_cond;
use crate::core::keyboard::KeyboardShared;
use crate::core::machine::{BreakpointSpec, Machine, StopReason, Watchpoint};
use crate::core::sound::{SoundEvent, SoundShared};
use crate::core::syscalls::{ConsoleEvent, InputMsg};
use crate::core::video::VideoShared;
use crate::core::{Xlen, DATA_BASE, SP_INIT, TEXT_BASE};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
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
    SetBreakpoints(Vec<BreakpointSpec>),
    SetWatchpoints {
        watchpoints: Vec<Watchpoint>,
        reply: Sender<Result<(), String>>,
    },
    WriteReg {
        n: u8,
        val: u32,
        reply: Sender<Result<(), String>>,
    },
    WritePc {
        val: u32,
        reply: Sender<Result<(), String>>,
    },
    WriteMem {
        addr: u32,
        bytes: Vec<u8>,
        reply: Sender<Result<(), String>>,
    },
    GetCsrs {
        reply: Sender<[u32; 8]>,
    },
    WriteCsr {
        indice: usize,
        val: u32,
        reply: Sender<Result<(), String>>,
    },
    GetFregs {
        reply: Sender<(Result<[u64; 32], String>, u32)>,
    },
    WriteRegF {
        n: usize,
        bits: u64,
        reply: Sender<Result<(), String>>,
    },
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
    /// El hilo de emulación está dentro del bucle de Ejecutar (RP7): las
    /// ediciones en vivo se rechazan mientras esté en true.
    corriendo: Arc<AtomicBool>,
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
    /// Valores de registro como hex de 16 dígitos (u64), igual que `get_fregs`
    /// (RP12): un `u64` > 2^53 perdería precisión como `number` de JS, así que
    /// los 64 bits cruzan como string. La UI muestra 16 dígitos en RV64 y los
    /// 8 bajos en RV32, según `xlen`.
    pub registers: Vec<String>,
    pub pc: u32,
    pub line: Option<u32>,
    /// Ancho de la máquina en ejecución (RP32): la UI formatea los registros
    /// según esto (16 dígitos hex en RV64, 8 en RV32).
    pub xlen: Xlen,
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
        registers: m.registers()?.iter().map(|v| format!("{v:016x}")).collect(),
        pc,
        line: m.program().source_map.line_for_addr(pc),
        xlen: m.program().xlen,
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
                        registers: vec![String::from("0000000000000000"); 32],
                        pc: 0,
                        line: None,
                        xlen: Xlen::Rv32,
                    },
                },
            );
        }
    }
}

/// Drena la cola de tonos pendientes y emite un `emu://sonido` por cada uno
/// (RP31). A diferencia del video (que PUBLICA un frame conservado), el sonido
/// es por EVENTO: cada tono se consume y se emite una sola vez. Se drena la
/// cola a un Vec antes de emitir para no retener el lock durante el emit.
fn drenar_sonido(app: &AppHandle, sound: &Arc<Mutex<SoundShared>>) {
    let eventos: Vec<SoundEvent> = match sound.lock() {
        Ok(mut s) => s.cola.drain(..).collect(),
        // Lock envenenado: recuperarlo (la cola es POD), no perder los tonos.
        Err(e) => e.into_inner().cola.drain(..).collect(),
    };
    for ev in eventos {
        let _ = app.emit("emu://sonido", ev);
    }
}

/// Bucle del hilo de emulación: atiende órdenes hasta que la sesión muere
/// (el canal se cierra cuando la UI ensambla de nuevo).
fn emu_thread(
    app: AppHandle,
    mut machine: Machine<'static>,
    cmd_rx: Receiver<EmuCmd>,
    stop_flag: Arc<AtomicBool>,
    corriendo: Arc<AtomicBool>,
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
    // Cola de sonido de la sesión (RP31): se drena tras cada tramo y paso,
    // igual que `check_video`, para emitir los tonos disparados (emu://sonido).
    let sound = machine.sound_shared();
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            EmuCmd::Step => {
                // Espejo del brazo Run: reset() y stop() dejan stop_flag en
                // true y nadie más lo limpiaba. Como stop_request es el
                // MISMO Arc que consulta dormir(), la secuencia
                // «Reiniciar → Paso sobre un ecall a7=32» saltearía el
                // sleep con el flag colgado de la orden anterior.
                stop_flag.store(false, Ordering::SeqCst);
                let reason = machine.step();
                machine.publish_frame();
                check_video(&mut video_notificado);
                drenar_sonido(&app, &sound);
                emit_stopped(&app, &machine, reason);
            }
            EmuCmd::Run => {
                stop_flag.store(false, Ordering::SeqCst);
                // Edición en vivo vetada mientras corre (RP7): la UI lo
                // consulta vía Session.corriendo.
                corriendo.store(true, Ordering::SeqCst);
                // El refresco de la UI va desacoplado del tramo: el tramo
                // es corto para la latencia de Detener; la cadencia de
                // eventos a la UI la marca este Instant.
                let mut ultimo_refresco = Instant::now();
                // Solo el PRIMER tramo suprime el breakpoint del pc de
                // arranque (semántica de reanudar desde un bp ya
                // reportado). Los siguientes son continuación: si un tramo
                // agotó su count justo sobre un bp, suprimirlo en el
                // próximo lo haría pasar de largo sin reportarse.
                let mut primer_tramo = true;
                loop {
                    let reason = if primer_tramo {
                        machine.run_slice(RUN_SLICE_MS, RUN_SLICE_COUNT)
                    } else {
                        machine.run_slice_continuation(RUN_SLICE_MS, RUN_SLICE_COUNT)
                    };
                    primer_tramo = false;
                    // publish_frame queda por tramo a propósito: el flag
                    // sucio corta enseguida si la VRAM no cambió, y así el
                    // último frame nunca queda sin publicar al detenerse
                    // (un Instant extra no paga su complejidad acá).
                    machine.publish_frame();
                    check_video(&mut video_notificado);
                    drenar_sonido(&app, &sound);
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
                corriendo.store(false, Ordering::SeqCst);
            }
            EmuCmd::Reset => {
                let resultado = machine.reset();
                // El comando reset() deja stop_flag en true para despertar
                // un posible sleep o espera de entrada de la corrida
                // anterior. Completado el reinicio ya cumplió su función:
                // se limpia acá (espejo de la línea del brazo Run) para
                // que un Paso posterior no herede un stop fantasma y
                // saltee los sleeps (mismo Arc que mira dormir()).
                stop_flag.store(false, Ordering::SeqCst);
                match resultado {
                    Ok(()) => emit_stopped(&app, &machine, StopReason::Paused),
                    Err(e) => emit_stopped(&app, &machine, StopReason::RuntimeError { message: e }),
                }
            }
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
        EmuCmd::SetWatchpoints { watchpoints, reply } => {
            let _ = reply.send(machine.set_watchpoints(watchpoints));
            Ok(())
        }
        EmuCmd::WriteReg { n, val, reply } => {
            let _ = reply.send(machine.write_reg(n, val));
            Ok(())
        }
        EmuCmd::WritePc { val, reply } => {
            let _ = reply.send(machine.write_pc(val));
            Ok(())
        }
        EmuCmd::WriteMem { addr, bytes, reply } => {
            let _ = reply.send(machine.write_mem(addr, &bytes));
            Ok(())
        }
        EmuCmd::GetCsrs { reply } => {
            let _ = reply.send(machine.read_csrs());
            Ok(())
        }
        EmuCmd::WriteCsr { indice, val, reply } => {
            let _ = reply.send(machine.write_csr(indice, val));
            Ok(())
        }
        EmuCmd::GetFregs { reply } => {
            let _ = reply.send((machine.registers_f(), machine.fcsr().unwrap_or(0)));
            Ok(())
        }
        EmuCmd::WriteRegF { n, bits, reply } => {
            let _ = reply.send(machine.write_reg_f(n, bits));
            Ok(())
        }
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
    sound: Arc<Mutex<SoundShared>>,
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
                // Si el programa disparó un tono y se bloqueó pidiendo entrada,
                // el hilo de emulación no va a drenarlo hasta desbloquearse:
                // se drena acá para que suene ya (espejo del aviso de video).
                drenar_sonido(&app, &sound);
                waiting.store(true, Ordering::SeqCst);
                let _ = app.emit("emu://pedir-entrada", ());
            }
            ConsoleEvent::Clear => {
                let _ = app.emit("emu://consola-limpiar", ());
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
    ruta: Option<String>,
    xlen: Option<Xlen>,
    app: AppHandle,
    manager: State<'_, EmuManager>,
) -> Result<AssembleOk, Vec<AsmError>> {
    // RP32: el ancho lo elige el usuario en la toolbar (RV32 por defecto si la
    // UI no lo manda). Habilita el repertorio de 64 bits y el Mode del motor.
    let xlen = xlen.unwrap_or_default();
    // RP5: si el buffer tiene archivo en disco, su carpeta es la base de
    // resolución de .include; sin ruta (buffer nunca guardado), .include
    // produce el error didáctico del preprocesador.
    let program: Program = match &ruta {
        Some(r) => {
            let path = std::path::Path::new(r);
            let nombre = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "programa.s".to_string());
            assembler::assemble_with_includes_xlen(&source, path.parent(), &nombre, xlen)?
        }
        None => assembler::assemble_with_xlen(&source, xlen)?,
    };
    crear_sesion(program, app, &manager).map_err(|e| asm_internal_error(&e))
}

/// Levanta la sesión de emulación para un `Program` ya construido
/// (ensamblado o importado de un ELF — RP11 extrajo este cuerpo de
/// `assemble` sin cambiarle una línea de semántica).
fn crear_sesion(
    program: Program,
    app: AppHandle,
    manager: &State<'_, EmuManager>,
) -> Result<AssembleOk, String> {
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
    let corriendo = Arc::new(AtomicBool::new(false));
    let corriendo_hilo = Arc::clone(&corriendo);
    let waiting_input = Arc::new(AtomicBool::new(false));
    // El estado de video de la sesión nueva continúa el contador de la
    // anterior (y limpia la pantalla): los consumidores ya abiertos —panel
    // embebido y ventana desacoplada— siguen recibiendo frames sin
    // reiniciarse (ver VideoShared::para_sesion_nueva).
    let video_anterior = manager
        .0
        .lock()
        .map_err(|_| String::from("estado del emulador corrupto"))?
        .as_ref()
        .map(|s| {
            // La sesión vieja muere cuando sus canales se cierran, pero eso
            // recién lo nota al volver a su recv: un hilo dormido en la
            // syscall 32 solo mira stop_request (quedaría retenido hasta
            // agotar a0 ms — 49,7 días con «li a0, -1» — y podría emitir
            // un emu://registros viejo al despertar) y uno bloqueado en
            // read_int solo mira su canal de entrada. Mismo gesto que
            // stop(): el flag corta el sleep y el Cancel despierta la
            // espera de entrada. Cierra la asimetría de reemplazar la
            // sesión sin avisarle.
            s.stop_flag.store(true, Ordering::SeqCst);
            let _ = s.input_tx.send(InputMsg::Cancel);
            Arc::clone(&s.video)
        });
    let video = Arc::new(Mutex::new(VideoShared::para_sesion_nueva(
        video_anterior.as_deref(),
    )));
    let program_copy = program.clone();

    // La máquina se construye DENTRO del hilo: Unicorn no es Send (usa Rc).
    // El canal init confirma la creación (devolviendo la cola de teclado,
    // que la sesión necesita para `push_key`) o devuelve el error.
    // El canal init confirma la creación devolviendo la cola de teclado y el
    // estado de sonido (ambos los crea la máquina y la sesión los necesita), o
    // el error.
    type InitDevices = (Arc<Mutex<KeyboardShared>>, Arc<Mutex<SoundShared>>);
    let (init_tx, init_rx) = channel::<Result<InitDevices, String>>();
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
                let _ = init_tx.send(Ok((machine.keyboard_shared(), machine.sound_shared())));
                emu_thread(app, machine, cmd_rx, stop, corriendo_hilo);
            }
            Err(e) => {
                let _ = init_tx.send(Err(e));
            }
        }
    });
    let (keyboard, sound) = init_rx
        .recv()
        .unwrap_or_else(|_| Err("el hilo de emulación murió al iniciar".into()))?;

    std::thread::spawn({
        let waiting = Arc::clone(&waiting_input);
        let video = Arc::clone(&video);
        let sound = Arc::clone(&sound);
        move || console_thread(app, out_rx, waiting, video, sound)
    });

    // Reemplaza la sesión anterior: sus canales se cierran y sus hilos
    // terminan solos.
    *manager
        .0
        .lock()
        .map_err(|_| String::from("estado del emulador corrupto"))? = Some(Session {
        cmd_tx,
        input_tx: in_tx,
        stop_flag,
        waiting_input,
        program: program_copy,
        video,
        keyboard,
        corriendo: Arc::clone(&corriendo),
    });
    Ok(ok)
}

/// Error interno de `assemble` con la forma que la UI espera (Vec<AsmError>).
fn asm_internal_error(msg: &str) -> Vec<AsmError> {
    vec![AsmError {
        line: 1,
        col: 1,
        msg: msg.into(),
        file: None,
    }]
}

/// Resultado del botón «Abrir» (RP11): un fuente para el editor o un
/// binario importado con su listado desensamblado (modo binario).
#[derive(Serialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum AperturaOk {
    Fuente { contenido: String },
    Binario { ok: AssembleOk, listado: String },
}

/// Abre un archivo del usuario con detección por MAGIC (la extensión no
/// importa): un ELF se importa y levanta sesión en modo binario (editor
/// solo-lectura con el listado; cada línea del listado = una dirección,
/// así los breakpoints/paso a paso reusan el mecanismo de siempre); un
/// archivo de texto vuelve como fuente para el editor.
#[tauri::command]
pub fn abrir_programa(
    path: String,
    app: AppHandle,
    manager: State<'_, EmuManager>,
) -> Result<AperturaOk, String> {
    let bytes =
        std::fs::read(&path).map_err(|e| format!("no se pudo leer el archivo: {e}"))?;
    if crate::core::elf_import::es_elf(&bytes) {
        let mut program = crate::core::elf_import::import_elf(&bytes)?;
        let lineas = crate::core::disasm::disasm_bytes(&program.text, TEXT_BASE);
        let listado = crate::core::disasm::listado(&lineas);
        // El listado ES el «fuente» del modo binario: línea n ↔ dirección.
        // Con el source_map poblado así, el resaltado de ejecución, la
        // validación de breakpoints y get_state funcionan sin tocarse.
        for (i, l) in lineas.iter().enumerate() {
            let linea = (i + 1) as u32;
            program.source_map.addr_to_line.insert(l.addr, linea);
            program.source_map.line_to_addr.insert(linea, l.addr);
        }
        let ok = crear_sesion(program, app, &manager)?;
        return Ok(AperturaOk::Binario { ok, listado });
    }
    match String::from_utf8(bytes) {
        Ok(contenido) => Ok(AperturaOk::Fuente { contenido }),
        Err(_) => Err(
            "el archivo no es ni un ELF ni un fuente de texto (bytes no \
             UTF-8). ¿Seguro que es un programa?"
                .to_string(),
        ),
    }
}

/// Listado desensamblado del .text de la sesión (RP11): la columna de
/// instrucción del panel de memoria. Mapa dirección → texto decodificado.
#[tauri::command]
pub fn get_disasm(
    manager: State<'_, EmuManager>,
) -> Result<std::collections::BTreeMap<u32, String>, String> {
    let guard = manager
        .0
        .lock()
        .map_err(|_| String::from("estado del emulador corrupto"))?;
    let s = guard
        .as_ref()
        .ok_or("no hay programa ensamblado: usá «Ensamblar» primero")?;
    let lineas = crate::core::disasm::disasm_bytes(&s.program.text, TEXT_BASE);
    Ok(lineas.into_iter().map(|l| (l.addr, l.texto)).collect())
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
pub fn set_breakpoints(bps: Vec<BpEntrada>, manager: State<'_, EmuManager>) -> Result<(), String> {
    // El parseo de condiciones ocurre ACÁ (hilo de la UI): un error
    // didáctico llega al panel SIN tocar la máquina (RP7).
    let mut specs = Vec::with_capacity(bps.len());
    for b in bps {
        let cond = match b.cond.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(texto) => Some(
                parse_cond(texto)
                    .map_err(|e| format!("condición inválida en 0x{:08X}: {e}", b.addr))?,
            ),
        };
        specs.push(BreakpointSpec { addr: b.addr, cond });
    }
    with_session(&manager, |s| {
        s.cmd_tx
            .send(EmuCmd::SetBreakpoints(specs))
            .map_err(|_| "la sesión de emulación terminó".into())
    })
}

/// Breakpoint como llega de la UI: dirección + condición textual opcional.
#[derive(serde::Deserialize)]
pub struct BpEntrada {
    pub addr: u32,
    pub cond: Option<String>,
}

/// Espera la respuesta de un comando auxiliar servido por el hilo de
/// emulación (patrón reply-channel de read_mem/get_state).
fn pedir<T>(rx: Receiver<Result<T, String>>) -> Result<T, String> {
    rx.recv()
        .map_err(|_| "la sesión de emulación terminó".to_string())?
}

/// Guard de edición en vivo (RP7): solo con la máquina pausada.
fn exigir_pausa(s: &Session) -> Result<(), String> {
    if s.corriendo.load(Ordering::SeqCst) {
        return Err("pausá la ejecución para editar (Detener o breakpoint)".into());
    }
    Ok(())
}

/// Instala watchpoints (RP7): la validación de regiones vive en la máquina
/// y el error didáctico vuelve por el canal de respuesta.
#[tauri::command]
pub fn set_watchpoints(
    watchpoints: Vec<Watchpoint>,
    manager: State<'_, EmuManager>,
) -> Result<(), String> {
    with_session(&manager, |s| {
        let (reply, rx) = channel();
        s.cmd_tx
            .send(EmuCmd::SetWatchpoints { watchpoints, reply })
            .map_err(|_| "la sesión de emulación terminó".to_string())?;
        pedir(rx)
    })
}

/// Edición en vivo de un registro (RP7). `reg` acepta nombre ABI, xN o
/// «pc». Solo con la máquina pausada.
#[tauri::command]
pub fn write_reg(reg: String, val: u32, manager: State<'_, EmuManager>) -> Result<(), String> {
    with_session(&manager, |s| {
        exigir_pausa(s)?;
        let (reply, rx) = channel();
        let cmd = if reg.trim().eq_ignore_ascii_case("pc") {
            EmuCmd::WritePc { val, reply }
        } else {
            let n = assembler::reg_number(reg.trim()).ok_or_else(|| {
                format!("«{reg}» no es un registro; valen los nombres ABI, x0–x31 o pc")
            })?;
            EmuCmd::WriteReg { n, val, reply }
        };
        s.cmd_tx
            .send(cmd)
            .map_err(|_| "la sesión de emulación terminó".to_string())?;
        pedir(rx)
    })
}

/// Estado de los CSRs con nombre (RP8), para el panel avanzado.
#[tauri::command]
pub fn get_csrs(manager: State<'_, EmuManager>) -> Result<Vec<u32>, String> {
    with_session(&manager, |s| {
        let (reply, rx) = channel();
        s.cmd_tx
            .send(EmuCmd::GetCsrs { reply })
            .map_err(|_| "la sesión de emulación terminó".to_string())?;
        rx.recv()
            .map(|a| a.to_vec())
            .map_err(|_| "la sesión de emulación terminó".to_string())
    })
}

/// Edición de un CSR (RP8): índice del orden mstatus…mip. Solo pausado.
#[tauri::command]
pub fn write_csr(
    indice: usize,
    val: u32,
    manager: State<'_, EmuManager>,
) -> Result<(), String> {
    with_session(&manager, |s| {
        exigir_pausa(s)?;
        let (reply, rx) = channel();
        s.cmd_tx
            .send(EmuCmd::WriteCsr { indice, val, reply })
            .map_err(|_| "la sesión de emulación terminó".to_string())?;
        pedir(rx)
    })
}

/// Registros f del panel flotante (RP12): 32 valores de 64 bits como hex
/// (JSON no representa u64 sin perder precisión) + fcsr.
#[tauri::command]
pub fn get_fregs(manager: State<'_, EmuManager>) -> Result<(Vec<String>, u32), String> {
    with_session(&manager, |s| {
        let (reply, rx) = channel();
        s.cmd_tx
            .send(EmuCmd::GetFregs { reply })
            .map_err(|_| "la sesión de emulación terminó".to_string())?;
        let (regs, fcsr) = rx
            .recv()
            .map_err(|_| "la sesión de emulación terminó".to_string())?;
        let regs = regs?;
        Ok((regs.iter().map(|v| format!("{v:016x}")).collect(), fcsr))
    })
}

/// Edición de un registro f (RP12): bits crudos de 64 en hex. Solo pausado.
#[tauri::command]
pub fn write_reg_f(
    indice: usize,
    bits: String,
    manager: State<'_, EmuManager>,
) -> Result<(), String> {
    let bits = u64::from_str_radix(bits.trim_start_matches("0x"), 16)
        .map_err(|_| "bits inválidos: se esperaban 16 dígitos hexadecimales".to_string())?;
    with_session(&manager, |s| {
        exigir_pausa(s)?;
        let (reply, rx) = channel();
        s.cmd_tx
            .send(EmuCmd::WriteRegF {
                n: indice,
                bits,
                reply,
            })
            .map_err(|_| "la sesión de emulación terminó".to_string())?;
        pedir(rx)
    })
}

/// Edición en vivo de memoria (RP7): 1, 2 o 4 bytes little-endian. Solo
/// con la máquina pausada.
#[tauri::command]
pub fn write_mem(addr: u32, bytes: Vec<u8>, manager: State<'_, EmuManager>) -> Result<(), String> {
    with_session(&manager, |s| {
        exigir_pausa(s)?;
        let (reply, rx) = channel();
        s.cmd_tx
            .send(EmuCmd::WriteMem { addr, bytes, reply })
            .map_err(|_| "la sesión de emulación terminó".to_string())?;
        pedir(rx)
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
