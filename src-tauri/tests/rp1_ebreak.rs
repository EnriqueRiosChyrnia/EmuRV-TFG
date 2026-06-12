//! RP1 — Bug latente de `ebreak` (BITACORA ~L282, sesión 2026-06-11).
//!
//! Semántica deseada (RARS/gdb): `ebreak` PAUSA, la GUI muestra el estado,
//! y al reanudar la ejecución continúa en la instrucción INMEDIATAMENTE
//! SIGUIENTE al `ebreak`, sin saltear ninguna ni re-ejecutarlo.
//!
//! Fase 1: este test documenta el bug (test sonda). Mientras el bug exista,
//! FALLA mostrando los valores reales de pc/registros observados — esa
//! evidencia va a BITACORA. La suite restante debe seguir verde.

use emurv_lib::core::assembler::assemble;
use emurv_lib::core::machine::{Machine, StopReason};
use emurv_lib::core::syscalls::{ConsoleEvent, InputMsg};
use emurv_lib::core::TEXT_BASE;
use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

fn machine_for(src: &str) -> (Machine<'static>, Receiver<ConsoleEvent>, Sender<InputMsg>) {
    let program = assemble(src).expect("el fuente de prueba debe ensamblar");
    let (out_tx, out_rx) = channel();
    let (in_tx, in_rx) = channel();
    let m = Machine::new(program, out_tx, in_rx).expect("máquina");
    (m, out_rx, in_tx)
}

/// Guardián anticuelgue (patrón tec1_detener.rs): si la reanudación colgara,
/// el test falla con mensaje claro en vez de colgar la suite.
fn con_guardia<T, F>(nombre: &'static str, limite: Duration, f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(limite) {
        Ok(v) => v,
        Err(_) => panic!("{nombre}: la máquina no retornó en {limite:?}"),
    }
}

/// Programa del plan (PLAN-RP1 Fase 1). Direcciones (todas las li expanden
/// a 1 instrucción por ser inmediatos chicos):
///   0x0040_0000  li t0, 1
///   0x0040_0004  ebreak          ← dirección del ebreak
///   0x0040_0008  li t1, 2        ← acá debe quedar el pc tras la pausa
///   0x0040_000C  li t2, 3
///   0x0040_0010  li a7, 10
///   0x0040_0014  ecall
const SRC: &str = "
.text
  li t0, 1
  ebreak
  li t1, 2
  li t2, 3
  li a7, 10
  ecall
";

const DIR_EBREAK: u32 = TEXT_BASE + 4;

struct Observado {
    razon_pausa: StopReason,
    pc_en_pausa: u32,
    t0_en_pausa: u32,
    t1_en_pausa: u32,
    razon_final: StopReason,
    t1_final: u32,
    t2_final: u32,
}

#[test]
fn ebreak_pausa_y_reanuda_en_la_instruccion_siguiente() {
    let limite = Duration::from_secs(if cfg!(debug_assertions) { 10 } else { 2 });
    let obs = con_guardia("rp1 sonda", limite, || {
        let (mut m, _out, _in) = machine_for(SRC);

        // Corrida libre acotada: debe frenar en el ebreak, no terminar.
        let razon_pausa = m.run_slice(0, 1_000);
        let pc_en_pausa = m.pc().expect("pc legible en la pausa");
        let regs = m.registers().expect("registros legibles en la pausa");
        let (t0_en_pausa, t1_en_pausa) = (regs[5], regs[6]);

        // Reanudar por el camino real de commands.rs (continuación de Run).
        let razon_final = m.run_slice_continuation(0, 1_000);
        let regs = m.registers().expect("registros legibles al final");
        Observado {
            razon_pausa,
            pc_en_pausa,
            t0_en_pausa,
            t1_en_pausa,
            razon_final,
            t1_final: regs[6],
            t2_final: regs[7],
        }
    });

    // La pausa por ebreak no es fin de programa.
    assert!(
        !matches!(obs.razon_pausa, StopReason::Exited { .. }),
        "el ebreak no debe terminar el programa; razón observada: {:?}",
        obs.razon_pausa
    );
    // Lo previo al ebreak se ejecutó; lo posterior todavía no.
    assert_eq!(
        obs.t0_en_pausa, 1,
        "t0 debía valer 1 en la pausa (lo previo corrió)"
    );
    assert_eq!(
        obs.t1_en_pausa, 0,
        "t1 debía valer 0 en la pausa (lo posterior NO debe haberse ejecutado)"
    );
    // Convención RARS/gdb: el pc queda en la instrucción siguiente al ebreak.
    assert_eq!(
        obs.pc_en_pausa,
        DIR_EBREAK + 4,
        "pc tras la pausa debía ser 0x{:08X} (la instrucción siguiente al ebreak); \
         observado 0x{:08X}",
        DIR_EBREAK + 4,
        obs.pc_en_pausa
    );
    // La reanudación ejecuta TODO lo que falta, sin saltear instrucciones.
    assert!(
        matches!(obs.razon_final, StopReason::Exited { code: 0 }),
        "la reanudación debía terminar con Exited{{0}}; observado: {:?}",
        obs.razon_final
    );
    assert_eq!(
        obs.t1_final, 2,
        "t1 debía valer 2 al final (instrucción post-ebreak)"
    );
    assert_eq!(obs.t2_final, 3, "t2 debía valer 3 al final");
}

