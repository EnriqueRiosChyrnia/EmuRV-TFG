//! Spike RP31-F1 — Dispositivo de SONIDO por MMIO (decisión de mecanismo).
//!
//! Pregunta del spike: ¿se puede dar a los programas un dispositivo de sonido
//! ESPEJANDO el modelo de video/teclado, sin pisar ninguna región existente,
//! y disparado por una escritura del guest a un registro de TRIGGER?
//!
//! El video PUBLICA pasivamente (el run loop relee la VRAM cada tramo). El
//! sonido NECESITA eventos DISCRETOS (cada disparo = un tono): un sondeo
//! pasivo perdería disparos o re-emitiría. El mecanismo elegido espeja al
//! TECLADO (tests/spike_mmio.rs, ya en producción en machine.rs): un hook
//! sobre la página MMIO con respaldo de memoria REAL. La diferencia es el
//! tipo de hook —MEM_WRITE en vez de MEM_READ— porque el disparo es una
//! ESCRITURA del guest a SOUND_TRIGGER.
//!
//! Conclusiones medidas acá (ver BITACORA RP31-F1):
//!   * S1: una escritura a SOUND_TRIGGER encola UN evento con la freq/dur/wave
//!     vigentes (los registros con respaldo real ya tienen el valor escrito).
//!   * S2: sin disparo NO se encola nada e `in_use` queda en false (apagado
//!     por defecto, RF4: solo suena si el programa usa el dispositivo).
//!   * S3: dos disparos encolan DOS eventos (cola drenable por el run loop).
//!   * S4: las regiones nuevas (sonido + sprites) son DISJUNTAS de video,
//!     paleta, control de video y teclado (no se pisa nada).
//!
//! Como tests/spike_mmio.rs, usa Unicorn CRUDO (no `Machine`): el spike fija
//! el mecanismo ANTES de integrarlo al núcleo (eso es la Fase 2).

use std::cell::RefCell;
use std::rc::Rc;
use unicorn_engine::{Arch, HookType, Mode, Prot, Unicorn};

/// Base del código guest (página de 4 KB con permiso de ejecución).
const CODE_BASE: u64 = 0x0000_1000;
/// Tamaño de página estándar de Unicorn.
const PAGINA: u64 = 0x1000;

// --- Layout MMIO de SONIDO propuesto por el spike (lejos de video/teclado) ---
/// Base de la página MMIO de sonido (RP31). Elegida en 0x9004_0000: dentro
/// de la franja de dispositivos 0x9000_xxxx (video vive en 0x9000–0x9003),
/// en la primera dirección libre, sin pisar nada.
const SOUND_BASE: u64 = 0x9004_0000;
/// Frecuencia del tono en Hz (u32).
const SOUND_FREQ: u64 = SOUND_BASE; // +0x00
/// Duración del tono en ms (u32).
const SOUND_DUR: u64 = SOUND_BASE + 0x04;
/// Forma de onda (u32): 0=cuadrada, 1=senoidal, 2=triángulo, 3=sierra.
const SOUND_WAVE: u64 = SOUND_BASE + 0x08;
/// Disparo: escribir cualquier valor emite un tono con freq/dur/wave actuales.
const SOUND_TRIGGER: u64 = SOUND_BASE + 0x0C;

// ---------------------------------------------------------------------------
// Codificaciones RV32I usadas, verificadas a mano contra la especificación:
//
// lui  t1, 0x90040  → U: (0x90040<<12) | (x6<<7) | 0x37        = 0x90040337
// addi t0, x0, imm  → I: (imm<<20) | (x0<<15) | (000<<12) | (x5<<7) | 0x13
//                       440→0x1B800293 · 200→0x0C800293 · 1→0x00100293
// sw   t0, off(t1)  → S: imm[11:5]<<25 | x5<<20 | x6<<15 | 010<<12
//                       | imm[4:0]<<7 | 0x23
//                       off 0→0x00532023 · 4→0x00532223 · 8→0x00532423
//                       · 12→0x00532623
// ---------------------------------------------------------------------------
const LUI_T1_SOUND: u32 = 0x9004_0337;
const ADDI_T0_440: u32 = 0x1B80_0293;
const ADDI_T0_200: u32 = 0x0C80_0293;
const ADDI_T0_1: u32 = 0x0010_0293;
const ADDI_T0_880: u32 = 0x3700_0293; // 880=0x370 → (0x370<<20)|0x280|0x13
const SW_T0_0_T1: u32 = 0x0053_2023; // sw t0, 0(t1)   → SOUND_FREQ
const SW_T0_4_T1: u32 = 0x0053_2223; // sw t0, 4(t1)   → SOUND_DUR
const SW_T0_8_T1: u32 = 0x0053_2423; // sw t0, 8(t1)   → SOUND_WAVE
const SW_T0_12_T1: u32 = 0x0053_2623; // sw t0, 12(t1)  → SOUND_TRIGGER

