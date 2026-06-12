//! Máquina virtual RV32 sobre Unicorn Engine.
//!
//! Modelo de ejecución (ARQUITECTURA.md §Diseños clave 3): la máquina se usa
//! desde un único hilo (en la GUI, el hilo de emulación dedicado). "Paso" es
//! `emu_start` con count=1; "Ejecutar" corre en tramos acotados por count
//! (`run_slice`, TEC1) para que el hilo pueda refrescar la UI, atender la
//! señal de stop y la entrada sin tocar Unicorn desde otro hilo. El count es
//! el único corte confiable: el timeout de Unicorn NO corta bucles apretados
//! sin syscalls (verificado en tests/spike_count.rs).
//!
//! El hook de código que compara el pc contra los breakpoints SOLO se
//! instala cuando hay breakpoints activos: con el hook puesto Unicorn no
//! puede usar su JIT a pleno y la velocidad cae en órdenes de magnitud.

use super::assembler::Program;
use super::keyboard::KeyboardShared;
use super::syscalls::{self, ConsoleEvent, InputMsg};
use super::video::{FramebufferRGBA, VideoShared};
use super::{
    DATA_BASE, DATA_REGION_BASE, DATA_REGION_SIZE, FRAMEBUFFER_BASE, FRAMEBUFFER_DATA_SIZE,
    FRAMEBUFFER_SIZE, GP_INIT, KEYBOARD_BASE, KEYBOARD_SIZE, PALETTE_BASE, PALETTE_DATA_SIZE,
    PALETTE_SIZE, RECEIVER_CONTROL, RECEIVER_DATA, SP_INIT, STACK_SIZE, STACK_TOP, TEXT_BASE,
    TEXT_SIZE, VIDEO_CONTROL_BASE, VIDEO_CONTROL_SIZE,
};
use serde::Serialize;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use unicorn_engine::Arch;
use unicorn_engine::{uc_error, HookType, MemType, Mode, Prot, RegisterRISCV, Unicorn};

/// `RegisterRISCV::X0 == 1`, por eso el +1.
fn xreg(n: u8) -> i32 {
    n as i32 + 1
}
const PC: RegisterRISCV = RegisterRISCV::PC;

/// Motivo por el que la ejecución devolvió el control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StopReason {
    /// El programa terminó vía syscall 10. Con solo syscall 10 (exit sin
    /// argumento, convención RARS) `code` es siempre 0; un código de salida
    /// variable llegaría con la syscall 93 (exit2 con código), trabajo
    /// futuro declarado.
    Exited { code: i32 },
    /// Se alcanzó un breakpoint (la instrucción en `addr` NO se ejecutó).
    Breakpoint { addr: u32 },
    /// Tramo de ejecución completado sin evento (timeout de refresco) o
    /// stop pedido por el usuario.
    Paused,
    /// Paso completado.
    StepDone,
    /// Error en tiempo de ejecución, con mensaje didáctico en español.
    RuntimeError { message: String },
}

/// Estado compartido con los hooks de Unicorn (vive en el `data` del motor).
pub struct MachData {
    pub out: Sender<ConsoleEvent>,
    pub input: Receiver<InputMsg>,
    pub exit_code: Option<i32>,
    pub hit_breakpoint: Option<u32>,
    pub runtime_error: Option<String>,
    pub breakpoints: HashSet<u32>,
    /// Breakpoint a ignorar una vez (el del pc actual al reanudar).
    pub suppress_bp: Option<u32>,
    /// Último acceso a memoria inválido capturado por el hook MEM_INVALID.
    pub bad_mem: Option<(MemType, u64)>,
    /// Estado de video compartido con la UI (FB2). En MachData (y no en
    /// Machine) porque el hook de syscalls publica el frame antes de
    /// bloquearse esperando entrada.
    pub video_shared: Arc<Mutex<VideoShared>>,
    /// Caché del último frame publicado (índices + paleta): flag sucio RF2.
    pub video_last: Option<Vec<u8>>,
    /// Ya se detectó uso de la pantalla (evita reescanear la VRAM, RF4).
    pub video_used: bool,
    /// Cola de teclas compartida con la UI (TEC2, espejo de `video_shared`):
    /// la UI empuja con `push_key` y el hook MEM_READ del teclado consume.
    pub keyboard: Arc<Mutex<KeyboardShared>>,
    /// Señal de stop de la sesión (mismo `Arc` que mira el hilo de
    /// emulación entre tramos). Por ahora solo se cablea: la fase siguiente
    /// la usará para hacer interrumpible la syscall de sleep.
    pub stop_request: Arc<AtomicBool>,
}

