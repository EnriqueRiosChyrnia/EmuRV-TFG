//! emurv-cli — modo headless de EmuRV (RP-3 del roadmap).
//!
//! Subcomandos: `asm` (ensambla y exporta ELF32) y `run` (ensambla y ejecuta
//! con la consola puenteada a stdin/stdout). Reglas de canal:
//! - stdout es EXCLUSIVAMENTE la salida del programa invitado (corrección
//!   automática por diff depende de esto);
//! - TODOS los diagnósticos (errores, avisos, uso) van por stderr.
//!
//! Códigos de salida del proceso (diseño acordado en PLAN-RP3):
//! 0 = el programa invitado terminó · 1 = error de runtime o de E/S ·
//! 2 = error de ensamblado · 3 = uso inválido · 124 = límite agotado
//! (`--max-instr`/`--timeout-s`, convención de timeout(1)).
//!
//! El parsing de argumentos es manual con `std::env` (regla CLAUDE.md de
//! dependencias: `clap` no se justifica para dos subcomandos y cuatro flags).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USO: &str = "\
emurv-cli — EmuRV sin interfaz gráfica (RV32IM, syscalls estilo RARS)

USO:
  emurv-cli asm <entrada.s> [-o <salida.elf>]
      Ensambla el fuente y exporta un ELF32 RISC-V estándar.
      Sin -o, la salida es la entrada con extensión .elf.

  emurv-cli run <entrada.s> [--max-instr N] [--timeout-s S]
      Ensambla y ejecuta. La consola del programa usa stdin/stdout:
      stdout lleva SOLO la salida del programa; los diagnósticos van
      por stderr. --max-instr y --timeout-s acotan la corrida
      (al agotarse: código de salida 124).

  emurv-cli debug <entrada.s> [--port N]
      Ensambla y espera un cliente GDB (protocolo gdbserver) en el
      puerto dado (3333 por defecto). Conectar con:
      gdb → target remote 127.0.0.1:N. stdout sigue siendo del programa.

  emurv-cli --help | --version

CÓDIGOS DE SALIDA:
  0 programa terminado · 1 error de runtime o de E/S · 2 error de
  ensamblado · 3 uso inválido · 124 límite agotado";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = ejecutar_cli(&args);
    ExitCode::from(code)
}

fn ejecutar_cli(args: &[String]) -> u8 {
    match args.first().map(String::as_str) {
        Some("--help" | "-h") => {
            eprintln!("{USO}");
            0
        }
        Some("--version" | "-V") => {
            eprintln!("emurv-cli {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some("asm") => subcomando_asm(&args[1..]),
        Some("run") => subcomando_run(&args[1..]),
        Some("debug") => subcomando_debug(&args[1..]),
        Some(otro) => {
            eprintln!("emurv-cli: subcomando desconocido «{otro}»\n\n{USO}");
            3
        }
        None => {
            eprintln!("emurv-cli: falta el subcomando\n\n{USO}");
            3
        }
    }
}

/// `asm <entrada.s> [-o <salida.elf>]`
fn subcomando_asm(args: &[String]) -> u8 {
    let mut entrada: Option<PathBuf> = None;
    let mut salida: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" => match it.next() {
                Some(p) => salida = Some(PathBuf::from(p)),
                None => {
                    eprintln!("emurv-cli asm: -o requiere una ruta de salida\n\n{USO}");
                    return 3;
                }
            },
            _ if entrada.is_none() => entrada = Some(PathBuf::from(a)),
            otro => {
                eprintln!("emurv-cli asm: argumento inesperado «{otro}»\n\n{USO}");
                return 3;
            }
        }
    }
    let Some(entrada) = entrada else {
        eprintln!("emurv-cli asm: falta el archivo de entrada\n\n{USO}");
        return 3;
    };
    let salida = salida.unwrap_or_else(|| entrada.with_extension("elf"));

    let programa = match ensamblar_archivo(&entrada) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let bytes = match emurv_lib::core::elf_export::export_elf(&programa) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("emurv-cli asm: no se pudo generar el ELF: {e}");
            return 1;
        }
    };
    if let Err(e) = std::fs::write(&salida, bytes) {
        eprintln!(
            "emurv-cli asm: no se pudo escribir «{}»: {e}",
            salida.display()
        );
        return 1;
    }
    eprintln!("ELF escrito en «{}»", salida.display());
    0
}

