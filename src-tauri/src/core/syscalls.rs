//! Syscalls estilo RARS (RF-07): selector en a7, argumentos en a0/a1.
//!
//! | a7 | Servicio        | Entrada              | Salida        |
//! |----|-----------------|----------------------|---------------|
//! | 1  | imprimir entero | a0 = valor (i32)     | —             |
//! | 4  | imprimir cadena | a0 = dir. cadena NUL | —             |
//! | 5  | leer entero     | —                    | a0 = valor    |
//! | 10 | terminar        | —                    | —             |
//! | 11 | imprimir carácter | a0 = código        | —             |
//!
//! `ecall` llega como interrupción de Unicorn con el número de causa
//! RISC-V en `intno`. Verificado empíricamente con Unicorn 2.1.5
//! (ver BITACORA.md): el hook dispara con el pc YA avanzado a la
//! instrucción siguiente, y las excepciones de instrucción ilegal también
//! llegan por este hook (no como error de `emu_start`).

use super::machine::MachData;
use unicorn_engine::{RegisterRISCV, Unicorn};

/// Eventos que la consola de la UI (o un test) recibe de la máquina.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleEvent {
    /// Texto a mostrar en la consola.
    Output(String),
    /// El programa espera una línea de entrada (syscall 5).
    InputRequest,
}

/// Mensajes hacia la máquina cuando está esperando entrada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMsg {
    /// Línea tipeada por el usuario.
    Line(String),
    /// La UI canceló (stop/reiniciar): la ejecución se frena con pausa.
    Cancel,
}

const EBREAK: u32 = 0x0010_0073;
/// `RegisterRISCV::X0 == 1`: a0 = x10 → 11, a7 = x17 → 18.
const A0: i32 = 11;
const A7: i32 = 18;
/// Límite de seguridad al leer cadenas NUL-terminated (RF-07).
const MAX_STR: u64 = 1 << 20;

/// Causas de excepción RISC-V (tabla mcause, espec privilegiada).
const CAUSE_ILLEGAL_INSN: u32 = 2;
const CAUSE_BREAKPOINT: u32 = 3;
const CAUSE_ECALL_U: u32 = 8;
const CAUSE_ECALL_S: u32 = 9;
const CAUSE_ECALL_M: u32 = 11;

/// Atiende la interrupción/excepción del CPU emulado.
///
/// Empírico (Unicorn 2.1.5): al entrar, el pc ya apunta a la instrucción
/// siguiente a la que causó la excepción, así que para `ecall` no hay que
/// avanzarlo y para los mensajes de error la instrucción culpable está en
/// `pc - 4`.
pub fn handle_trap(uc: &mut Unicorn<'_, MachData>, intno: u32) {
    let pc = uc.reg_read(RegisterRISCV::PC).unwrap_or(0);
    match intno {
        CAUSE_ECALL_U | CAUSE_ECALL_S | CAUSE_ECALL_M => {
            dispatch(uc, pc.wrapping_sub(4));
        }
        CAUSE_BREAKPOINT => {
            // ebreak = pausa por software: la GUI muestra el estado ahí.
            // Si el pc no avanzó (semántica de depurador), lo avanzamos para
            // poder reanudar sin re-ejecutar el ebreak.
            let mut insn = [0u8; 4];
            if uc.mem_read(pc, &mut insn).is_ok() && u32::from_le_bytes(insn) == EBREAK {
                let _ = uc.reg_write(RegisterRISCV::PC, pc + 4);
            }
            let _ = uc.emu_stop();
        }
        CAUSE_ILLEGAL_INSN => {
            let at = pc.wrapping_sub(4) as u32;
            fail(
                uc,
                format!(
                    "instrucción inválida en 0x{at:08X}. Si esa dirección está después de tu \
                     última instrucción, el programa siguió de largo: terminálo con \
                     «li a7, 10» + «ecall»"
                ),
            );
        }
        other => {
            fail(
                uc,
                format!(
                    "excepción de CPU no manejada (causa {other}) cerca de 0x{:08X}",
                    pc.wrapping_sub(4)
                ),
            );
        }
    }
}

fn fail(uc: &mut Unicorn<'_, MachData>, msg: String) {
    uc.get_data_mut().runtime_error = Some(msg);
    let _ = uc.emu_stop();
}