/// Evento de sonido capturado por el hook (espejo de lo que en Fase 2 será
/// `SoundShared`: una cola que el run loop drena y emite como `emu://sonido`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct EventoSonido {
    freq: u32,
    dur_ms: u32,
    wave: u32,
}

/// Estado del dispositivo de sonido compartido con el hook (modelo del spike).
struct Sonido {
    /// Eventos pendientes de emitir (cada disparo agrega uno).
    cola: Vec<EventoSonido>,
    /// El programa usó el dispositivo (igual que `VideoShared::in_use`).
    in_use: bool,
    /// Accesos vistos por el hook: (dirección, tamaño) — para introspección.
    eventos: Vec<(u64, usize)>,
}

/// Crea el motor RV32 y mapea código + página MMIO de sonido con respaldo real
/// (igual criterio que el teclado en machine.rs: las lecturas de host no
/// disparan hooks, así inspeccionar memoria NO tiene efectos colaterales).
fn nueva_maquina() -> Unicorn<'static, ()> {
    let mut uc = Unicorn::new(Arch::RISCV, Mode::RISCV32).expect("no se pudo iniciar el emulador");
    uc.mem_map(CODE_BASE, PAGINA, Prot::READ | Prot::EXEC)
        .expect("no se pudo mapear la página de código");
    uc.mem_map(SOUND_BASE, PAGINA, Prot::READ | Prot::WRITE)
        .expect("no se pudo mapear la página de sonido");
    uc
}

/// Vuelca el programa (palabras little-endian) en `CODE_BASE`.
fn cargar(uc: &mut Unicorn<'_, ()>, palabras: &[u32]) {
    let bytes: Vec<u8> = palabras.iter().flat_map(|w| w.to_le_bytes()).collect();
    uc.mem_write(CODE_BASE, &bytes)
        .expect("no se pudo cargar el programa");
}

/// Instala el hook MEM_WRITE del sonido sobre [SOUND_FREQ, SOUND_TRIGGER+3].
///
/// EL punto del spike: cuando el guest escribe en SOUND_TRIGGER, el hook lee
/// freq/dur/wave de memoria (ya escritos por las `sw` previas, que el respaldo
/// real conserva) y encola un evento. Las escrituras a freq/dur/wave también
/// pasan por el hook pero NO disparan: solo TRIGGER encola.
fn instalar_sonido(uc: &mut Unicorn<'static, ()>) -> Rc<RefCell<Sonido>> {
    let estado = Rc::new(RefCell::new(Sonido {
        cola: Vec::new(),
        in_use: false,
        eventos: Vec::new(),
    }));
    let est = Rc::clone(&estado);
    uc.add_mem_hook(
        HookType::MEM_WRITE,
        SOUND_FREQ,
        SOUND_TRIGGER + 3,
        move |uc, _tipo, addr, size, _val| {
            est.borrow_mut().eventos.push((addr, size));
            // Un sb/sh podría caer desalineado dentro del registro: se
            // normaliza a la palabra base (mismo truco que el teclado).
            let registro = addr & !0x3;
            if registro == SOUND_TRIGGER {
                // freq/dur/wave son contiguos (offset 0/4/8): un solo read de
                // 12 bytes los cubre. Ya están en memoria (respaldo real).
                let mut buf = [0u8; 12];
                uc.mem_read(SOUND_FREQ, &mut buf)
                    .expect("no se pudieron leer los registros de sonido");
                let freq = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                let dur_ms = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
                let wave = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
                let mut e = est.borrow_mut();
                e.cola.push(EventoSonido { freq, dur_ms, wave });
                e.in_use = true;
            }
            true
        },
    )
    .expect("no se pudo instalar el hook MEM_WRITE de sonido");
    estado
}

/// Spike S1 — una escritura a SOUND_TRIGGER encola UN evento con los valores
/// vigentes de frecuencia, duración y forma de onda.
#[test]
fn spike_s1_trigger_encola_evento_con_freq_dur_wave() {
    let mut uc = nueva_maquina();
    let estado = instalar_sonido(&mut uc);

    // freq=440, dur=200, wave=1(senoidal), disparo.
    cargar(
        &mut uc,
        &[
            LUI_T1_SOUND,
            ADDI_T0_440,
            SW_T0_0_T1,
            ADDI_T0_200,
            SW_T0_4_T1,
            ADDI_T0_1,
            SW_T0_8_T1,
            ADDI_T0_1,
            SW_T0_12_T1,
        ],
    );
    uc.emu_start(CODE_BASE, CODE_BASE + 36, 0, 0)
        .expect("la emulación falló");

    let s = estado.borrow();
    assert!(s.in_use, "el disparo debe marcar el dispositivo en uso (RF4)");
    assert_eq!(
        s.cola,
        vec![EventoSonido {
            freq: 440,
            dur_ms: 200,
            wave: 1,
        }],
        "el disparo debía encolar UN evento con freq/dur/wave vigentes"
    );
}

