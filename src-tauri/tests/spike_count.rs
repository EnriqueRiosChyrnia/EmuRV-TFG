//! PoC A — Interrumpibilidad por `count` (spike previo a la entrada de teclado).
//!
//! BITACORA (~280) registra que `emu_start(pc, NEVER, timeout_µs, count=0)` NO
//! retorna jamás con un bucle apretado tipo `loop: j loop` (el timeout de
//! Unicorn no corta un TB que salta a sí mismo). Hipótesis del plan: con
//! `count > 0` (4º parámetro de `emu_start`) el tramo SÍ retorna al agotar
//! las instrucciones, lo que habilita un lazo de ejecución por tramos capaz
//! de atender entrada de teclado entre tramo y tramo.
//!
//! Estos PoC van contra Unicorn CRUDO (`unicorn_engine::Unicorn`), no contra
//! `Machine`: deciden el mecanismo antes de construir encima.
//!
//! Los umbrales de tiempo son laxos a propósito para tolerar build debug
//! (el core C de Unicorn se compila sin optimizar bajo el perfil debug).
//! La cifra seria sale de:
//! `cargo test --release --test spike_count -- --nocapture`

use std::sync::mpsc;
use std::time::{Duration, Instant};
use unicorn_engine::{uc_error, Arch, Mode, Prot, RegisterRISCV, Unicorn};

/// Base de código: la misma región .text que mapea machine.rs (TEXT_BASE).
const BASE: u64 = 0x0040_0000;
/// Una página de 4 KiB alcanza para estos programas de 1–2 instrucciones.
const PAGE: u64 = 0x1000;
/// Dirección "hasta" inalcanzable en RV32: el tramo nunca corta por `until`
/// (mismo truco NEVER de machine.rs).
const NEVER: u64 = 0x1_0000_0000;

// --- Codificaciones RV32I verificadas a mano contra la espec ---
//
// j .  =  JAL x0, 0 (J-type):
//   imm[20|10:1|11|19:12]=0…0 | rd=00000 | opcode=1101111
//   → 0x0000006F
const J_SELF: u32 = 0x0000_006F;
//
// addi x5, x5, 1 (I-type):
//   imm[11:0]=000000000001 | rs1=00101 | funct3=000 | rd=00101 | opcode=0010011
//   → 0000 0000 0001 0010 1000 0010 1001 0011 = 0x00128293
const ADDI_X5_1: u32 = 0x0012_8293;
//
// bne x5, x0, -4 (B-type, salta a la instrucción anterior):
//   offset −4 = 1_1111_1111_1100 (13 bits) → imm[12]=1, imm[11]=1,
//   imm[10:5]=111111, imm[4:1]=1110
//   imm[12]|imm[10:5]=1111111 | rs2=00000 | rs1=00101 | funct3=001(BNE)
//   | imm[4:1]=1110 | imm[11]=1 | opcode=1100011
//   → 1111 1110 0000 0010 1001 1110 1110 0011 = 0xFE029EE3
const BNE_X5_BACK: u32 = 0xFE02_9EE3;

/// Crea un Unicorn RV32 crudo (patrón de machine.rs: Arch::RISCV +
/// Mode::RISCV32) con `palabras` escritas en BASE como código.
fn uc_con_codigo(palabras: &[u32]) -> Unicorn<'static, ()> {
    let mut uc = Unicorn::new(Arch::RISCV, Mode::RISCV32).expect("crear Unicorn RV32");
    uc.mem_map(BASE, PAGE, Prot::READ | Prot::EXEC)
        .expect("mapear página de código");
    let mut bytes = Vec::with_capacity(palabras.len() * 4);
    for p in palabras {
        bytes.extend_from_slice(&p.to_le_bytes());
    }
    uc.mem_write(BASE, &bytes).expect("escribir código");
    uc
}