// ---------------------------------------------------------------------------
// Fase 4 (PLAN-RP1): cobertura de bordes del rescate de ebreak.
// Todos los casos usan run_slice/step (camino con count, donde el rescate
// aplica); la corrida libre run(0) queda fuera por limitación documentada.
// ---------------------------------------------------------------------------

/// Límite del guardián anticuelgue de los tests de Fase 4 (mismos valores
/// debug/release que el test sonda de Fase 1).
fn limite_guardia() -> Duration {
    Duration::from_secs(if cfg!(debug_assertions) { 10 } else { 2 })
}

/// Borde 1: el ebreak es la ÚLTIMA instrucción del programa. La pausa debe
/// reportarse igual (addr correcto, pc = addr+4) y la reanudación debe caer
/// en el RuntimeError didáctico de "siguió de largo" (la palabra 0x00000000
/// tras el ebreak no es EBREAK, así que el rescate NO re-aplica) — nunca en
/// otra EbreakPause, que sería el bucle eterno de pausas que el diseño evitó.
#[test]
fn ebreak_como_ultima_instruccion() {
    const SRC_FIN: &str = "
.text
  li t0, 1
  ebreak
";
    let dir_ebreak = TEXT_BASE + 4;
    let (razon_pausa, pc_en_pausa, razon_final) =
        con_guardia("rp1 último ebreak", limite_guardia(), move || {
            let (mut m, _out, _in) = machine_for(SRC_FIN);
            let razon_pausa = m.run_slice(0, 1_000);
            let pc_en_pausa = m.pc().expect("pc legible en la pausa");
            let razon_final = m.run_slice_continuation(0, 1_000);
            (razon_pausa, pc_en_pausa, razon_final)
        });

    assert_eq!(
        razon_pausa,
        StopReason::EbreakPause { addr: dir_ebreak },
        "el ebreak final debía pausar con su dirección"
    );
    assert_eq!(
        pc_en_pausa,
        dir_ebreak + 4,
        "pc tras la pausa debía quedar en la (inexistente) instrucción siguiente"
    );
    // Verificación explícita anti-bucle: reanudar NO debe producir otra pausa.
    assert!(
        !matches!(razon_final, StopReason::EbreakPause { .. }),
        "reanudar tras el ebreak final no debe dar otra EbreakPause (bucle \
         eterno de pausas); observado: {razon_final:?}"
    );
    match &razon_final {
        StopReason::RuntimeError { message } => assert!(
            message.contains("siguió de largo"),
            "el mensaje debía ser el didáctico de instrucción inválida \
             («siguió de largo»); observado: {message}"
        ),
        otra => panic!("reanudar debía dar RuntimeError, no {otra:?}"),
    }
}