fn dispatch(uc: &mut Unicorn<'_, MachData>, pc: u64) {
    let a7 = uc.reg_read(A7).unwrap_or(0) as u32;
    match a7 {
        1 => {
            let v = uc.reg_read(A0).unwrap_or(0) as u32 as i32;
            send_output(uc, v.to_string());
        }
        4 => {
            let addr = uc.reg_read(A0).unwrap_or(0) as u32 as u64;
            match read_c_string(uc, addr) {
                Ok(s) => send_output(uc, s),
                Err(CStringError::Unreadable(bad)) => fail(
                    uc,
                    format!(
                        "syscall 4 (imprimir cadena): no se pudo leer la dirección 0x{bad:08X}. \
                         a0 debe apuntar a una cadena en .data (cargala con «la»)"
                    ),
                ),
                Err(CStringError::MissingNul) => fail(
                    uc,
                    format!(
                        "syscall 4 (imprimir cadena): cadena sin terminador NUL (límite 1 MB) \
                         desde 0x{addr:08X}. Declarala con «.asciz» o agregale el byte 0 final"
                    ),
                ),
            }
        }
        5 => read_int(uc),
        10 => {
            // Convención RARS: la syscall 10 termina SIEMPRE con código 0
            // (no recibe argumento). Un código de salida variable llegaría
            // con la syscall 93 (exit2 con código), trabajo futuro declarado.
            uc.get_data_mut().exit_code = Some(0);
            let _ = uc.emu_stop();
        }
        11 => {
            let v = uc.reg_read(A0).unwrap_or(0) as u32;
            let c = char::from_u32(v).unwrap_or(char::REPLACEMENT_CHARACTER);
            send_output(uc, c.to_string());
        }
        other => fail(
            uc,
            format!(
                "syscall desconocida: a7 = {other} (pc = 0x{pc:08X}). \
                 Servicios disponibles: 1 imprimir entero, 4 imprimir cadena, \
                 5 leer entero, 10 terminar, 11 imprimir carácter"
            ),
        ),
    }
}

fn send_output(uc: &mut Unicorn<'_, MachData>, s: String) {
    // Si la UI cerró el canal, no es un error del programa del estudiante.
    let _ = uc.get_data().out.send(ConsoleEvent::Output(s));
}

/// Errores posibles al leer una cadena C de la memoria emulada.
enum CStringError {
    /// La dirección indicada no se pudo leer (no está mapeada).
    Unreadable(u64),
    /// Se alcanzó `MAX_STR` sin encontrar el byte NUL terminador.
    MissingNul,
}

/// Lee la cadena NUL-terminated apuntada por `addr` (bytes UTF-8).
/// Si no aparece el NUL dentro de `MAX_STR` bytes, falla de forma
/// explícita en vez de truncar en silencio.
fn read_c_string(uc: &mut Unicorn<'_, MachData>, addr: u64) -> Result<String, CStringError> {
    let mut bytes = Vec::new();
    let mut a = addr;
    loop {
        if a - addr >= MAX_STR {
            return Err(CStringError::MissingNul);
        }
        let mut b = [0u8; 1];
        uc.mem_read(a, &mut b)
            .map_err(|_| CStringError::Unreadable(a))?;
        if b[0] == 0 {
            break;
        }
        bytes.push(b[0]);
        a += 1;
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Syscall 5: pide una línea a la UI y bloquea el hilo de emulación hasta
/// recibirla (ARQUITECTURA.md §Diseños clave 4). Reintenta si la línea no
/// es un entero válido, avisando por consola.
fn read_int(uc: &mut Unicorn<'_, MachData>) {
    // El hilo va a quedar bloqueado esperando la línea: publicar antes el
    // frame de video pendiente, o un juego por turnos pediría entrada con
    // el tablero todavía invisible (FB2).
    super::machine::publicar_frame(uc);
    loop {
        let _ = uc.get_data().out.send(ConsoleEvent::InputRequest);
        // En variable propia: el temporal del scrutinee retendría el préstamo
        // inmutable de `uc` durante todo el match.
        let received = uc.get_data().input.recv();
        match received {
            Ok(InputMsg::Line(line)) => match line.trim().parse::<i64>() {
                Ok(v) if (i32::MIN as i64..=u32::MAX as i64).contains(&v) => {
                    let _ = uc.reg_write(A0, v as u32 as u64);
                    return;
                }
                _ => {
                    send_output(
                        uc,
                        format!("«{}» no es un entero válido; probá de nuevo: ", line.trim()),
                    );
                }
            },
            Ok(InputMsg::Cancel) | Err(_) => {
                // Stop pedido por la UI (o canal cerrado): pausa limpia.
                let _ = uc.emu_stop();
                return;
            }
        }
    }
}