/// Corre `f` en un hilo aparte y espera el resultado a lo sumo `limite`.
/// Si la hipótesis falla y `emu_start` cuelga, el test FALLA con mensaje
/// claro en vez de colgar la suite (el hilo zombi muere con el proceso).
/// Unicorn no es Send, así que cada escenario construye su motor adentro
/// del hilo.
fn con_guardia<T, F>(nombre: &'static str, limite: Duration, f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        // Si el receptor ya abandonó (timeout), el send falla y da igual.
        let _ = tx.send(f());
    });
    match rx.recv_timeout(limite) {
        Ok(v) => v,
        Err(_) => panic!(
            "{nombre}: emu_start no retornó en {limite:?}; \
             la hipótesis de interrumpibilidad por count queda refutada"
        ),
    }
}

/// PoC 1: `count > 0` corta un bucle infinito apretado que el timeout no
/// corta. `j .` con count=10M y timeout=0 debe retornar Ok en poco tiempo.
#[test]
fn poc1_count_corta_bucle_infinito() {
    let (res, ms, pc) = con_guardia("PoC1", Duration::from_secs(10), || {
        let mut uc = uc_con_codigo(&[J_SELF]);
        let t0 = Instant::now();
        let res = uc.emu_start(BASE, NEVER, 0, 10_000_000);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let pc = uc.pc_read().expect("leer pc");
        (res, ms, pc)
    });
    eprintln!("PoC1: emu_start(j ., timeout=0, count=10M) → {res:?} en {ms:.1} ms (pc=0x{pc:08X})");
    assert_eq!(res, Ok(()), "count=10M debe retornar Ok sobre 'j .'");
    // `j .` salta siempre a sí mismo: el pc queda clavado en BASE.
    assert_eq!(pc, BASE, "tras 10M de 'j .' el pc debe seguir en BASE");
    // Umbral laxo para debug; en release son decenas de ms.
    assert!(ms < 8_000.0, "10M instrucciones tardaron {ms:.0} ms (>8 s)");
}

/// PoC 2: timeout>0 Y count>0 conviven. Con count=10M y timeout=5 s sobre
/// `j .`, el count debe ganar y retornar Ok mucho antes del timeout.
#[test]
fn poc2_timeout_y_count_conviven() {
    let (res, ms) = con_guardia("PoC2", Duration::from_secs(10), || {
        let mut uc = uc_con_codigo(&[J_SELF]);
        let t0 = Instant::now();
        // 5_000_000 µs = 5 s de timeout, count=10M: gana el count.
        let res = uc.emu_start(BASE, NEVER, 5_000_000, 10_000_000);
        (res, t0.elapsed().as_secs_f64() * 1000.0)
    });
    eprintln!("PoC2: emu_start(j ., timeout=5s, count=10M) → {res:?} en {ms:.1} ms");
    assert_eq!(res, Ok(()), "timeout y count combinados deben retornar Ok");
    assert!(
        ms < 8_000.0,
        "con count=10M no debería acercarse al timeout"
    );
}

/// Resultado del escenario de caché caliente sobre el bucle addi+bne.
struct CacheCaliente {
    x5_fria: u64,
    x5_caliente: u64,
    x5_count_distinto: u64,
    paso_directo_ok: bool,
    paso_tras_flush_ok: Option<bool>,
}

/// PoC 3a: con la caché JIT caliente (TBs ya traducidos de la corrida 1),
/// un nuevo emu_start con count sigue retornando sobre `j .`.
#[test]
fn poc3a_count_con_cache_caliente_bucle_apretado() {
    let (res, ms) = con_guardia("PoC3a", Duration::from_secs(10), || {
        let mut uc = uc_con_codigo(&[J_SELF]);
        // Corrida 1: calienta la caché (mismo escenario que PoC1).
        uc.emu_start(BASE, NEVER, 0, 10_000_000)
            .expect("corrida fría debe retornar");
        // Corrida 2 con TB cacheado: ¿respeta un count nuevo (y chico)?
        let t0 = Instant::now();
        let res = uc.emu_start(BASE, NEVER, 0, 1_000);
        (res, t0.elapsed().as_secs_f64() * 1000.0)
    });
    eprintln!("PoC3a: re-emu_start(j ., count=1000) con TB caliente → {res:?} en {ms:.3} ms");
    assert_eq!(res, Ok(()), "count debe respetarse con TB cacheado");
    assert!(ms < 5_000.0);
}