/// `run <entrada.s> [--max-instr N] [--timeout-s S]`
///
/// Bucle de tramos idéntico al de producción (count + timeout de respaldo).
/// El puente de consola vive en un hilo propio: la máquina se BLOQUEA dentro
/// del tramo al esperar entrada (syscall 5), así que en un solo hilo el
/// proceso se trabaría — el hilo puente drena `out_rx` y alimenta `in_tx`
/// mientras el principal corre la máquina (espejo del modelo GUI).
fn subcomando_run(args: &[String]) -> u8 {
    // Espejo de RUN_SLICE_COUNT (privado en commands.rs; mismo criterio que
    // tests/tec1_detener.rs): ~2-3 ms por tramo en release.
    const SLICE: u64 = 1_000_000;
    const SLICE_MS: u64 = 100;

    let mut entrada: Option<PathBuf> = None;
    let mut max_instr: Option<u64> = None;
    let mut timeout_s: Option<u64> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--max-instr" => match it.next().map(|v| v.parse::<u64>()) {
                Some(Ok(n)) if n > 0 => max_instr = Some(n),
                _ => {
                    eprintln!("emurv-cli run: --max-instr requiere un entero positivo\n\n{USO}");
                    return 3;
                }
            },
            "--timeout-s" => match it.next().map(|v| v.parse::<u64>()) {
                Some(Ok(s)) if s > 0 => timeout_s = Some(s),
                _ => {
                    eprintln!("emurv-cli run: --timeout-s requiere un entero positivo\n\n{USO}");
                    return 3;
                }
            },
            _ if entrada.is_none() => entrada = Some(PathBuf::from(a)),
            otro => {
                eprintln!("emurv-cli run: argumento inesperado «{otro}»\n\n{USO}");
                return 3;
            }
        }
    }
    let Some(entrada) = entrada else {
        eprintln!("emurv-cli run: falta el archivo de entrada\n\n{USO}");
        return 3;
    };

    let programa = match ensamblar_archivo(&entrada) {
        Ok(p) => p,
        Err(code) => return code,
    };

    use emurv_lib::core::machine::{Machine, StopReason};
    use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    let (out_tx, out_rx) = channel::<ConsoleEvent>();
    let (in_tx, in_rx) = channel::<InputMsg>();
    let mut maquina = match Machine::new(programa, out_tx, in_rx) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("emurv-cli run: no se pudo crear la máquina: {e}");
            return 1;
        }
    };

    // Señales compartidas con el puente y el temporizador.
    let eof_stdin = Arc::new(AtomicBool::new(false));
    let en_stdin = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    maquina.set_stop_request(Arc::clone(&stop));

    // Temporizador de --timeout-s: setea stop (corta sleeps de syscall 32 en
    // ≤10 ms) Y manda Cancel (despierta un read_int bloqueado en syscall 5).
    if let Some(s) = timeout_s {
        let stop = Arc::clone(&stop);
        let cancel_tx = in_tx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(s));
            stop.store(true, Ordering::SeqCst);
            let _ = cancel_tx.send(InputMsg::Cancel);
        });
    }

    // Hilo puente: salida del invitado → stdout (sin saltos propios, con
    // flush por evento); InputRequest → una línea de stdin → Line, o EOF →
    // Cancel + marca para el diagnóstico del hilo principal.
    let puente = puente_consola(
        out_rx,
        in_tx.clone(),
        Arc::clone(&eof_stdin),
        Arc::clone(&en_stdin),
    );

    // Bucle de tramos. El presupuesto de --max-instr se descuenta por tramo
    // EMITIDO (cota superior: un tramo cortado antes —ebreak, pausa— cuenta
    // entero); precisión suficiente para acotar corridas, documentado.
    let inicio = Instant::now();
    let mut restante = max_instr;
    let mut primer_tramo = true;
    let codigo: u8 = loop {
        if stop.load(Ordering::SeqCst)
            || timeout_s.is_some_and(|s| inicio.elapsed() >= Duration::from_secs(s))
        {
            eprintln!("emurv-cli run: se agotó --timeout-s sin que el programa termine");
            break 124;
        }
        let tramo = match restante {
            Some(0) => {
                eprintln!("emurv-cli run: se agotó --max-instr sin que el programa termine");
                break 124;
            }
            Some(r) => r.min(SLICE),
            None => SLICE,
        };
        let razon = if primer_tramo {
            maquina.run_slice(SLICE_MS, tramo)
        } else {
            maquina.run_slice_continuation(SLICE_MS, tramo)
        };
        primer_tramo = false;
        if let Some(r) = restante.as_mut() {
            *r -= tramo;
        }
        match razon {
            StopReason::Exited { code } => break codigo_salida_invitado(code),
            StopReason::RuntimeError { message } => {
                eprintln!("{message}");
                break 1;
            }
            // Semántica headless (diseño acordado): sin UI que mostrar, las
            // pausas avisan por stderr y la ejecución continúa.
            StopReason::EbreakPause { addr } => {
                eprintln!("ebreak en 0x{addr:08X} — continuando");
            }
            StopReason::Breakpoint { addr } => {
                eprintln!("breakpoint en 0x{addr:08X} — continuando");
            }
            StopReason::Watchpoint { addr, value, .. } => {
                // El CLI no instala watchpoints; brazo defensivo (RP7).
                eprintln!(
                    "watchpoint en 0x{addr:08X} (valor 0x{value:08X}) — continuando"
                );
            }
            StopReason::Paused | StopReason::StepDone => {
                if eof_stdin.load(Ordering::SeqCst) {
                    eprintln!(
                        "emurv-cli run: el programa pedía entrada (syscall 5) pero stdin \
                         se agotó (EOF). Pasale la entrada por pipe o redirección."
                    );
                    break 1;
                }
                // Tramo terminado sin evento: seguir.
            }
        }
    };

    // Cerrar el canal de salida (drop de la máquina) hace terminar al puente
    // tras drenar lo pendiente. Si el puente quedó bloqueado leyendo stdin,
    // el canal está vacío (la máquina no produce salida mientras espera
    // entrada): no hay nada que drenar y se sale sin join.
    drop(maquina);
    if !en_stdin.load(Ordering::SeqCst) {
        let _ = puente.join();
    }
    codigo
}