/// La máquina NO es `Send` (Unicorn usa `Rc` internamente): debe crearse y
/// usarse dentro del hilo de emulación. La señal de stop entre hilos vive
/// en la sesión; acá solo entra un espejo del mismo `Arc`
/// (`set_stop_request`) para que los hooks puedan consultarla.
pub struct Machine<'a> {
    uc: Unicorn<'a, MachData>,
    program: Program,
    bp_hook: Option<unicorn_engine::UcHookId>,
    /// Una corrida libre (count=0) dejó bloques traducidos en la caché JIT.
    /// Esos bloques NO respetan el count=1 de un paso posterior (Unicorn
    /// ejecuta el bloque cacheado entero): hay que flushear la caché antes
    /// de steppear. Era el bug "Reiniciar → Paso no avanza".
    tb_de_corrida: bool,
}

/// Dirección "hasta" que nunca se alcanza en RV32: ejecuta sin tope.
const NEVER: u64 = 0x1_0000_0000;

impl Machine<'_> {
    /// Crea la máquina, mapea el espacio de direcciones estilo RARS y carga
    /// el programa. `out`/`input` son los canales de consola (syscalls).
    pub fn new(
        program: Program,
        out: Sender<ConsoleEvent>,
        input: Receiver<InputMsg>,
    ) -> Result<Self, String> {
        Self::new_with_video(program, out, input, Arc::default())
    }

    /// Variante que comparte el estado de video con la UI (FB2): la GUI
    /// pasa un `Arc` que después lee desde el comando `read_framebuffer`.
    pub fn new_with_video(
        program: Program,
        out: Sender<ConsoleEvent>,
        input: Receiver<InputMsg>,
        video_shared: Arc<Mutex<VideoShared>>,
    ) -> Result<Self, String> {
        let data = MachData {
            out,
            input,
            exit_code: None,
            hit_breakpoint: None,
            runtime_error: None,
            breakpoints: HashSet::new(),
            suppress_bp: None,
            bad_mem: None,
            video_shared,
            video_last: None,
            video_used: false,
            keyboard: Arc::default(),
            stop_request: Arc::default(),
        };
        let mut uc = Unicorn::new_with_data(Arch::RISCV, Mode::RISCV32, data)
            .map_err(|e| format!("no se pudo iniciar el emulador: {e:?}"))?;

        uc.mem_map(TEXT_BASE as u64, TEXT_SIZE as u64, Prot::READ | Prot::EXEC)
            .map_err(|e| format!("no se pudo mapear .text: {e:?}"))?;
        uc.mem_map(
            DATA_REGION_BASE as u64,
            DATA_REGION_SIZE as u64,
            Prot::READ | Prot::WRITE,
        )
        .map_err(|e| format!("no se pudo mapear .data: {e:?}"))?;
        uc.mem_map(
            (STACK_TOP - STACK_SIZE) as u64,
            STACK_SIZE as u64,
            Prot::READ | Prot::WRITE,
        )
        .map_err(|e| format!("no se pudo mapear la pila: {e:?}"))?;

        uc.mem_map(
            FRAMEBUFFER_BASE as u64,
            FRAMEBUFFER_SIZE as u64,
            Prot::READ | Prot::WRITE,
        )
        .map_err(|e| format!("no se pudo mapear el framebuffer: {e:?}"))?;
        uc.mem_map(
            PALETTE_BASE as u64,
            PALETTE_SIZE as u64,
            Prot::READ | Prot::WRITE,
        )
        .map_err(|e| format!("no se pudo mapear la paleta: {e:?}"))?;
        uc.mem_map(
            VIDEO_CONTROL_BASE as u64,
            VIDEO_CONTROL_SIZE as u64,
            Prot::READ | Prot::WRITE,
        )
        .map_err(|e| format!("no se pudo mapear el registro de control: {e:?}"))?;

        // Página del teclado CON respaldo real de memoria (TEC2): las
        // lecturas de host (visor de memoria de la UI) leen los bytes sin
        // disparar hooks, así inspeccionar memoria NO consume teclas. La
        // alternativa mmio_map se descartó por eso (tests/spike_mmio.rs B6).
        uc.mem_map(
            KEYBOARD_BASE as u64,
            KEYBOARD_SIZE as u64,
            Prot::READ | Prot::WRITE,
        )
        .map_err(|e| format!("no se pudo mapear la página del teclado: {e:?}"))?;

        // Syscalls y excepciones: llegan como interrupción con la causa
        // RISC-V en intno (ver syscalls::handle_trap).
        uc.add_intr_hook(syscalls::handle_trap)
            .map_err(|e| format!("no se pudo instalar el hook de syscalls: {e:?}"))?;

        // Captura la dirección exacta de un acceso inválido para el mensaje
        // de error didáctico. Devolver false deja que el error se propague.
        uc.add_mem_hook(
            HookType::MEM_INVALID,
            0,
            NEVER,
            |uc, kind, addr, _sz, _val| {
                uc.get_data_mut().bad_mem = Some((kind, addr));
                false
            },
        )
        .map_err(|e| format!("no se pudo instalar el hook de memoria: {e:?}"))?;

        // Teclado MMIO (TEC2, mecanismo validado en tests/spike_mmio.rs):
        // hook MEM_READ permanente sobre CONTROL y DATA que escribe el
        // valor fresco en memoria ANTES de que la lectura del guest
        // complete. Funciona igual con lb y con lw (el spike midió ambos
        // anchos) y con la caché JIT caliente.
        uc.add_mem_hook(
            HookType::MEM_READ,
            RECEIVER_CONTROL as u64,
            u64::from(RECEIVER_DATA) + 3,
            |uc, _tipo, addr, _size, _val| {
                // Clonar el Arc primero: `get_data()` presta `uc` inmutable
                // y `mem_write` lo necesita `&mut`; con el clon, el lock no
                // retiene ningún préstamo sobre `uc`. Si la UI entró en
                // pánico con el lock tomado, se recupera igual: la cola es
                // POD, no hay invariantes a medio romper.
                let keyboard = Arc::clone(&uc.get_data().keyboard);
                let mut k = keyboard.lock().unwrap_or_else(|e| e.into_inner());
                // Un lb puede caer desalineado dentro del registro: se
                // normaliza a la palabra base (mismo truco del spike).
                let registro = (addr as u32) & !0x3;
                if registro == RECEIVER_DATA {
                    // Leer DATA consume (semántica RARS); cola vacía da 0.
                    let byte = u32::from(k.pop().unwrap_or(0));
                    let _ = uc.mem_write(RECEIVER_DATA as u64, &byte.to_le_bytes());
                }
                // CONTROL siempre se refresca al estado actual de la cola
                // (en lectura de CONTROL es lo único que pasa: no consume).
                // Los mem_write no pueden fallar: la página queda mapeada
                // de por vida; si fallaran, el guest leería el valor viejo.
                let ready = u32::from(k.hay_dato());
                let _ = uc.mem_write(RECEIVER_CONTROL as u64, &ready.to_le_bytes());
                true
            },
        )
        .map_err(|e| format!("no se pudo instalar el hook del teclado: {e:?}"))?;

        let mut m = Machine {
            uc,
            program,
            bp_hook: None,
            tb_de_corrida: false,
        };
        m.load()?;
        Ok(m)
    }

    /// Vuelca el programa a memoria y deja registros en el estado inicial.
    /// Es la operación de "Reiniciar" (RF-10): estado post-ensamblado.
    fn load(&mut self) -> Result<(), String> {
        let err = |e: uc_error| format!("error cargando el programa en memoria: {e:?}");
        {
            let d = self.uc.get_data_mut();
            d.video_last = None;
            d.video_used = false;
            // Sin teclas fantasma entre sesiones/reinicios (TEC2): lo
            // tipeado durante la corrida anterior no debe llegarle al
            // programa reiniciado.
            d.keyboard
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .limpiar();
            // Si la pantalla estaba en uso, publicar un frame negro: tras
            // Reiniciar el panel debe reflejar la VRAM limpia, no el último
            // frame de la corrida anterior.
            let mut v = d
                .video_shared
                .lock()
                .map_err(|_| String::from("estado de video corrupto"))?;
            if v.in_use {
                let mut negro = FramebufferRGBA::new();
                negro.convert(
                    &[0u8; FRAMEBUFFER_DATA_SIZE as usize],
                    &super::video::palette_vga13h_default(),
                );
                v.rgba = Some(negro.data);
                v.seq += 1;
            }
        }
        // Limpia regiones (suficiente con la extensión usada + un margen no
        // acotado por el programa: pila completa y datos completos, por si
        // la corrida anterior escribió en cualquier parte).
        self.uc
            .mem_write(TEXT_BASE as u64, &vec![0u8; TEXT_SIZE as usize])
            .map_err(err)?;
        self.uc
            .mem_write(
                DATA_REGION_BASE as u64,
                &vec![0u8; DATA_REGION_SIZE as usize],
            )
            .map_err(err)?;
        self.uc
            .mem_write(
                (STACK_TOP - STACK_SIZE) as u64,
                &vec![0u8; STACK_SIZE as usize],
            )
            .map_err(err)?;
        self.uc
            .mem_write(
                FRAMEBUFFER_BASE as u64,
                &vec![0u8; FRAMEBUFFER_DATA_SIZE as usize],
            )
            .map_err(err)?;

        use super::video::palette_vga13h_default;
        let pal = palette_vga13h_default();
        self.uc.mem_write(PALETTE_BASE as u64, &pal).map_err(err)?;
        self.uc
            .mem_write(VIDEO_CONTROL_BASE as u64, &[0u8; 4])
            .map_err(err)?;
        // CONTROL y DATA del teclado vuelven a 0 (acompaña el vaciado de
        // la cola de más arriba: el respaldo real tampoco conserva restos).
        self.uc
            .mem_write(RECEIVER_CONTROL as u64, &[0u8; 8])
            .map_err(err)?;
        self.uc
            .mem_write(TEXT_BASE as u64, &self.program.text)
            .map_err(err)?;
        self.uc
            .mem_write(DATA_BASE as u64, &self.program.data)
            .map_err(err)?;

        for n in 1..32u8 {
            self.uc.reg_write(xreg(n), 0).map_err(err)?;
        }
        self.uc.reg_write(xreg(2), SP_INIT as u64).map_err(err)?;
        self.uc.reg_write(xreg(3), GP_INIT as u64).map_err(err)?;
        self.uc
            .reg_write(PC, self.program.entry() as u64)
            .map_err(err)?;

        let d = self.uc.get_data_mut();
        d.exit_code = None;
        d.hit_breakpoint = None;
        d.runtime_error = None;
        d.suppress_bp = None;
        d.bad_mem = None;
        Ok(())
    }

    /// Reinicia al estado post-ensamblado (RF-10).
    pub fn reset(&mut self) -> Result<(), String> {
        self.load()
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Lee el pc. Si Unicorn no puede leerlo, el error se propaga en vez de
    /// disfrazarse como "pc = 0".
    pub fn pc(&self) -> Result<u32, String> {
        self.uc
            .reg_read(PC)
            .map(|v| v as u32)
            .map_err(|e| format!("no se pudo leer el pc: {e:?}"))
    }

    /// x0..x31 (x0 siempre 0). Si algún registro no se puede leer, el error
    /// se propaga en vez de reportar "el registro vale 0".
    pub fn registers(&self) -> Result<[u32; 32], String> {
        let mut regs = [0u32; 32];
        for (n, r) in regs.iter_mut().enumerate().skip(1) {
            *r = self
                .uc
                .reg_read(xreg(n as u8))
                .map_err(|e| format!("no se pudo leer el registro x{n}: {e:?}"))?
                as u32;
        }
        Ok(regs)
    }

    pub fn read_mem(&self, addr: u32, len: usize) -> Result<Vec<u8>, String> {
        self.uc
            .mem_read_as_vec(addr as u64, len)
            .map_err(|_| format!("la dirección 0x{addr:08X} no está mapeada"))
    }

    // --- API de soporte para tests de integración (fb0) ---
    // `read_mem` cubre las lecturas que usa producción, pero no existe una
    // API de escritura equivalente: estos cuatro helpers quedan para
    // tests/fb0_video_memory.rs.

    pub fn read_u8(&self, addr: u64) -> Result<u8, String> {
        self.read_mem(addr as u32, 1).map(|v| v[0])
    }

    pub fn write_u8(&mut self, addr: u64, val: u8) -> Result<(), String> {
        self.uc
            .mem_write(addr, &[val])
            .map_err(|_| format!("no se pudo escribir en 0x{addr:08X}"))
    }

    pub fn read_u32(&self, addr: u64) -> Result<u32, String> {
        self.read_mem(addr as u32, 4)
            .map(|v| u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
    }

    pub fn write_u32(&mut self, addr: u64, val: u32) -> Result<(), String> {
        self.uc
            .mem_write(addr, &val.to_le_bytes())
            .map_err(|_| format!("no se pudo escribir en 0x{addr:08X}"))
    }

    /// Lee framebuffer + paleta y convierte a RGBA para renderizar.
    /// Retorna Vec<u8> con 320×240×4 bytes (RGBA8888).
    pub fn read_framebuffer_rgba(&self) -> Result<Vec<u8>, String> {
        use super::video::{palette_vga13h_default, FramebufferRGBA};

        let fb_data = self.read_mem(FRAMEBUFFER_BASE, FRAMEBUFFER_DATA_SIZE as usize)?;
        let pal_data = self
            .read_mem(PALETTE_BASE, PALETTE_DATA_SIZE as usize)
            .unwrap_or_else(|_| palette_vga13h_default());

        let mut fb_rgba = FramebufferRGBA::new();
        fb_rgba.convert(&fb_data, &pal_data);
        Ok(fb_rgba.data)
    }

    /// Publica el frame actual en el estado compartido con la UI (FB2).
    /// El hilo de emulación lo llama al final de cada tramo y al detenerse.
    pub fn publish_frame(&mut self) {
        publicar_frame(&mut self.uc);
    }

    /// `Arc` del estado de video, para que la GUI lo lea sin pasar por el
    /// hilo de emulación (que puede estar bloqueado esperando entrada).
    pub fn video_shared(&self) -> Arc<Mutex<VideoShared>> {
        Arc::clone(&self.uc.get_data().video_shared)
    }

    /// `Arc` de la cola de teclas (TEC2), para que la GUI empuje teclas
    /// sin pasar por el hilo de emulación (espejo de `video_shared`).
    pub fn keyboard_shared(&self) -> Arc<Mutex<KeyboardShared>> {
        Arc::clone(&self.uc.get_data().keyboard)
    }

    /// Cablea la señal de stop de la sesión dentro de `MachData` (mismo
    /// `Arc` que mira el hilo de emulación entre tramos). Setter y no
    /// parámetro del constructor a propósito: no rompe ningún call site
    /// existente (tests y GUI siguen usando `new`/`new_with_video`) y la
    /// señal recién se consumirá en la fase del sleep interrumpible.
    pub fn set_stop_request(&mut self, stop: Arc<AtomicBool>) {
        self.uc.get_data_mut().stop_request = stop;
    }

    /// Define el conjunto de breakpoints (direcciones de .text). Instala o
    /// retira el hook de código según haga falta (clave para RNF-03).
    pub fn set_breakpoints(&mut self, addrs: HashSet<u32>) -> Result<(), String> {
        let need_hook = !addrs.is_empty();
        self.uc.get_data_mut().breakpoints = addrs;
        match (need_hook, self.bp_hook) {
            (true, None) => {
                let id = self
                    .uc
                    .add_code_hook(
                        TEXT_BASE as u64,
                        (TEXT_BASE + TEXT_SIZE) as u64,
                        |uc, addr, _| {
                            let d = uc.get_data_mut();
                            let addr = addr as u32;
                            if d.suppress_bp.take() == Some(addr) {
                                return;
                            }
                            if d.breakpoints.contains(&addr) {
                                d.hit_breakpoint = Some(addr);
                                let _ = uc.emu_stop();
                            }
                        },
                    )
                    .map_err(|e| format!("no se pudo instalar el hook de breakpoints: {e:?}"))?;
                self.bp_hook = Some(id);
                // Los bloques ya traducidos no quedan instrumentados por el
                // hook nuevo (misma caché JIT del bug de Reiniciar→Paso):
                // sin flush, un breakpoint puesto después de una corrida
                // libre no frena.
                let _ = self.uc.ctl_flush_tb();
                self.tb_de_corrida = false;
            }
            (false, Some(id)) => {
                self.uc
                    .remove_hook(id)
                    .map_err(|e| format!("no se pudo retirar el hook de breakpoints: {e:?}"))?;
                self.bp_hook = None;
                // Simétrico: los bloques instrumentados frenan aunque el
                // hook ya no exista; flushear para volver a correr a pleno.
                let _ = self.uc.ctl_flush_tb();
                self.tb_de_corrida = false;
            }
            _ => {}
        }
        Ok(())
    }

    fn clear_run_state(&mut self) {
        let d = self.uc.get_data_mut();
        d.hit_breakpoint = None;
        d.runtime_error = None;
        d.bad_mem = None;
    }

    /// Ejecuta una sola instrucción.
    pub fn step(&mut self) -> StopReason {
        if let Some(code) = self.uc.get_data().exit_code {
            return StopReason::Exited { code };
        }
        self.clear_run_state();
        if self.tb_de_corrida {
            // Sin esto, el paso ejecuta el bloque cacheado completo de la
            // corrida anterior en vez de UNA instrucción.
            let _ = self.uc.ctl_flush_tb();
            self.tb_de_corrida = false;
        }
        // Un paso nunca debe frenarse en el breakpoint del pc actual.
        let pc = match self.pc() {
            Ok(pc) => pc,
            Err(message) => return StopReason::RuntimeError { message },
        };
        self.uc.get_data_mut().suppress_bp = Some(pc);
        let result = self.uc.emu_start(pc as u64, NEVER, 0, 1);
        self.uc.get_data_mut().suppress_bp = None;
        self.interpret(result, StopReason::StepDone)
    }

    /// Ejecuta hasta breakpoint/salida/error o hasta agotar `timeout_ms`
    /// (0 = sin límite). OJO: sin tope de instrucciones un bucle apretado
    /// sin syscalls puede no retornar jamás (el timeout de Unicorn no lo
    /// corta); producción usa `run_slice` por eso (TEC1).
    pub fn run(&mut self, timeout_ms: u64) -> StopReason {
        self.run_slice(timeout_ms, 0)
    }

    /// Ejecuta hasta breakpoint/salida/error, o hasta agotar `timeout_ms`
    /// (0 = sin límite) o `max_instr` instrucciones (0 = sin límite), lo
    /// que llegue primero. El count agotado devuelve `Paused`: tramo
    /// terminado, NO pausa de usuario. El llamador decide refrescar la UI
    /// y volver a llamar mientras reciba `Paused` sin señal de stop.
    ///
    /// Es el PRIMER tramo de una orden Run: si el pc arranca sobre un
    /// breakpoint, se lo suprime una vez (reanudar desde un bp ya
    /// reportado no debe volver a frenar ahí). Para los tramos siguientes
    /// de la MISMA orden usá `run_slice_continuation`.
    pub fn run_slice(&mut self, timeout_ms: u64, max_instr: u64) -> StopReason {
        self.run_slice_imp(timeout_ms, max_instr, true)
    }

    /// Tramo de CONTINUACIÓN de una orden Run ya iniciada: NO suprime el
    /// breakpoint del pc de arranque. Si el tramo anterior agotó su count
    /// EXACTO en la dirección de un bp, el corte por count gana la carrera
    /// de hooks (el de breakpoints ni corre), el tramo devuelve `Paused` y
    /// ese bp queda SIN reportar: suprimirlo acá lo haría pasar de largo
    /// (regresión vigilada en tests/tec1_detener.rs).
    pub fn run_slice_continuation(&mut self, timeout_ms: u64, max_instr: u64) -> StopReason {
        self.run_slice_imp(timeout_ms, max_instr, false)
    }

    /// Implementación común de los tramos: `suppress_bp_at_start` decide
    /// si el breakpoint del pc de arranque se ignora una vez (solo el
    /// primer tramo de una orden Run; ver run_slice/run_slice_continuation).
    fn run_slice_imp(
        &mut self,
        timeout_ms: u64,
        max_instr: u64,
        suppress_bp_at_start: bool,
    ) -> StopReason {
        if let Some(code) = self.uc.get_data().exit_code {
            return StopReason::Exited { code };
        }
        self.clear_run_state();
        let pc = match self.pc() {
            Ok(pc) => pc,
            Err(message) => return StopReason::RuntimeError { message },
        };
        // Al reanudar sobre un breakpoint, ese breakpoint no cuenta — pero
        // SOLO en el primer tramo de la orden: en una continuación el bp
        // del pc de arranque todavía no se reportó y debe frenar.
        if suppress_bp_at_start && self.uc.get_data().breakpoints.contains(&pc) {
            self.uc.get_data_mut().suppress_bp = Some(pc);
        }
        let result = self
            .uc
            .emu_start(pc as u64, NEVER, timeout_ms * 1000, max_instr as usize);
        // Conservador a propósito: el spike (PoC3b) mostró que tras corridas
        // con count>0 un paso count=1 sí se respeta, pero el flush barato de
        // step() sigue cubriendo el caso histórico (count=0) sin distinguir.
        self.tb_de_corrida = true;
        self.uc.get_data_mut().suppress_bp = None;
        self.interpret(result, StopReason::Paused)
    }

    /// Traduce el resultado de `emu_start` + flags de los hooks a un
    /// `StopReason`, con errores en español.
    fn interpret(&mut self, result: Result<(), uc_error>, normal: StopReason) -> StopReason {
        // El pc se lee antes de tomar `data` (préstamos); solo se usa para
        // armar el mensaje de error, así que su fallo se difiere a esa rama.
        let pc = self.pc();
        let d = self.uc.get_data_mut();
        if let Some(msg) = d.runtime_error.take() {
            return StopReason::RuntimeError { message: msg };
        }
        if let Some(code) = d.exit_code {
            return StopReason::Exited { code };
        }
        if let Some(addr) = d.hit_breakpoint.take() {
            return StopReason::Breakpoint { addr };
        }
        match result {
            Ok(()) => normal,
            Err(e) => {
                let bad = d.bad_mem.take();
                match pc {
                    Ok(pc) => StopReason::RuntimeError {
                        message: runtime_error_message(e, pc, bad),
                    },
                    // Si ni el pc se pudo leer, ese fallo ES el diagnóstico:
                    // no inventamos un pc en 0 para el mensaje didáctico.
                    Err(message) => StopReason::RuntimeError { message },
                }
            }
        }
    }
}

/// Mensajes didácticos para los errores de ejecución de Unicorn.
fn runtime_error_message(e: uc_error, pc: u32, bad_mem: Option<(MemType, u64)>) -> String {
    let mem_detail = |verbo: &str| -> String {
        match bad_mem {
            Some((_, addr)) => format!(
                "el programa intentó {verbo} la dirección 0x{addr:08X}, que no pertenece \
                 a .data, a la pila ni a ninguna zona válida (pc = 0x{pc:08X}). \
                 Revisá que el registro con la dirección esté bien cargado (¿usaste «la»?)"
            ),
            None => format!("acceso a memoria inválido cerca de pc = 0x{pc:08X}"),
        }
    };
    match e {
        uc_error::READ_UNMAPPED | uc_error::READ_PROT => mem_detail("leer"),
        uc_error::WRITE_UNMAPPED | uc_error::WRITE_PROT => mem_detail("escribir en"),
        uc_error::FETCH_UNMAPPED | uc_error::FETCH_PROT => format!(
            "el pc saltó a 0x{pc:08X}, fuera del código del programa. \
             Causa típica: «jr» o «ret» con una dirección de retorno pisada; \
             revisá el manejo de ra y de la pila"
        ),
        uc_error::INSN_INVALID => format!(
            "instrucción inválida en 0x{pc:08X}. Si la dirección está después de tu \
             última instrucción, el programa siguió de largo: terminálo con \
             «li a7, 10» + «ecall»"
        ),
        uc_error::READ_UNALIGNED | uc_error::WRITE_UNALIGNED | uc_error::FETCH_UNALIGNED => {
            format!(
                "acceso a memoria desalineado cerca de pc = 0x{pc:08X}: \
                 lw/sw necesitan direcciones múltiplo de 4 y lh/sh múltiplo de 2"
            )
        }
        other => format!("error del emulador en pc = 0x{pc:08X}: {other:?}"),
    }
}

/// Publica el frame actual (VRAM + paleta → RGBA) en `video_shared` si la
/// VRAM cambió desde la última publicación (flag sucio, RF2). Función libre
/// sobre `Unicorn` para poder llamarla también desde el hook de syscalls,
/// donde no hay `Machine`. Mientras el programa no use la pantalla no
/// publica nada (RF4: cero costo para programas sin gráficos).
pub fn publicar_frame(uc: &mut Unicorn<'_, MachData>) {
    let Ok(fb) = uc.mem_read_as_vec(FRAMEBUFFER_BASE as u64, FRAMEBUFFER_DATA_SIZE as usize) else {
        return;
    };
    if !uc.get_data().video_used {
        let ctrl = uc
            .mem_read_as_vec(VIDEO_CONTROL_BASE as u64, 4)
            .unwrap_or_default();
        let usado = ctrl.iter().any(|&b| b != 0) || fb.iter().any(|&b| b != 0);
        if !usado {
            return;
        }
        uc.get_data_mut().video_used = true;
    }
    let Ok(pal) = uc.mem_read_as_vec(PALETTE_BASE as u64, PALETTE_DATA_SIZE as usize) else {
        return;
    };
    let mut snap = fb;
    snap.extend_from_slice(&pal);
    if uc.get_data().video_last.as_deref() == Some(snap.as_slice()) {
        return;
    }
    let (fb_data, pal_data) = snap.split_at(FRAMEBUFFER_DATA_SIZE as usize);
    let mut rgba = FramebufferRGBA::new();
    rgba.convert(fb_data, pal_data);
    {
        // Si otro hilo entró en pánico con el lock tomado, el mutex queda
        // envenenado: lo recuperamos igual porque los datos del frame son
        // POD (bytes RGBA y contadores), no hay invariantes que puedan
        // quedar rotos a medias.
        let mut v = uc
            .get_data()
            .video_shared
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        v.rgba = Some(rgba.data);
        v.seq += 1;
        v.in_use = true;
    }
    uc.get_data_mut().video_last = Some(snap);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::assembler::assemble;
    use std::sync::mpsc::channel;

    /// Máquina lista + extremos de los canales de consola.
    fn machine_for(src: &str) -> (Machine<'static>, Receiver<ConsoleEvent>, Sender<InputMsg>) {
        let program = assemble(src).expect("el fuente de prueba debe ensamblar");
        let (out_tx, out_rx) = channel();
        let (in_tx, in_rx) = channel();
        let m = Machine::new(program, out_tx, in_rx).expect("máquina");
        (m, out_rx, in_tx)
    }

    fn collected_output(rx: &Receiver<ConsoleEvent>) -> String {
        let mut s = String::new();
        while let Ok(ev) = rx.try_recv() {
            if let ConsoleEvent::Output(t) = ev {
                s.push_str(&t);
            }
        }
        s
    }

    #[test]
    fn estado_inicial_estilo_rars() {
        let (m, _o, _i) = machine_for("nop");
        assert_eq!(m.pc().unwrap(), TEXT_BASE);
        let r = m.registers().unwrap();
        assert_eq!(r[2], SP_INIT); // sp
        assert_eq!(r[3], GP_INIT); // gp
        assert_eq!(r[0], 0);
    }

    #[test]
    fn paso_a_paso_actualiza_registros() {
        let (mut m, _o, _i) = machine_for("li t0, 7\nli t1, 35\nadd t2, t0, t1");
        assert_eq!(m.step(), StopReason::StepDone);
        assert_eq!(m.registers().unwrap()[5], 7);
        assert_eq!(m.pc().unwrap(), TEXT_BASE + 4);
        m.step();
        m.step();
        assert_eq!(m.registers().unwrap()[7], 42);
    }

    #[test]
    fn ecall_avanza_pc_y_no_se_repite() {
        // Si el hook no avanzara el pc, la syscall 1 se ejecutaría infinitas
        // veces y este test no terminaría / acumularía salida repetida.
        let (mut m, out, _i) = machine_for("li a0, 5\nli a7, 1\necall\nli a1, 9\nli a7, 10\necall");
        let r = m.run(0);
        assert_eq!(r, StopReason::Exited { code: 0 });
        assert_eq!(collected_output(&out), "5");
        assert_eq!(m.registers().unwrap()[11], 9); // la instrucción post-ecall corrió
    }

    #[test]
    fn salida_con_codigo_y_estado_final() {
        let (mut m, _o, _i) = machine_for("li a0, 3\nli a7, 10\necall\nli a0, 99");
        assert_eq!(m.run(0), StopReason::Exited { code: 0 });
        assert_eq!(m.registers().unwrap()[10], 3); // li a0,99 nunca se ejecutó
                                                   // correr de nuevo sigue reportando la salida
        assert_eq!(m.run(0), StopReason::Exited { code: 0 });
    }

    #[test]
    fn breakpoint_detiene_sin_ejecutar_y_reanuda() {
        let src = "li t0, 1\nli t1, 2\nli t2, 3\nli a7, 10\necall";
        let (mut m, _o, _i) = machine_for(src);
        // breakpoint en la 3ª instrucción (li t2)
        let bp = TEXT_BASE + 8;
        m.set_breakpoints(HashSet::from([bp])).unwrap();
        assert_eq!(m.run(0), StopReason::Breakpoint { addr: bp });
        assert_eq!(m.pc().unwrap(), bp);
        assert_eq!(m.registers().unwrap()[6], 2); // t1 ya asignado
        assert_eq!(m.registers().unwrap()[7], 0); // t2 todavía no
                                                  // reanudar desde el breakpoint no debe volver a frenar ahí
        assert_eq!(m.run(0), StopReason::Exited { code: 0 });
        assert_eq!(m.registers().unwrap()[7], 3);
    }

    #[test]
    fn reset_vuelve_al_estado_post_ensamblado() {
        let src = ".data\nv: .word 1\n.text\nli t0, 5\nla t1, v\nli t2, 8\nsw t2, 0(t1)\nli a7, 10\necall";
        let (mut m, _o, _i) = machine_for(src);
        assert_eq!(m.run(0), StopReason::Exited { code: 0 });
        let w = m.read_mem(DATA_BASE, 4).unwrap();
        assert_eq!(w, 8u32.to_le_bytes());
        m.reset().unwrap();
        assert_eq!(m.pc().unwrap(), TEXT_BASE);
        assert_eq!(m.registers().unwrap()[5], 0);
        assert_eq!(m.read_mem(DATA_BASE, 4).unwrap(), 1u32.to_le_bytes());
    }

    /// Mismo origen que el bug de Reiniciar→Paso (caché JIT): un breakpoint
    /// puesto DESPUÉS de una corrida libre debe frenar igual.
    #[test]
    fn breakpoint_puesto_despues_de_correr_frena() {
        let src = ".text\nli t0, 1\nli t0, 2\nli t0, 3\nli a7, 10\necall";
        let (mut m, _o, _i) = machine_for(src);
        assert_eq!(m.run(0), StopReason::Exited { code: 0 });
        m.reset().unwrap();
        let bp = TEXT_BASE + 8; // tercera instrucción
        m.set_breakpoints([bp].into()).unwrap();
        assert_eq!(m.run(0), StopReason::Breakpoint { addr: bp });
        assert_eq!(
            m.registers().unwrap()[5],
            2,
            "ejecutó hasta antes del breakpoint"
        );
    }

    /// Regresión del bug reportado: tras Reiniciar, «Paso» no avanzaba y
    /// solo se recuperaba re-ensamblando. Reset → step debe ejecutar la
    /// primera instrucción inmediatamente, y los pasos siguientes seguir.
    #[test]
    fn paso_funciona_inmediatamente_despues_de_reset() {
        let src = ".text\nli t0, 5\nli t1, 7\nadd t2, t0, t1\nli a7, 10\necall";
        let (mut m, _o, _i) = machine_for(src);
        assert_eq!(m.run(0), StopReason::Exited { code: 0 });
        m.reset().unwrap();

        assert_eq!(m.pc().unwrap(), TEXT_BASE);
        assert_eq!(m.step(), StopReason::StepDone, "primer paso tras reset");
        assert_eq!(m.pc().unwrap(), TEXT_BASE + 4, "el pc debe avanzar");
        assert_eq!(m.registers().unwrap()[5], 5, "li t0, 5 ejecutado");

        assert_eq!(m.step(), StopReason::StepDone, "segundo paso");
        assert_eq!(m.registers().unwrap()[6], 7, "li t1, 7 ejecutado");
        assert_eq!(m.step(), StopReason::StepDone, "tercer paso");
        assert_eq!(m.registers().unwrap()[7], 12, "add ejecutado");
    }

    #[test]
    fn error_didactico_en_acceso_invalido() {
        let (mut m, _o, _i) = machine_for("li t0, 0x500\nlw t1, 0(t0)");
        match m.run(0) {
            StopReason::RuntimeError { message } => {
                assert!(message.contains("0x00000500"), "{message}");
                assert!(message.contains("leer"), "{message}");
            }
            other => panic!("se esperaba RuntimeError, hubo {other:?}"),
        }
    }

    #[test]
    fn error_didactico_si_no_hay_salida() {
        let (mut m, _o, _i) = machine_for("li t0, 1");
        match m.run(0) {
            StopReason::RuntimeError { message } => {
                assert!(message.contains("li a7, 10"), "{message}");
            }
            other => panic!("se esperaba RuntimeError, hubo {other:?}"),
        }
    }

    #[test]
    fn syscall_de_entrada_bloqueante() {
        // syscall 5: la máquina pide entrada y espera el canal (RF-07).
        let (mut m, out, input) = machine_for("li a7, 5\necall\nmv t0, a0\nli a7, 10\necall");
        input.send(InputMsg::Line("123".into())).unwrap();
        assert_eq!(m.run(0), StopReason::Exited { code: 0 });
        assert_eq!(m.registers().unwrap()[5], 123);
        let mut saw_request = false;
        while let Ok(ev) = out.try_recv() {
            if matches!(ev, ConsoleEvent::InputRequest) {
                saw_request = true;
            }
        }
        assert!(saw_request, "debe emitirse InputRequest antes de bloquear");
    }
}
