//! RP6 Fase 1 — spike de viabilidad de udbserver (PoC RSP).
//!
//! Habla Remote Serial Protocol CRUDO por TCP desde el propio test, sin
//! gdb externo (patrón de spikes del proyecto: evidencia antes de
//! arquitectura). Modelo de ejecución verificado acá:
//! `udbserver(uc, port, 0)` BLOQUEA sirviendo al cliente hasta que este
//! manda continue/step; recién entonces el host corre `emu_start` libre —
//! el modo debug NO usa run_slice (es un modo de ejecución propio).

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::Machine;
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::TEXT_BASE;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::channel;
use std::time::Duration;

/// Puerto libre del SO (bind a :0 y soltar; ventana de carrera irrelevante
/// en un test local).
fn puerto_libre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind :0")
        .local_addr()
        .expect("addr")
        .port()
}

/// Cliente RSP mínimo: empaqueta $datos#ck, espera el ack «+» y devuelve
/// el payload de la respuesta.
struct Rsp {
    s: TcpStream,
}

impl Rsp {
    fn conectar(port: u16) -> Rsp {
        // udbserver levanta el listener dentro de la llamada bloqueante:
        // reintentar hasta que acepte (máx ~5 s).
        for _ in 0..100 {
            if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
                s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
                return Rsp { s };
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("no se pudo conectar al stub en :{port}");
    }

    fn enviar(&mut self, datos: &str) -> String {
        let ck: u8 = datos.bytes().fold(0u8, |a, b| a.wrapping_add(b));
        let paquete = format!("${datos}#{ck:02x}");
        self.s.write_all(paquete.as_bytes()).expect("write");
        // Leer hasta el cierre de la respuesta: «+$payload#xx».
        let mut crudo = Vec::new();
        let mut byte = [0u8; 1];
        // ack
        loop {
            self.s.read_exact(&mut byte).expect("ack");
            if byte[0] == b'+' || byte[0] == b'$' {
                break;
            }
        }
        if byte[0] == b'+' {
            loop {
                self.s.read_exact(&mut byte).expect("inicio");
                if byte[0] == b'$' {
                    break;
                }
            }
        }
        loop {
            self.s.read_exact(&mut byte).expect("cuerpo");
            if byte[0] == b'#' {
                break;
            }
            crudo.push(byte[0]);
        }
        let mut ck = [0u8; 2];
        self.s.read_exact(&mut ck).expect("checksum");
        let _ = self.s.write_all(b"+"); // ack nuestro
        // Descomprimir el run-length encoding del RSP: «X*c» = X repetida
        // (c-29) veces MÁS (la espec usa count+29 como carácter imprimible;
        // p. ej. «0* » = "0000").
        let mut datos = Vec::with_capacity(crudo.len());
        let mut i = 0usize;
        while i < crudo.len() {
            if crudo[i] == b'*' && i + 1 < crudo.len() && !datos.is_empty() {
                let n = crudo[i + 1].saturating_sub(29) as usize;
                let prev = *datos.last().unwrap();
                datos.extend(std::iter::repeat_n(prev, n));
                i += 2;
            } else {
                datos.push(crudo[i]);
                i += 1;
            }
        }
        String::from_utf8_lossy(&datos).into_owned()
    }
}

#[test]
fn spike_qsupported_g_y_m() {
    // li t0, 1 / li t1, 2 / exit — programa trivial conocido.
    let programa = assemble(".text\n  li t0, 1\n  li t1, 2\n  li a7, 10\n  ecall\n")
        .expect("ensambla");
    let primera_palabra =
        u32::from_le_bytes(programa.text[0..4].try_into().unwrap());

    let port = puerto_libre();
    let (informe_tx, informe_rx) = channel::<String>();

    // Cliente en hilo propio: udbserver bloquea el hilo de la máquina.
    let cliente = std::thread::spawn(move || {
        let mut rsp = Rsp::conectar(port);
        let mut informe = String::new();

        let r = rsp.enviar("qSupported:swbreak+");
        informe.push_str(&format!("qSupported → {r}\n"));
        assert!(!r.is_empty(), "qSupported debe responder algo");

        let r = rsp.enviar("?");
        informe.push_str(&format!("? → {r}\n"));
        assert!(r.starts_with('S') || r.starts_with('T'), "halt reason: {r}");

        // g: 33 registros RV32 (x0..x31 + pc) en hex little-endian.
        let g = rsp.enviar("g");
        informe.push_str(&format!("g → {} chars\n", g.len()));
        assert!(g.len() >= 33 * 8, "g corto: {} chars", g.len());
        let pc_hex = &g[32 * 8..33 * 8];
        let pc = u32::from_le_bytes([
            u8::from_str_radix(&pc_hex[0..2], 16).unwrap(),
            u8::from_str_radix(&pc_hex[2..4], 16).unwrap(),
            u8::from_str_radix(&pc_hex[4..6], 16).unwrap(),
            u8::from_str_radix(&pc_hex[6..8], 16).unwrap(),
        ]);
        assert_eq!(pc, TEXT_BASE, "pc inicial en el halt = entrada de .text");
        informe.push_str(&format!("pc = 0x{pc:08X}\n"));

        // m: primera palabra de .text.
        let m = rsp.enviar(&format!("m{TEXT_BASE:x},4"));
        let mut bytes = [0u8; 4];
        for i in 0..4 {
            bytes[i] = u8::from_str_radix(&m[i * 2..i * 2 + 2], 16).unwrap();
        }
        let leida = u32::from_le_bytes(bytes);
        assert_eq!(leida, primera_palabra, "m debe leer .text real");
        informe.push_str(&format!("m .text → 0x{leida:08X} ✓\n"));

        // Z0: breakpoint por software en la 3.ª instrucción; c: continue.
        let dir_bp = TEXT_BASE + 8;
        let r = rsp.enviar(&format!("Z0,{dir_bp:x},4"));
        informe.push_str(&format!("Z0 → {r}\n"));
        assert_eq!(r, "OK", "Z0 debe aceptar el breakpoint");
        let r = rsp.enviar("c");
        informe.push_str(&format!("c → {r}\n"));
        assert!(r.starts_with('S') || r.starts_with('T'), "stop por bp: {r}");
        // pc en el breakpoint:
        let g = rsp.enviar("g");
        let pc_hex = &g[32 * 8..33 * 8];
        let pc = u32::from_le_bytes([
            u8::from_str_radix(&pc_hex[0..2], 16).unwrap(),
            u8::from_str_radix(&pc_hex[2..4], 16).unwrap(),
            u8::from_str_radix(&pc_hex[4..6], 16).unwrap(),
            u8::from_str_radix(&pc_hex[6..8], 16).unwrap(),
        ]);
        assert_eq!(pc, dir_bp, "frenó exactamente en el Z0");
        informe.push_str(&format!("bp en 0x{pc:08X} ✓\n"));

        // quitar bp y seguir hasta el final (el programa sale solo;
        // udbserver reporta la desconexión o el kill — toleramos ambos).
        let r = rsp.enviar(&format!("z0,{dir_bp:x},4"));
        assert_eq!(r, "OK");
        let _ = rsp.s.write_all(b"$c#63"); // continue final, sin esperar
        informe_tx.send(informe).expect("informe");
    });

    // Hilo de la máquina (Unicorn no es Send: nace acá).
    let (out_tx, _out_rx) = channel::<ConsoleEvent>();
    let (_in_tx, in_rx) = channel::<InputMsg>();
    let mut m = Machine::new(programa, out_tx, in_rx).expect("máquina");
    let uc = m.unicorn_for_debug();
    // start_addr=0: el server arranca YA y bloquea hasta el primer
    // continue/step del cliente.
    emurv_lib::udbserver::udbserver(uc, port, 0).expect("udbserver");
    // El cliente mandó «c»: ahora la emulación corre libre; los hooks del
    // stub retoman el control en cada breakpoint.
    let _ = uc.emu_start(u64::from(TEXT_BASE), 0x1_0000_0000, 0, 0);

    let informe = informe_rx
        .recv_timeout(Duration::from_secs(20))
        .expect("el cliente no terminó");
    eprintln!("[RP6-F1 spike]\n{informe}");
    cliente.join().expect("join cliente");
}