/// `debug <entrada.s> [--port N]` — stub GDB (RP6, feature `gdb`).
///
/// Modelo de ejecución PROPIO (decidido por el spike de F1, BITACORA RP6):
/// `udbserver(uc, port, 0)` bloquea sirviendo al cliente hasta el primer
/// continue/step; después la emulación corre LIBRE con `machine.run(0)` y
/// los hooks del stub retoman el control en cada breakpoint/step. Acá NO
/// hay tramos por count: el depurador manda.
fn subcomando_debug(args: &[String]) -> u8 {
    let mut entrada: Option<PathBuf> = None;
    let mut port: u16 = 3333;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--port" => match it.next().map(|v| v.parse::<u16>()) {
                Some(Ok(p)) if p > 0 => port = p,
                _ => {
                    eprintln!("emurv-cli debug: --port requiere un puerto válido (1-65535)\n\n{USO}");
                    return 3;
                }
            },
            _ if entrada.is_none() => entrada = Some(PathBuf::from(a)),
            otro => {
                eprintln!("emurv-cli debug: argumento inesperado «{otro}»\n\n{USO}");
                return 3;
            }
        }
    }
    let Some(entrada) = entrada else {
        eprintln!("emurv-cli debug: falta el archivo de entrada\n\n{USO}");
        return 3;
    };

    let programa = match ensamblar_archivo(&entrada) {
        Ok(p) => p,
        Err(code) => return code,
    };

    use emurv_lib::core::machine::{Machine, StopReason};
    use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
    use std::sync::mpsc::channel;

    // udbserver con start_addr=0 PANIQUEA si el bind falla (expect interno):
    // pre-chequeo del puerto acá para dar un error claro. Queda una ventana
    // de carrera mínima entre soltar y re-bindear; aceptable en uso real.
    match std::net::TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => drop(l),
        Err(e) => {
            eprintln!("emurv-cli debug: no se pudo usar el puerto {port}: {e}");
            return 1;
        }
    }

    let (out_tx, out_rx) = channel::<ConsoleEvent>();
    let (in_tx, in_rx) = channel::<InputMsg>();
    let mut maquina = match Machine::new(programa, out_tx, in_rx) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("emurv-cli debug: no se pudo crear la máquina: {e}");
            return 1;
        }
    };
    let puente = puente_consola(
        out_rx,
        in_tx,
        std::sync::Arc::default(),
        std::sync::Arc::default(),
    );

    eprintln!("esperando conexión gdb en 127.0.0.1:{port} (target remote 127.0.0.1:{port})…");
    let uc = maquina.unicorn_for_debug();
    if let Err(e) = emurv_lib::udbserver::udbserver(uc, port, 0) {
        eprintln!("emurv-cli debug: el stub gdb falló: {e}");
        return 1;
    }
    eprintln!("cliente gdb conectado; la ejecución queda bajo control del depurador");

    // Corrida libre: breakpoints/steps del cliente retoman vía hooks. Si el
    // cliente se desconecta, el programa sigue hasta terminar (mensaje y
    // salida normales — no nos colgamos esperándolo).
    let razon = maquina.run(0);
    let codigo = match razon {
        StopReason::Exited { code } => {
            eprintln!("programa terminado bajo el depurador");
            codigo_salida_invitado(code)
        }
        StopReason::RuntimeError { message } => {
            eprintln!("{message}");
            1
        }
        otra => {
            eprintln!("sesión de depuración finalizada ({otra:?})");
            0
        }
    };
    drop(maquina);
    let _ = puente.join();
    codigo
}