/// Spike S2 — escribir freq/dur/wave SIN disparar no encola nada y deja el
/// dispositivo apagado (apagado por defecto, como el video: RF4).
#[test]
fn spike_s2_sin_trigger_no_encola_ni_marca_en_uso() {
    let mut uc = nueva_maquina();
    let estado = instalar_sonido(&mut uc);

    // freq=440, dur=200, wave=1, pero NUNCA se escribe SOUND_TRIGGER.
    cargar(
        &mut uc,
        &[
            LUI_T1_SOUND,
            ADDI_T0_440,
            SW_T0_0_T1,
            ADDI_T0_200,
            SW_T0_4_T1,
            ADDI_T0_1,
            SW_T0_8_T1,
        ],
    );
    uc.emu_start(CODE_BASE, CODE_BASE + 28, 0, 0)
        .expect("la emulación falló");

    let s = estado.borrow();
    assert!(s.cola.is_empty(), "sin disparo no debe encolarse ningún tono");
    assert!(!s.in_use, "sin disparo el dispositivo queda apagado (RF4)");
    // El hook SÍ vio las 3 escrituras de configuración (no las ignora a nivel
    // de bus; simplemente no disparan).
    assert_eq!(s.eventos.len(), 3, "el hook debía ver las 3 escrituras de config");
}

/// Spike S3 — dos disparos encolan DOS eventos (la cola es drenable por el run
/// loop, que en Fase 2 emitirá un `emu://sonido` por cada uno).
#[test]
fn spike_s3_dos_disparos_encolan_dos_eventos() {
    let mut uc = nueva_maquina();
    let estado = instalar_sonido(&mut uc);

    // freq=440 → disparo; freq=880 → disparo (dur/wave quedan en 0).
    cargar(
        &mut uc,
        &[
            LUI_T1_SOUND,
            ADDI_T0_440,
            SW_T0_0_T1,
            ADDI_T0_1,
            SW_T0_12_T1, // 1.er disparo (freq=440)
            ADDI_T0_880,
            SW_T0_0_T1,
            ADDI_T0_1,
            SW_T0_12_T1, // 2.º disparo (freq=880)
        ],
    );
    uc.emu_start(CODE_BASE, CODE_BASE + 36, 0, 0)
        .expect("la emulación falló");

    let s = estado.borrow();
    assert_eq!(s.cola.len(), 2, "dos disparos debían encolar dos eventos");
    assert_eq!(s.cola[0].freq, 440);
    assert_eq!(s.cola[1].freq, 880);
}

/// Spike S4 — las regiones MMIO nuevas (sonido + sprites) son DISJUNTAS de las
/// existentes (framebuffer, paleta, control de video, teclado). Verificación
/// de layout: la regla dura es «no pisar video/teclado».
#[test]
fn spike_s4_regiones_nuevas_disjuntas_de_las_existentes() {
    // El layout de registros de sonido es el documentado (offsets 0/4/8/12).
    assert_eq!(SOUND_FREQ, SOUND_BASE, "freq en el offset 0");
    assert_eq!(SOUND_DUR, SOUND_BASE + 0x04, "dur en el offset 4");
    assert_eq!(SOUND_WAVE, SOUND_BASE + 0x08, "wave en el offset 8");
    assert_eq!(SOUND_TRIGGER, SOUND_BASE + 0x0C, "trigger en el offset 12");

    /// (base, tamaño, nombre).
    type Region = (u64, u64, &'static str);

    // Regiones EXISTENTES (constantes verificadas en core/mod.rs).
    let framebuffer: Region = (0x9000_0000, 0x0002_0000, "framebuffer");
    let palette: Region = (0x9002_0000, 0x0000_1000, "paleta");
    let video_control: Region = (0x9003_0000, 0x0000_1000, "control de video");
    let keyboard: Region = (0xFFFF_0000, 0x0000_1000, "teclado");

    // Regiones NUEVAS propuestas por el spike (sonido + sprites de la opción A).
    let sound: Region = (SOUND_BASE, 0x0000_1000, "sonido");
    let sprite_table: Region = (0x9005_0000, 0x0000_1000, "tabla de sprites");
    let sprite_pattern: Region = (0x9006_0000, 0x0000_4000, "patrones de sprites");

    let existentes = [framebuffer, palette, video_control, keyboard];
    let nuevas = [sound, sprite_table, sprite_pattern];

    let solapan = |a: Region, b: Region| a.0 < b.0 + b.1 && b.0 < a.0 + a.1;

    // Cada región nueva, disjunta de cada existente.
    for &n in &nuevas {
        for &e in &existentes {
            assert!(
                !solapan(n, e),
                "la región «{}» (0x{:08X}) pisa «{}» (0x{:08X})",
                n.2,
                n.0,
                e.2,
                e.0
            );
        }
    }
    // Las regiones nuevas tampoco se pisan entre sí.
    for (i, &a) in nuevas.iter().enumerate() {
        for &b in &nuevas[i + 1..] {
            assert!(
                !solapan(a, b),
                "la región nueva «{}» pisa «{}»",
                a.2,
                b.2
            );
        }
    }
}