/// Borde 2: dos ebreak consecutivos. Cada uno produce SU pausa (el pc tras
/// la primera queda exactamente sobre el segundo) y la segunda reanudación
/// termina el programa. Exactamente dos pausas: ni una se pierde ni sobra.
#[test]
fn dos_ebreak_consecutivos() {
    const SRC_DOBLE: &str = "
.text
  li t0, 1
  ebreak
  ebreak
  li t1, 2
  li a7, 10
  ecall
";
    let dir_eb1 = TEXT_BASE + 4;
    let dir_eb2 = TEXT_BASE + 8;
    let (razon1, pc1, razon2, pc2, t1_en_pausa2, razon_final, t1_final) =
        con_guardia("rp1 doble ebreak", limite_guardia(), move || {
            let (mut m, _out, _in) = machine_for(SRC_DOBLE);
            let razon1 = m.run_slice(0, 1_000);
            let pc1 = m.pc().expect("pc legible en la pausa 1");
            let razon2 = m.run_slice_continuation(0, 1_000);
            let pc2 = m.pc().expect("pc legible en la pausa 2");
            let t1_en_pausa2 = m.registers().expect("registros en pausa 2")[6];
            let razon_final = m.run_slice_continuation(0, 1_000);
            let t1_final = m.registers().expect("registros al final")[6];
            (
                razon1,
                pc1,
                razon2,
                pc2,
                t1_en_pausa2,
                razon_final,
                t1_final,
            )
        });

    assert_eq!(
        razon1,
        StopReason::EbreakPause { addr: dir_eb1 },
        "pausa 1 con la dirección del PRIMER ebreak"
    );
    assert_eq!(
        pc1, dir_eb2,
        "el pc tras la pausa 1 debía quedar sobre el SEGUNDO ebreak"
    );
    assert_eq!(
        razon2,
        StopReason::EbreakPause { addr: dir_eb2 },
        "pausa 2 con la dirección del SEGUNDO ebreak"
    );
    assert_eq!(
        pc2,
        dir_eb2 + 4,
        "pc tras la pausa 2 en la instrucción siguiente"
    );
    assert_eq!(t1_en_pausa2, 0, "t1 todavía sin ejecutar en la pausa 2");
    // La tercera corrida termina: hubo EXACTAMENTE dos pausas.
    assert_eq!(
        razon_final,
        StopReason::Exited { code: 0 },
        "tras las dos pausas el programa debía terminar (exactamente dos pausas)"
    );
    assert_eq!(t1_final, 2, "t1 debía valer 2 al final");
}

/// Borde 3: un ebreak dentro de un bucle de N=5 iteraciones pausa N veces,
/// siempre con la MISMA dirección, y al final el contador refleja que no se
/// salteó ni se repitió ninguna iteración (pausa visible una vez por pasada).
#[test]
fn ebreak_en_bucle_n_veces() {
    const SRC_BUCLE: &str = "
.text
  li t0, 0
  li t1, 5
bucle:
  addi t0, t0, 1
  ebreak
  bne t0, t1, bucle
  li a7, 10
  ecall
";
    let dir_ebreak = TEXT_BASE + 12; // li/li/addi ocupan 0/4/8; el ebreak va en 12.
    let (pausas, razon_final, t0_final) =
        con_guardia("rp1 ebreak en bucle", limite_guardia(), move || {
            let (mut m, _out, _in) = machine_for(SRC_BUCLE);
            // Bucle host del plan: la primera run_slice normal y después
            // continuaciones, juntando las pausas hasta que el programa
            // termine. Tope defensivo (el guardián cubre el cuelgue real).
            let mut razon = m.run_slice(0, 1_000);
            let mut pausas: Vec<u32> = Vec::new();
            let mut razon_final = None;
            for _ in 0..20 {
                match razon {
                    StopReason::EbreakPause { addr } => {
                        pausas.push(addr);
                        razon = m.run_slice_continuation(0, 1_000);
                    }
                    otra => {
                        razon_final = Some(otra);
                        break;
                    }
                }
            }
            let t0_final = m.registers().expect("registros al final")[5];
            (pausas, razon_final, t0_final)
        });

    assert_eq!(
        pausas.len(),
        5,
        "debía haber exactamente 5 pausas (una por iteración); addrs: {pausas:08X?}"
    );
    assert!(
        pausas.iter().all(|&a| a == dir_ebreak),
        "todas las pausas debían reportar la dirección del ebreak \
         0x{dir_ebreak:08X}; observado: {pausas:08X?}"
    );
    assert_eq!(
        razon_final,
        Some(StopReason::Exited { code: 0 }),
        "tras las 5 pausas el bucle debía terminar"
    );
    assert_eq!(
        t0_final, 5,
        "el contador debía valer 5 (ni iteración de más ni de menos)"
    );
}