/// Traduce el código de salida del invitado (syscall 93/10, i32 real) al
/// código de proceso del CLI (RP10, diseño acordado): primero `code & 0xFF`
/// (estilo Unix: -1 → 255), después tope en 125 — 126/127/128+ tienen
/// significado propio del shell (no ejecutable, no encontrado, señales) y
/// un exceso se reporta por stderr. Colisión documentada en README: si el
/// invitado elige 124, es indistinguible del timeout propio del CLI.
fn codigo_salida_invitado(code: i32) -> u8 {
    let c = (code & 0xFF) as u8;
    if c > 125 {
        eprintln!(
            "emurv-cli: el código de salida del programa ({code}) queda fuera del rango \
             seguro del shell tras «& 0xFF» ({c}); se acota a 125"
        );
        125
    } else {
        c
    }
}

/// Hilo puente de consola compartido por `run` y `debug`: salida del
/// invitado → stdout (sin saltos propios, flush por evento); InputRequest →
/// una línea de stdin → Line, o EOF → Cancel + marca. Termina solo cuando
/// el canal de salida se cierra (drop de la máquina).
fn puente_consola(
    out_rx: std::sync::mpsc::Receiver<emurv_lib::core::syscalls::ConsoleEvent>,
    in_tx: std::sync::mpsc::Sender<emurv_lib::core::syscalls::InputMsg>,
    eof_stdin: std::sync::Arc<std::sync::atomic::AtomicBool>,
    en_stdin: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> std::thread::JoinHandle<()> {
    use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
    use std::io::{BufRead, Write};
    use std::sync::atomic::Ordering;
    std::thread::spawn(move || {
        let mut stdout = std::io::stdout();
        let stdin = std::io::stdin();
        while let Ok(ev) = out_rx.recv() {
            match ev {
                ConsoleEvent::Output(s) => {
                    let _ = stdout.write_all(s.as_bytes());
                    let _ = stdout.flush();
                }
                ConsoleEvent::InputRequest => {
                    en_stdin.store(true, Ordering::SeqCst);
                    let mut linea = String::new();
                    let n = stdin.lock().read_line(&mut linea).unwrap_or(0);
                    en_stdin.store(false, Ordering::SeqCst);
                    if n == 0 {
                        eof_stdin.store(true, Ordering::SeqCst);
                        let _ = in_tx.send(InputMsg::Cancel);
                    } else {
                        let _ = in_tx.send(InputMsg::Line(
                            linea.trim_end_matches(['\n', '\r']).to_string(),
                        ));
                    }
                }
                ConsoleEvent::Clear => {
                    // Syscall 60 (RP10): la secuencia ANSI de limpiar SOLO
                    // si stdout es una terminal real — en pipe/CI no se
                    // ensucian los diffs (el stdout sigue siendo
                    // EXCLUSIVAMENTE la salida del invitado).
                    use std::io::IsTerminal;
                    if stdout.is_terminal() {
                        let _ = stdout.write_all(b"\x1B[2J\x1B[H");
                        let _ = stdout.flush();
                    }
                }
            }
        }
    })
}

/// Lee y ensambla un fuente. Errores de E/S → exit 1; de ensamblado → 2,
/// con el formato didáctico en español intacto (uno por línea, a stderr).
fn ensamblar_archivo(ruta: &Path) -> Result<emurv_lib::core::assembler::Program, u8> {
    let fuente = std::fs::read_to_string(ruta).map_err(|e| {
        eprintln!("emurv-cli: no se pudo leer «{}»: {e}", ruta.display());
        1u8
    })?;
    // RP5: los .include se resuelven relativos al directorio del archivo.
    let base = ruta.parent();
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "programa.s".to_string());
    emurv_lib::core::assembler::assemble_with_includes(&fuente, base, &nombre).map_err(|errores| {
        for err in &errores {
            eprintln!("{err}");
        }
        eprintln!(
            "emurv-cli: el ensamblado falló con {} error(es)",
            errores.len()
        );
        2u8
    })
}