/// PoC 3b: exactitud del count con caché caliente, medida con un contador
/// real (addi x5,x5,1 ; bne x5,x0,-4: x5 cuenta las iteraciones, así que
/// instrucciones ejecutadas = 2·x5 y un count par N deja x5 = N/2).
/// También prueba el paso count=1 tras corridas largas (familia del bug
/// JIT v0.1 "Reiniciar→Paso": TBs viejos que no respetan count=1) y, si
/// falla, documenta si ctl_flush_tb() lo arregla.
#[test]
fn poc3b_count_exacto_con_cache_caliente() {
    let r = con_guardia("PoC3b", Duration::from_secs(20), || {
        let mut uc = uc_con_codigo(&[ADDI_X5_1, BNE_X5_BACK]);

        // Corrida fría: count=10M → 5M iteraciones esperadas.
        uc.emu_start(BASE, NEVER, 0, 10_000_000)
            .expect("corrida fría");
        let x5_fria = uc.reg_read(RegisterRISCV::X5).expect("leer x5");

        // Corrida caliente, mismo pc inicial, mismo count.
        uc.reg_write(RegisterRISCV::X5, 0).expect("reset x5");
        uc.emu_start(BASE, NEVER, 0, 10_000_000)
            .expect("corrida caliente");
        let x5_caliente = uc.reg_read(RegisterRISCV::X5).expect("leer x5");

        // Reset de pc → count DISTINTO (2M → 1M iteraciones). Nota: el
        // `begin` de emu_start ya reescribe el pc; el reg_write explícito
        // replica el "reset de pc" del plan.
        uc.reg_write(RegisterRISCV::X5, 0).expect("reset x5");
        uc.reg_write(RegisterRISCV::PC, BASE).expect("reset pc");
        uc.emu_start(BASE, NEVER, 0, 2_000_000)
            .expect("corrida con count distinto");
        let x5_count_distinto = uc.reg_read(RegisterRISCV::X5).expect("leer x5");

        // Paso de UNA instrucción con TBs calientes: debe ejecutar solo el
        // addi (x5: 0→1) y dejar el pc en BASE+4.
        let paso = |uc: &mut Unicorn<'static, ()>| -> Result<(u64, u64), uc_error> {
            uc.reg_write(RegisterRISCV::X5, 0)?;
            uc.emu_start(BASE, NEVER, 0, 1)?;
            Ok((uc.reg_read(RegisterRISCV::X5)?, uc.pc_read()?))
        };
        let (x5_paso, pc_paso) = paso(&mut uc).expect("paso count=1");
        let paso_directo_ok = x5_paso == 1 && pc_paso == BASE + 4;

        // Si el paso no respetó count=1, probar el remedio conocido del
        // bug JIT v0.1: flushear la caché de bloques traducidos.
        let paso_tras_flush_ok = if paso_directo_ok {
            None
        } else {
            uc.ctl_flush_tb().expect("ctl_flush_tb");
            let (x5_f, pc_f) = paso(&mut uc).expect("paso tras flush");
            Some(x5_f == 1 && pc_f == BASE + 4)
        };

        CacheCaliente {
            x5_fria,
            x5_caliente,
            x5_count_distinto,
            paso_directo_ok,
            paso_tras_flush_ok,
        }
    });

    eprintln!(
        "PoC3b: x5 fría={} (esperado 5_000_000), caliente={} (5_000_000), \
         count distinto={} (1_000_000)",
        r.x5_fria, r.x5_caliente, r.x5_count_distinto
    );
    eprintln!(
        "PoC3b: paso count=1 con caché caliente directo_ok={}, tras_flush={:?}",
        r.paso_directo_ok, r.paso_tras_flush_ok
    );

    assert_eq!(r.x5_fria, 5_000_000, "count frío debe ser exacto");
    assert_eq!(
        r.x5_caliente, 5_000_000,
        "count caliente debe seguir siendo exacto con TBs cacheados"
    );
    assert_eq!(
        r.x5_count_distinto, 1_000_000,
        "un count distinto tras reset de pc debe respetarse"
    );
    // El paso debe funcionar directo o, en el peor caso, tras flush; si ni
    // el flush lo arregla, el mecanismo no sirve para "Paso" y el PoC falla.
    assert!(
        r.paso_directo_ok || r.paso_tras_flush_ok == Some(true),
        "count=1 no se respeta ni siquiera tras ctl_flush_tb()"
    );
}