/// Borde 4: step() sobre un ebreak. La semántica correcta es pausa visible
/// UNA sola vez: el paso que cae sobre el ebreak reporta EbreakPause (no
/// StepDone) dejando pc = dir+4 sin ejecutar lo posterior, y el paso
/// siguiente ejecuta la instrucción post-ebreak con normalidad.
#[test]
fn step_sobre_ebreak() {
    const SRC_STEP: &str = "
.text
  li t0, 1
  ebreak
  li t1, 2
  li a7, 10
  ecall
";
    let dir_ebreak = TEXT_BASE + 4;
    let (paso1, paso2, pc_tras_pausa, t1_en_pausa, paso3, t1_tras_paso3, razon_final) =
        con_guardia("rp1 step sobre ebreak", limite_guardia(), move || {
            let (mut m, _out, _in) = machine_for(SRC_STEP);
            let paso1 = m.step(); // ejecuta li t0,1
            let paso2 = m.step(); // cae sobre el ebreak
            let pc_tras_pausa = m.pc().expect("pc legible tras la pausa");
            let t1_en_pausa = m.registers().expect("registros tras la pausa")[6];
            let paso3 = m.step(); // ejecuta li t1,2
            let t1_tras_paso3 = m.registers().expect("registros tras paso 3")[6];
            // Steps restantes hasta el fin del programa (tope defensivo).
            let mut razon_final = None;
            for _ in 0..10 {
                match m.step() {
                    StopReason::StepDone => {}
                    otra => {
                        razon_final = Some(otra);
                        break;
                    }
                }
            }
            (
                paso1,
                paso2,
                pc_tras_pausa,
                t1_en_pausa,
                paso3,
                t1_tras_paso3,
                razon_final,
            )
        });

    assert_eq!(
        paso1,
        StopReason::StepDone,
        "el primer paso (li t0,1) es normal"
    );
    // El paso sobre el ebreak reporta la pausa, NO un StepDone que la oculte.
    assert_eq!(
        paso2,
        StopReason::EbreakPause { addr: dir_ebreak },
        "step sobre el ebreak debía reportar EbreakPause con su dirección"
    );
    assert_eq!(
        pc_tras_pausa,
        dir_ebreak + 4,
        "pc tras la pausa por step debía quedar en la instrucción siguiente"
    );
    assert_eq!(t1_en_pausa, 0, "t1 aún sin ejecutar tras la pausa por step");
    assert_eq!(
        paso3,
        StopReason::StepDone,
        "el paso siguiente ejecuta la instrucción post-ebreak sin re-pausar"
    );
    assert_eq!(
        t1_tras_paso3, 2,
        "t1 debía valer 2 tras el paso post-ebreak"
    );
    assert_eq!(
        razon_final,
        Some(StopReason::Exited { code: 0 }),
        "steppear hasta el final debía terminar el programa"
    );
}

/// Borde 5: ebreak y breakpoints de UI conviven sin pisarse, en ambos
/// órdenes, incluido el sub-caso fino del bp de UI EXACTAMENTE en
/// dir_ebreak+4 (donde queda el pc tras la pausa por ebreak).
#[test]
fn ebreak_convive_con_breakpoint_de_ui() {
    con_guardia("rp1 ebreak + bp de UI", limite_guardia(), || {
        // --- Caso A: bp de UI ANTES del ebreak -----------------------------
        const SRC_A: &str = "
.text
  li t0, 1
  li t1, 2
  li t2, 3
  ebreak
  li t3, 4
  li a7, 10
  ecall
";
        let dir_bp_a = TEXT_BASE + 4; // li t1,2
        let dir_eb_a = TEXT_BASE + 12; // ebreak
        let (mut m, _out, _in) = machine_for(SRC_A);
        m.set_breakpoints(HashSet::from([dir_bp_a])).expect("bp A");
        assert_eq!(
            m.run_slice(0, 1_000),
            StopReason::Breakpoint { addr: dir_bp_a },
            "caso A: primero frena el breakpoint de UI"
        );
        // Reanudar tras un bp reportado = orden Run nueva (camino de
        // commands.rs): run_slice suprime UNA vez el bp del pc de arranque.
        assert_eq!(
            m.run_slice(0, 1_000),
            StopReason::EbreakPause { addr: dir_eb_a },
            "caso A: después pausa el ebreak"
        );
        assert_eq!(
            m.run_slice(0, 1_000),
            StopReason::Exited { code: 0 },
            "caso A: la última reanudación termina"
        );
        assert_eq!(
            m.registers().expect("regs A")[28],
            4,
            "caso A: t3 ejecutado"
        );

        // --- Caso B: ebreak ANTES del bp de UI -----------------------------
        const SRC_B: &str = "
.text
  li t0, 1
  ebreak
  li t1, 2
  li t2, 3
  li a7, 10
  ecall
";
        let dir_eb_b = TEXT_BASE + 4; // ebreak
        let dir_bp_b = TEXT_BASE + 12; // li t2,3
        let (mut m, _out, _in) = machine_for(SRC_B);
        m.set_breakpoints(HashSet::from([dir_bp_b])).expect("bp B");
        assert_eq!(
            m.run_slice(0, 1_000),
            StopReason::EbreakPause { addr: dir_eb_b },
            "caso B: primero pausa el ebreak"
        );
        // El pc quedó en dir_eb_b+4, que NO tiene bp: run_slice no suprime
        // nada y el bp posterior frena con normalidad.
        assert_eq!(
            m.run_slice(0, 1_000),
            StopReason::Breakpoint { addr: dir_bp_b },
            "caso B: después frena el breakpoint de UI"
        );
        assert_eq!(
            m.run_slice(0, 1_000),
            StopReason::Exited { code: 0 },
            "caso B: la última reanudación termina"
        );
        assert_eq!(m.registers().expect("regs B")[7], 3, "caso B: t2 ejecutado");

        // --- Caso C (sub-caso fino): bp de UI EXACTAMENTE en dir_ebreak+4 --
        // Tras la pausa por ebreak el pc queda sobre la instrucción con bp.
        // Ese bp DEBE reportarse (la pausa previa fue por el ebreak, no por
        // él): se reanuda con run_slice_continuation, que NO suprime el bp
        // del pc de arranque. OJO: el camino real de commands.rs para una
        // orden Run nueva es run_slice (primer tramo CON supresión), que acá
        // pasaría el bp de largo — conflicto de semántica documentado en el
        // reporte de Fase 4, sin tocar producción.
        const SRC_C: &str = "
.text
  li t0, 1
  ebreak
  li t1, 2
  li a7, 10
  ecall
";
        let dir_eb_c = TEXT_BASE + 4; // ebreak
        let dir_bp_c = dir_eb_c + 4; // li t1,2 — exactamente donde cae el pc
        let (mut m, _out, _in) = machine_for(SRC_C);
        m.set_breakpoints(HashSet::from([dir_bp_c])).expect("bp C");
        assert_eq!(
            m.run_slice(0, 1_000),
            StopReason::EbreakPause { addr: dir_eb_c },
            "caso C: primero pausa el ebreak"
        );
        assert_eq!(
            m.run_slice_continuation(0, 1_000),
            StopReason::Breakpoint { addr: dir_bp_c },
            "caso C: el bp en dir_ebreak+4 DEBE reportarse tras la pausa"
        );
        assert_eq!(
            m.registers().expect("regs C en bp")[6],
            0,
            "caso C: la instrucción del bp NO debe haberse ejecutado aún"
        );
        // Reanudar tras el bp reportado sí suprime (orden Run nueva).
        assert_eq!(
            m.run_slice(0, 1_000),
            StopReason::Exited { code: 0 },
            "caso C: la reanudación final termina"
        );
        assert_eq!(m.registers().expect("regs C")[6], 2, "caso C: t1 ejecutado");
    });
}

