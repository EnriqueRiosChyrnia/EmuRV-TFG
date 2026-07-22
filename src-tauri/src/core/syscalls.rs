//! Syscalls estilo RARS (RF-07): selector en a7, argumentos en a0/a1.
//!
//! | a7 | Servicio        | Entrada              | Salida        |
//! |----|-----------------|----------------------|---------------|
//! | 1  | imprimir entero | a0 = valor (i32)     | —             |
//! | 2  | imprimir float  | fa0 = valor (f32)    | —             |
//! | 3  | imprimir double | fa0 = valor (f64)    | —             |
//! | 4  | imprimir cadena | a0 = dir. cadena NUL | —             |
//! | 5  | leer entero     | —                    | a0 = valor    |
//! | 6  | leer float      | —                    | fa0 = valor (NaN-boxed) |
//! | 7  | leer double     | —                    | fa0 = valor   |
//! | 8  | leer cadena     | a0 = buffer, a1 = máx (con NUL) | escribe ≤ a1−1 bytes + NUL |
//! | 9  | sbrk            | a0 = bytes           | a0 = dirección |
//! | 10 | terminar        | —                    | —             |
//! | 11 | imprimir carácter | a0 = código        | —             |
//! | 30 | tiempo en ms    | —                    | a0 = bits bajos, a1 = bits altos |
//! | 32 | dormir          | a0 = ms (sin signo)  | —             |
//! | 60 | limpiar consola (extensión EmuRV, no RARS) | — | —     |
//! | 93 | terminar con código (Exit2) | a0 = código (i32) | —    |
//!
//! El 17 (Exit2 de MARS/MIPS) NO existe en RARS ni acá: se rechaza con un
//! mensaje que apunta al 93 (RP10; fuente en BITACORA).
//!
//! `ecall` llega como interrupción de Unicorn con el número de causa
//! RISC-V en `intno`. Verificado empíricamente con Unicorn 2.1.5
//! (ver BITACORA.md): el hook dispara con el pc YA avanzado a la
//! instrucción siguiente, y las excepciones de instrucción ilegal también
//! llegan por este hook (no como error de `emu_start`).

use super::machine::MachData;
use super::{
    CSR_ID_MCAUSE, CSR_ID_MEPC, CSR_ID_MSTATUS, CSR_ID_MTVAL, CSR_ID_MTVEC, DATA_REGION_BASE,
    FREG_ID_BASE,
    DATA_REGION_SIZE, FRAMEBUFFER_BASE, FRAMEBUFFER_SIZE, HEAP_BASE, KEYBOARD_BASE, KEYBOARD_SIZE,
    PALETTE_BASE, PALETTE_SIZE, STACK_SIZE, STACK_TOP, TEXT_BASE, TEXT_SIZE,
};
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime};
use unicorn_engine::{RegisterRISCV, Unicorn};

/// Eventos que la consola de la UI (o un test) recibe de la máquina.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleEvent {
    /// Texto a mostrar en la consola.
    Output(String),
    /// El programa espera una línea de entrada (syscall 5 u 8).
    InputRequest,
    /// El programa pidió limpiar la consola (syscall 60, extensión EmuRV).
    Clear,
}

/// Mensajes hacia la máquina cuando está esperando entrada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMsg {
    /// Línea tipeada por el usuario.
    Line(String),
    /// La UI canceló (stop/reiniciar): la ejecución se frena con pausa.
    Cancel,
}

