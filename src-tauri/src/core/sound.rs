//! Dispositivo de SONIDO compartido entre el hilo de emulación y los comandos
//! Tauri (RP31). Espejo de `video::VideoShared`, pero por EVENTO en vez de por
//! frame: el video PUBLICA un frame que se CONSERVA (varios consumidores lo
//! repintan); el sonido ENCOLA tonos que se CONSUMEN (cada tono se emite una
//! sola vez). El hook MEM_WRITE de `SOUND_TRIGGER` (ver `machine.rs`, mecanismo
//! validado en `tests/spike_sonido.rs`) encola acá; el run loop drena la cola y
//! emite `emu://sonido { freq, dur_ms, wave }` por cada tono.
use std::collections::VecDeque;

/// Un tono pedido por el programa vía MMIO. Es el payload del evento
/// `emu://sonido` que consume el frontend para reproducirlo con WebAudio.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SoundEvent {
    /// Frecuencia en Hz.
    pub freq: u32,
    /// Duración en ms.
    pub dur_ms: u32,
    /// Forma de onda: 0=cuadrada, 1=senoidal, 2=triángulo, 3=sierra.
    pub wave: u32,
}

/// Estado de sonido compartido (espejo de `VideoShared`): una cola de tonos
/// pendientes de emitir + el flag de uso. Apagado por defecto (RF4): `in_use`
/// arranca en false y solo pasa a true cuando el programa dispara un tono, así
/// un programa que no usa sonido no enciende nada (igual que el panel de video
/// solo aparece con uso real de la VRAM).
#[derive(Default)]
pub struct SoundShared {
    /// Tonos encolados por el hook, pendientes de emitir a la UI.
    pub cola: VecDeque<SoundEvent>,
    /// El programa usó el dispositivo de sonido (disparó al menos un tono).
    pub in_use: bool,
}

impl SoundShared {
    /// Encola un tono y marca el dispositivo en uso. Lo llama el hook de
    /// `SOUND_TRIGGER` a través del `Arc<Mutex<…>>`.
    pub fn push(&mut self, ev: SoundEvent) {
        self.cola.push_back(ev);
        self.in_use = true;
    }

    /// Vacía la cola y apaga el uso (Reiniciar / sesión nueva): sin tonos
    /// fantasma de la corrida anterior, igual que el vaciado de la cola del
    /// teclado en `Machine::load`.
    pub fn limpiar(&mut self) {
        self.cola.clear();
        self.in_use = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arranca_apagado_y_sin_tonos() {
        let s = SoundShared::default();
        assert!(!s.in_use, "el sonido arranca apagado (RF4)");
        assert!(s.cola.is_empty());
    }

    #[test]
    fn push_encola_y_marca_en_uso() {
        let mut s = SoundShared::default();
        s.push(SoundEvent {
            freq: 440,
            dur_ms: 200,
            wave: 1,
        });
        assert!(s.in_use, "disparar un tono marca el dispositivo en uso");
        assert_eq!(s.cola.len(), 1);
        let ev = s.cola.front().unwrap();
        assert_eq!((ev.freq, ev.dur_ms, ev.wave), (440, 200, 1));
    }

    #[test]
    fn limpiar_vacia_la_cola_y_apaga() {
        let mut s = SoundShared::default();
        s.push(SoundEvent {
            freq: 440,
            dur_ms: 200,
            wave: 0,
        });
        s.limpiar();
        assert!(s.cola.is_empty(), "Reiniciar no deja tonos fantasma");
        assert!(!s.in_use);
    }
}