/// PoC 4: throughput con slicing. Ejecuta el bucle de trabajo real
/// (addi+bne) en tramos de count fijo, retomando desde el pc donde quedó,
/// y mide instr/s totales. En debug se reduce el volumen (el número
/// canónico sale de release con --nocapture).
#[test]
fn poc4_throughput_con_slicing() {
    // Release: 10 tramos × 10M = 100M instrucciones (lo que pide el plan).
    // Debug: 10 tramos × 2M = 20M para no empantanar la suite.
    let (tramos, count): (u64, usize) = if cfg!(debug_assertions) {
        (10, 2_000_000)
    } else {
        (10, 10_000_000)
    };
    let total = tramos * count as u64;

    let (x5, segs) = con_guardia("PoC4", Duration::from_secs(120), move || {
        let mut uc = uc_con_codigo(&[ADDI_X5_1, BNE_X5_BACK]);
        let t0 = Instant::now();
        let mut pc = BASE;
        for _ in 0..tramos {
            uc.emu_start(pc, NEVER, 0, count).expect("tramo");
            pc = uc.pc_read().expect("leer pc entre tramos");
        }
        let segs = t0.elapsed().as_secs_f64();
        (uc.reg_read(RegisterRISCV::X5).expect("leer x5"), segs)
    });

    let ips = total as f64 / segs;
    let perfil = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    eprintln!(
        "PoC4 [{perfil}]: {total} instr en {tramos} tramos de count={count} \
         → {segs:.3} s → {:.1} M instr/s (x5 final={x5})",
        ips / 1e6
    );
    // El bucle alterna addi,bne,addi,bne…: de `total` instrucciones, la
    // mitad exacta son addi → x5 == total/2 si el count fue exacto en
    // todos los tramos.
    assert_eq!(x5, total / 2, "el slicing por count debe ser exacto");
    // Umbral laxo apto debug; la cifra seria es la de release.
    assert!(
        ips > 500_000.0,
        "throughput sospechosamente bajo: {ips:.0} instr/s"
    );
}

/// PoC 5 (bonus): overhead por tramo con count chico (1000) sobre el bucle
/// apretado. Mide tramos/s para dimensionar cuántos cortes por segundo
/// tolera el lazo de teclado.
#[test]
fn poc5_overhead_count_chico() {
    let tramos: u32 = if cfg!(debug_assertions) { 300 } else { 2_000 };

    let segs = con_guardia("PoC5", Duration::from_secs(60), move || {
        let mut uc = uc_con_codigo(&[J_SELF]);
        // Calienta la caché para medir solo el overhead de re-entrada.
        uc.emu_start(BASE, NEVER, 0, 1_000).expect("calentar");
        let t0 = Instant::now();
        for _ in 0..tramos {
            uc.emu_start(BASE, NEVER, 0, 1_000).expect("tramo chico");
        }
        t0.elapsed().as_secs_f64()
    });

    let tramos_s = f64::from(tramos) / segs;
    let perfil = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    eprintln!(
        "PoC5 [{perfil}]: {tramos} tramos de count=1000 sobre 'j .' en {segs:.3} s \
         → {tramos_s:.0} tramos/s ({:.2} M instr/s)",
        f64::from(tramos) * 1_000.0 / segs / 1e6
    );
    // Para servir al lazo de teclado alcanza con que cada tramo cueste
    // mucho menos que un frame (~16 ms): umbral laxo de 50 tramos/s.
    assert!(
        tramos_s > 50.0,
        "overhead por tramo demasiado alto: {tramos_s:.1} tramos/s"
    );
}