use super::EBREAK_WORD;
/// `RegisterRISCV::X0 == 1`: a0 = x10 → 11, a1 = x11 → 12, a7 = x17 → 18.
const A0: i32 = 11;
const A1: i32 = 12;
const A7: i32 = 18;
/// fa0 = f10 (RP12): id del binding = FREG_ID_BASE + 10.
const FA0: i32 = FREG_ID_BASE + 10;
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
            // Toda syscall es actividad observable del guest: exime al
            // programa de la pista de «posible bucle infinito» (E/S,
            // sleep, etc. — ver Machine::actividad).
            uc.get_data().actividad.fetch_add(1, Ordering::Relaxed);
            dispatch(uc, pc.wrapping_sub(4));
        }
        CAUSE_BREAKPOINT => {
            // RAMA DEFENSIVA — hoy INALCANZABLE [BITACORA RP1-F2, Seguro]:
            // Unicorn 2.1.5 NO entrega el trap de `ebreak` a este hook;
            // `emu_start` aborta con INSN_INVALID y el caso real se rescata
            // en `Machine::interpret()`. Se conserva el manejo correcto por
            // si una versión futura sí lo entrega: localizar la dirección
            // real del ebreak (la palabra puede estar en pc o, si el pc ya
            // vino avanzado, en pc-4), fijar pc = dir+4 UNA sola vez y
            // marcar la pausa por software para `interpret()`.
            let palabra = |uc: &mut Unicorn<'_, MachData>, dir: u64| -> Option<u32> {
                let mut w = [0u8; 4];
                uc.mem_read(dir, &mut w).ok()?;
                Some(u32::from_le_bytes(w))
            };
            let dir_ebreak = if palabra(uc, pc) == Some(EBREAK_WORD) {
                Some(pc)
            } else if palabra(uc, pc.wrapping_sub(4)) == Some(EBREAK_WORD) {
                Some(pc.wrapping_sub(4))
            } else {
                None
            };
            if let Some(dir) = dir_ebreak {
                let _ = uc.reg_write(RegisterRISCV::PC, dir + 4);
                uc.get_data_mut().ebreak_pause = Some(dir as u32);
            }
            let _ = uc.emu_stop();
        }
        CAUSE_ILLEGAL_INSN => {
            let at = pc.wrapping_sub(4) as u32;
            // RP8: con mtvec, la instrucción ilegal se delega (mcause=2,
            // mtval = la palabra culpable si es legible).
            let mut palabra = [0u8; 4];
            let mtval = if uc.mem_read(u64::from(at), &mut palabra).is_ok() {
                u32::from_le_bytes(palabra)
            } else {
                0
            };
            if !delegar_trap(uc, 2, mtval, at) {
                fail(
                    uc,
                    format!(
                        "instrucción inválida en 0x{at:08X}. Si esa dirección está después de tu \
                         última instrucción, el programa siguió de largo: terminálo con \
                         «li a7, 10» + «ecall»"
                    ),
                );
            }
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

/// Delegación didáctica de una excepción SÍNCRONA al handler del guest
/// (RP8): escribe mepc/mcause/mtval, apaga mstatus.MIE (protocolo de la
/// espec: la doble falta se detecta porque MIE ya está en 0 — `mret`
/// nativo la restaura, autolimpiante) y reinyecta pc←mtvec SIN frenar
/// (mecanismo validado por el spike P3b2). Devuelve false si NO delegó
/// (mtvec==0): el llamador sigue con el camino didáctico de siempre.
pub(crate) fn delegar_trap(
    uc: &mut Unicorn<'_, MachData>,
    mcause: u32,
    mtval: u32,
    mepc: u32,
) -> bool {
    let mtvec = uc.reg_read(CSR_ID_MTVEC).unwrap_or(0);
    if mtvec == 0 {
        return false; // sin handler instalado: comportamiento de siempre
    }
    let mstatus = uc.reg_read(CSR_ID_MSTATUS).unwrap_or(0);
    const MIE: u64 = 1 << 3;
    const MPIE: u64 = 1 << 7;
    if mstatus & MIE == 0 {
        fail(
            uc,
            format!(
                "excepción dentro del manejador de excepciones (doble falta): \
                 el manejador en mtvec=0x{mtvec:08X} provocó otra excepción \
                 (mcause={mcause}). Revisá que el manejador no acceda a memoria \
                 inválida ni use servicios desconocidos"
            ),
        );
        return true; // «manejado»: ya cortamos con el error didáctico
    }
    let nuevo_mstatus = (mstatus & !MIE) | MPIE;
    let _ = uc.reg_write(CSR_ID_MSTATUS, nuevo_mstatus);
    let _ = uc.reg_write(CSR_ID_MEPC, u64::from(mepc));
    let _ = uc.reg_write(CSR_ID_MCAUSE, u64::from(mcause));
    let _ = uc.reg_write(CSR_ID_MTVAL, u64::from(mtval));
    let _ = uc.reg_write(RegisterRISCV::PC, mtvec);
    true
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
        // RP12 — flotantes RARS (2/3 imprimen fa0; 6/7 leen a fa0).
        // Formato de impresión: el «{}» de Rust (1.5, -0, NaN, inf) —
        // DIVERGENCIA REGISTRADA con RARS/Java (que imprime 1.5 igual,
        // pero p. ej. Infinity en vez de inf); el test fija NUESTRO formato.
        2 => {
            let bits = uc.reg_read(FA0).unwrap_or(0) as u32;
            send_output(uc, f32::from_bits(bits).to_string());
        }
        3 => {
            let bits = uc.reg_read(FA0).unwrap_or(0);
            send_output(uc, f64::from_bits(bits).to_string());
        }
        5 => read_int(uc),
        6 => read_float(uc, true),
        7 => read_float(uc, false),
        8 => leer_cadena(uc),
        9 => sbrk(uc),
        10 => {
            // Convención RARS: la syscall 10 termina SIEMPRE con código 0
            // (no recibe argumento). El código variable es el 93 (Exit2).
            uc.get_data_mut().exit_code = Some(0);
            let _ = uc.emu_stop();
        }
        11 => {
            let v = uc.reg_read(A0).unwrap_or(0) as u32;
            let c = char::from_u32(v).unwrap_or(char::REPLACEMENT_CHARACTER);
            send_output(uc, c.to_string());
        }
        30 => {
            // Compatible RARS: los 32 bits bajos del tiempo en a0 y los
            // altos en a1 (en RV32 un valor de 64 bits viaja en dos regs).
            let ms = tiempo_ms();
            let _ = uc.reg_write(A0, ms & 0xFFFF_FFFF);
            let _ = uc.reg_write(A1, ms >> 32);
        }
        32 => dormir(uc),
        60 => {
            // Extensión EmuRV (no existe en RARS; documentada en Ayuda y
            // docs): limpiar la consola. La UI vacía el panel; el CLI solo
            // emite la secuencia ANSI si stdout es una terminal real.
            let _ = uc.get_data().out.send(ConsoleEvent::Clear);
        }
        93 => {
            // Exit2 (RARS, número del ABI Linux RISC-V): termina con el
            // código de a0. El núcleo guarda el i32 REAL; el acotamiento a
            // rango seguro de shell (0..=125) es responsabilidad del CLI.
            let code = uc.reg_read(A0).unwrap_or(0) as u32 as i32;
            uc.get_data_mut().exit_code = Some(code);
            let _ = uc.emu_stop();
        }
        17 => {
            // El 17 era el Exit2 de MARS (MIPS); RARS lo reemplazó por el
            // 93 y EmuRV sigue a RARS [BITACORA RP10-F1, wiki oficial de
            // RARS «Environment Calls»]. Para la delegación RP-8 sigue
            // siendo un servicio DESCONOCIDO: con mtvec el handler del
            // guest tiene prioridad; el mensaje didáctico es el camino sin
            // handler instalado.
            if !delegar_trap(uc, 11, 0, pc as u32) {
                fail(
                    uc,
                    format!(
                        "syscall 17 (pc = 0x{pc:08X}): ese servicio era el Exit2 de MARS \
                         (MIPS) y no existe en RARS ni en EmuRV. Para terminar con un \
                         código de salida usá el servicio 93: cargá el código en a0 y \
                         ejecutá «li a7, 93» + «ecall»"
                    ),
                );
            }
        }
        other => {
            // RP8: con mtvec configurado, un ecall DESCONOCIDO se delega al
            // handler del guest (mcause=11, ECALL desde M-mode según la
            // espec — no el intno interno de Unicorn). mepc = dirección del
            // ecall, fiel a la espec: el handler ajusta mepc+4 antes de
            // mret (como un kernel real; el ejemplo 12 lo enseña). Los
            // ecall CONOCIDOS jamás se delegan (este brazo es solo a7
            // desconocido).
            if !delegar_trap(uc, 11, 0, pc as u32) {
                fail(
                    uc,
                    format!(
                        "syscall desconocida: a7 = {other} (pc = 0x{pc:08X}). \
                         Servicios disponibles: 1 imprimir entero, 2/3 imprimir \
                         float/double, 4 imprimir cadena, 5 leer entero, 6/7 leer \
                         float/double, 8 leer cadena, 9 reservar memoria (sbrk), \
                         10 terminar, 11 imprimir carácter, \
                         30 tiempo del sistema en ms, 32 dormir a0 ms, \
                         60 limpiar consola, 93 terminar con código (a0)"
                    ),
                );
            }
        }
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

/// Valida que `[addr, addr+len)` caiga COMPLETO en una zona escribible por
/// el programa — espejo de los rechazos didácticos de la edición en vivo
/// (`Machine::write_mem`, RP-7), pero para un rango arbitrario (el buffer
/// de la syscall 8).
fn validar_buffer_escribible(addr: u32, len: u32) -> Result<(), String> {
    let fin = addr.wrapping_add(len);
    let en = |base: u32, size: u32| addr >= base && fin <= base.wrapping_add(size);
    if en(TEXT_BASE, TEXT_SIZE) {
        return Err(format!(
            "el buffer (a0 = 0x{addr:08X}) apunta a .text y el código es \
             inmutable. Reservá el buffer en .data (p. ej. «.space {len}») o \
             pedilo al heap con el servicio 9"
        ));
    }
    if en(KEYBOARD_BASE, KEYBOARD_SIZE) {
        return Err(format!(
            "el buffer (a0 = 0x{addr:08X}) apunta a la página del teclado \
             MMIO: el dispositivo la maneja solo"
        ));
    }
    let valida = en(DATA_REGION_BASE, DATA_REGION_SIZE)
        || en(STACK_TOP - STACK_SIZE, STACK_SIZE)
        || en(FRAMEBUFFER_BASE, FRAMEBUFFER_SIZE)
        || en(PALETTE_BASE, PALETTE_SIZE);
    if !valida {
        return Err(format!(
            "el buffer [0x{addr:08X}, 0x{fin:08X}) no cae completo en una \
             zona escribible (.data/heap, pila, framebuffer o paleta)"
        ));
    }
    Ok(())
}

/// Syscall 8 (ReadString, estilo RARS): lee una línea por el mismo canal
/// que la 5 y la guarda en a0 como cadena C: a lo sumo a1−1 bytes + NUL;
/// una línea más larga se TRUNCA sin error (al byte, no al carácter: un
/// UTF-8 multibyte puede quedar partido — semántica de bytes, documentada).
/// a1 ≤ 0 no escribe nada (ni pide entrada); a1 == 1 escribe solo el NUL.
/// Apartamiento de RARS (fgets) registrado en BITACORA RP10-F2: NO se
/// agrega el '\n' final — un salto de línea invisible dentro de la cadena
/// es trampa para principiantes.
fn leer_cadena(uc: &mut Unicorn<'_, MachData>) {
    let buf = uc.reg_read(A0).unwrap_or(0) as u32;
    let max = uc.reg_read(A1).unwrap_or(0) as u32 as i32;
    if max <= 0 {
        return;
    }
    if let Err(motivo) = validar_buffer_escribible(buf, max as u32) {
        fail(uc, format!("syscall 8 (leer cadena): {motivo}"));
        return;
    }
    // Mismo protocolo bloqueante que read_int (frame pendiente primero).
    super::machine::publicar_frame(uc);
    let _ = uc.get_data().out.send(ConsoleEvent::InputRequest);
    let received = uc.get_data().input.recv();
    match received {
        Ok(InputMsg::Line(line)) => {
            let mut bytes = line.into_bytes();
            bytes.truncate(max as usize - 1);
            bytes.push(0);
            if uc.mem_write(u64::from(buf), &bytes).is_err() {
                fail(
                    uc,
                    format!("syscall 8 (leer cadena): no se pudo escribir en 0x{buf:08X}"),
                );
            }
        }
        Ok(InputMsg::Cancel) | Err(_) => {
            // Stop pedido por la UI (o canal cerrado): pausa limpia.
            let _ = uc.emu_stop();
        }
    }
}

/// Syscall 9 (Sbrk, estilo RARS): reserva a0 bytes del heap y devuelve en
/// a0 la dirección del bloque. Puntero creciente desde `HEAP_BASE`
/// (0x1004_0000, canónico RARS), el siguiente bloque queda alineado a 4,
/// sin free (como RARS). Apartamientos didácticos registrados en BITACORA
/// RP10-F2: cantidad negativa y agotamiento NO devuelven -1 silencioso —
/// cortan con un error claro.
fn sbrk(uc: &mut Unicorn<'_, MachData>) {
    let pedido = uc.reg_read(A0).unwrap_or(0) as u32 as i32;
    if pedido < 0 {
        fail(
            uc,
            format!(
                "syscall 9 (sbrk): la cantidad es negativa ({pedido}). El heap \
                 de EmuRV solo crece (no hay free), igual que en RARS"
            ),
        );
        return;
    }
    let tope = DATA_REGION_BASE + DATA_REGION_SIZE;
    let actual = uc.get_data().heap_next;
    let libre = tope - actual;
    if pedido as u32 > libre {
        fail(
            uc,
            format!(
                "syscall 9 (sbrk): el heap se agotó: pediste {pedido} bytes y \
                 quedan {libre} (el heap va de 0x{HEAP_BASE:08X} a 0x{tope:08X})"
            ),
        );
        return;
    }
    // El próximo bloque arranca alineado a 4; el redondeo puede pasarse
    // del tope hasta 3 bytes — se recorta (no hay nada después del heap).
    let nuevo = actual.saturating_add((pedido as u32).div_ceil(4) * 4).min(tope);
    uc.get_data_mut().heap_next = nuevo;
    let _ = uc.reg_write(A0, u64::from(actual));
}

/// Syscalls 6/7 (RP12): leen una línea y la parsean como float (punto
/// decimal); reintento didáctico ante basura (patrón read_int). El single
/// se escribe NaN-BOXED en fa0 (espec D: un f32 en registro de 64 lleva
/// los 32 altos en unos — el panel y las ops .s dependen de eso).
fn read_float(uc: &mut Unicorn<'_, MachData>, single: bool) {
    super::machine::publicar_frame(uc);
    loop {
        let _ = uc.get_data().out.send(ConsoleEvent::InputRequest);
        let received = uc.get_data().input.recv();
        match received {
            Ok(InputMsg::Line(line)) => {
                let texto = line.trim();
                if single {
                    match texto.parse::<f32>() {
                        Ok(v) => {
                            let bits = 0xFFFF_FFFF_0000_0000u64 | u64::from(v.to_bits());
                            let _ = uc.reg_write(FA0, bits);
                            return;
                        }
                        Err(_) => avisar_float(uc, texto),
                    }
                } else {
                    match texto.parse::<f64>() {
                        Ok(v) => {
                            let _ = uc.reg_write(FA0, v.to_bits());
                            return;
                        }
                        Err(_) => avisar_float(uc, texto),
                    }
                }
            }
            Ok(InputMsg::Cancel) | Err(_) => {
                let _ = uc.emu_stop();
                return;
            }
        }
    }
}

fn avisar_float(uc: &mut Unicorn<'_, MachData>, texto: &str) {
    send_output(
        uc,
        format!("«{texto}» no es un número (usá punto decimal, ej. 1.5); probá de nuevo: "),
    );
}

/// Ancla del reloj de la syscall 30: par (instante monotónico, ms de época
/// Unix) capturado UNA sola vez por proceso, la primera vez que un programa
/// pide la hora.
///
/// ¿Por qué un ancla y no `SystemTime::now()` en cada llamada? `SystemTime`
/// puede saltar hacia atrás (ajuste NTP, cambio de hora manual). RARS usa
/// `System.currentTimeMillis()`, que sí salta; diferencia documentada con
/// RARS: EmuRV devuelve época_del_ancla + transcurrido monotónico, así el
/// valor es NO DECRECIENTE dentro de una sesión (clave para que medir un
/// delta con dos syscalls 30 nunca dé negativo).
///
/// Es un `static` y no un campo de `MachData` (machine.rs no se toca en
/// esta fase): el ancla queda compartida entre máquinas/tests del mismo
/// proceso, lo cual para la monotonía está bien e incluso es deseable
/// (ninguna máquina ve el reloj retroceder respecto de otra); el costo es
/// que un salto del reloj de pared posterior al ancla no se refleja, solo
/// desplaza el origen unos ms.
static ANCLA_RELOJ: OnceLock<(Instant, u128)> = OnceLock::new();

/// Milisegundos desde la época Unix, no decrecientes (ver `ANCLA_RELOJ`).
fn tiempo_ms() -> u64 {
    let (instante, epoch_ms) = ANCLA_RELOJ.get_or_init(|| {
        // Un reloj anterior a 1970 es un sistema mal configurado: se ancla
        // en 0 antes que entrar en pánico dentro del hook de Unicorn.
        let epoch = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        (Instant::now(), epoch)
    });
    (epoch_ms + instante.elapsed().as_millis()) as u64
}

/// Syscall 32: duerme `a0` milisegundos (sin signo), interrumpible.
///
/// Duerme en trozos de ≤10 ms chequeando `stop_request` entre trozos: un
/// «Detener» durante un sleep largo corta el sueño de inmediato, y el
/// `emu_stop` hace que el tramo (`run_slice`) retorne ya. SIN marcar
/// `runtime_error`: es una pausa de usuario, la misma semántica que la rama
/// Cancel de `read_int` (commands.rs decide mirando su stop_flag).
fn dormir(uc: &mut Unicorn<'_, MachData>) {
    // El hilo de emulación va a quedar dormido: publicar antes el frame de
    // video pendiente (mismo patrón que read_int), o una animación que
    // duerme entre cuadros no mostraría ningún cuadro hasta terminar.
    super::machine::publicar_frame(uc);
    let ms = uc.reg_read(A0).unwrap_or(0) as u32;
    // En Arc propio: el préstamo de `get_data()` no debe retener a `uc`
    // durante el bucle (y `emu_stop` lo necesita `&mut`).
    let stop = Arc::clone(&uc.get_data().stop_request);
    let fin = Instant::now() + Duration::from_millis(u64::from(ms));
    const TROZO: Duration = Duration::from_millis(10);
    loop {
        if stop.load(Ordering::SeqCst) {
            // Pausa de usuario: cortar ya, sin error.
            let _ = uc.emu_stop();
            return;
        }
        let restante = fin.saturating_duration_since(Instant::now());
        if restante.is_zero() {
            return;
        }
        std::thread::sleep(restante.min(TROZO));
    }
}