/// 5C por el CAMINO REAL de la GUI (fix RP1-F4): tras una pausa por ebreak,
/// el usuario aprieta «Ejecutar» de nuevo y commands.rs emite una orden Run
/// NUEVA cuyo primer tramo es `run_slice` (con supresión del bp del pc de
/// arranque). Antes del fix, un bp de UI exactamente en dir_ebreak+4 se
/// suprimía ahí y jamás se reportaba. Con `last_bp`, la supresión del primer
/// tramo solo aplica si la última parada fue ESE bp ya reportado.
#[test]
fn bp_en_la_siguiente_al_ebreak_se_reporta_en_run_nuevo() {
    let obs = con_guardia("rp1 5C camino GUI", limite_guardia(), || {
        let (mut m, _out, _in) = machine_for(SRC);
        let mut bps = HashSet::new();
        bps.insert(DIR_EBREAK + 4);
        m.set_breakpoints(bps).expect("instalar bp");

        let r1 = m.run_slice(0, 1_000); // → pausa por ebreak
        let r2 = m.run_slice(0, 1_000); // orden Run NUEVA (primer tramo)
        let t1_en_bp = m.registers().expect("regs en bp")[6];
        let r3 = m.run_slice(0, 1_000); // reanudar desde el bp YA reportado
        let t1_final = m.registers().expect("regs finales")[6];
        (r1, r2, t1_en_bp, r3, t1_final)
    });

    assert_eq!(
        obs.0,
        StopReason::EbreakPause { addr: DIR_EBREAK },
        "primero la pausa por ebreak"
    );
    assert_eq!(
        obs.1,
        StopReason::Breakpoint {
            addr: DIR_EBREAK + 4
        },
        "el bp en la instrucción siguiente al ebreak DEBE reportarse aunque \
         el Run nuevo arranque con supresión (la pausa previa no fue ese bp)"
    );
    assert_eq!(obs.2, 0, "en el bp, t1 aún no se ejecutó");
    assert!(
        matches!(obs.3, StopReason::Exited { code: 0 }),
        "reanudar desde el bp reportado suprime UNA vez y termina; observado {:?}",
        obs.3
    );
    assert_eq!(obs.4, 2, "t1 ejecutado al final");
}
