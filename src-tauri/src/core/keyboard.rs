//! Cola de teclas del dispositivo de teclado MMIO (TEC2).
//!
//! La UI EMPUJA códigos de tecla acá (comando `push_key`, desde su propio
//! hilo) y el hook MEM_READ de la máquina los CONSUME cuando el programa
//! lee RECEIVER_DATA (ver machine.rs). Por eso vive detrás de
//! `Arc<Mutex<…>>`, espejo del patrón de `VideoShared`.

use std::collections::VecDeque;

/// Capacidad máxima de la cola de teclas. Con el programa leyendo por
/// encuesta alcanza de sobra; si el usuario tipea más rápido de lo que el
/// programa consume, se pierden las teclas MÁS VIEJAS (ver `push`).
const QUEUE_CAP: usize = 64;

/// Cola de teclas pendientes, compartida entre la UI y el hilo de
/// emulación. Acotada a `QUEUE_CAP`: nunca crece sin límite ni entra en
/// pánico por llenarse.
pub struct KeyboardShared {
    /// Códigos pendientes en orden de llegada (el frente es el más viejo).
    cola: VecDeque<u8>,
}

impl Default for KeyboardShared {
    fn default() -> Self {
        Self {
            cola: VecDeque::with_capacity(QUEUE_CAP),
        }
    }
}

impl KeyboardShared {
    /// Encola una tecla. Si la cola está llena descarta la MÁS VIEJA para
    /// hacer lugar (política de RARS para entrada no consumida: lo último
    /// tipeado es lo que importa), sin pánico ni error.
    pub fn push(&mut self, code: u8) {
        if self.cola.len() >= QUEUE_CAP {
            self.cola.pop_front();
        }
        self.cola.push_back(code);
    }

    /// Saca y devuelve la tecla más vieja, o `None` si no hay ninguna.
    pub fn pop(&mut self) -> Option<u8> {
        self.cola.pop_front()
    }

    /// ¿Hay al menos una tecla pendiente? (bit 0 de RECEIVER_CONTROL).
    pub fn hay_dato(&self) -> bool {
        !self.cola.is_empty()
    }

    /// Vacía la cola (Reiniciar / sesión nueva: sin teclas fantasma).
    pub fn limpiar(&mut self) {
        self.cola.clear();
    }
}
