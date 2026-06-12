//! Cliente RSP mínimo compartido por el spike (F1) y los tests de
//! integración del subcomando debug (F2/F3). Protocolo crudo por TCP:
//! $datos#ck, ack «+», y descompresión del run-length encoding del RSP.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// Puerto libre del SO (bind a :0 y soltar).
pub fn puerto_libre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind :0")
        .local_addr()
        .expect("addr")
        .port()
}

pub struct Rsp {
    pub s: TcpStream,
}

impl Rsp {
    pub fn conectar(port: u16) -> Rsp {
        for _ in 0..200 {
            if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
                s.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
                return Rsp { s };
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("no se pudo conectar al stub en :{port}");
    }

    pub fn enviar(&mut self, datos: &str) -> String {
        let ck: u8 = datos.bytes().fold(0u8, |a, b| a.wrapping_add(b));
        let paquete = format!("${datos}#{ck:02x}");
        self.s.write_all(paquete.as_bytes()).expect("write");
        let mut crudo = Vec::new();
        let mut byte = [0u8; 1];
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
        let _ = self.s.write_all(b"+");
        // RSP run-length: «X*c» = X repetida (c-29) veces más.
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

    /// Registro N (0..32; 32 = pc) de una respuesta `g` ya descomprimida.
    pub fn reg_de_g(g: &str, n: usize) -> u32 {
        let h = &g[n * 8..n * 8 + 8];
        u32::from_le_bytes([
            u8::from_str_radix(&h[0..2], 16).unwrap(),
            u8::from_str_radix(&h[2..4], 16).unwrap(),
            u8::from_str_radix(&h[4..6], 16).unwrap(),
            u8::from_str_radix(&h[6..8], 16).unwrap(),
        ])
    }
}
