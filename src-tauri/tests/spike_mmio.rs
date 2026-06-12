//! PoC B — Hook MEM_READ para MMIO de teclado (spike, rama feature/teclado).
//!
//! Pregunta del spike: ¿unicorn-engine 2.1.5 permite el patrón MMIO estándar,
//! es decir, un hook MEM_READ que escribe el valor en memoria ANTES de que la
//! lectura del guest complete, de modo que el guest lea el valor fresco?
//!
//! Dispositivo bajo prueba (estilo RARS/MARS):
//!   * 0xFFFF_0000 RECEIVER_CONTROL — bit 0 = hay dato listo.
//!   * 0xFFFF_0004 RECEIVER_DATA    — leerlo consume el dato y limpia ready.
//!
//! Estos tests usan Unicorn CRUDO (no `Machine`): el spike decide el
//! mecanismo antes de integrarlo. Conclusiones medidas acá:
//!   * B1: el guest ve el valor que el hook escribió (no el viejo). Funciona.
//!   * B2: la semántica leer-consume (CONTROL→1, DATA→'h', CONTROL→0) se
//!     cumple verificando registros del guest tras `emu_start`.
//!   * B3: el hook ve `size` correcto (1 para lb, 4 para lw) y la semántica
//!     se mantiene con cualquiera de los dos.
//!   * B4: el hook se dispara igual con la caché JIT caliente (2.ª corrida).
//!   * B5: con el hook instalado pero la página MMIO no accedida, el bucle
//!     aritmético en otra página no se degrada de forma apreciable (cifras
//!     por eprintln; el número serio sale de correr en release).
//!   * B6 (mecanismo alternativo): el binding también expone `mmio_map`,
//!     cuyo callback de lectura DEVUELVE el valor directamente. Sirve, pero
//!     con `mmio_map` la página no tiene respaldo real de memoria: cualquier
//!     `mem_read` del host (p. ej. el visor de memoria de la UI) pasaría por
//!     el callback y consumiría el dato. Con hook + memoria real, el host
//!     lee sin disparar hooks: mejor para un emulador didáctico.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Instant;
use unicorn_engine::{Arch, HookType, Mode, Prot, Unicorn};

/// Base del código guest (página de 4 KB con permiso de ejecución).
const CODE_BASE: u64 = 0x0000_1000;
/// Página MMIO del teclado (4 KB, alineada como exige Unicorn).
const MMIO_BASE: u64 = 0xFFFF_0000;
/// RECEIVER_CONTROL: bit 0 = dato listo.
const CONTROL: u64 = MMIO_BASE;
/// RECEIVER_DATA: leerlo consume el dato.
const DATA: u64 = MMIO_BASE + 4;
/// Tamaño de página que mapeamos.
const PAGINA: u64 = 0x1000;
/// Dirección "hasta" inalcanzable en RV32 (mismo truco que machine.rs).
const NUNCA: u64 = 0x1_0000_0000;

/// `RegisterRISCV::X0 == 1`, por eso el +1 (mismo helper que machine.rs).
fn xreg(n: u8) -> i32 {
    i32::from(n) + 1
}

// ---------------------------------------------------------------------------
// Codificaciones RV32I usadas, verificadas a mano contra la especificación:
//
// lui t1, 0xFFFF0      → U-type: imm[31:12]=0xFFFF0, rd=x6 (00110), op=0110111
//                        0xFFFF0<<12 | 6<<7 | 0x37            = 0xFFFF0337
// lw  t0, 0(t1)        → I-type: imm=0, rs1=x6, f3=010, rd=x5, op=0000011
//                        0<<20 | 6<<15 | 2<<12 | 5<<7 | 0x03  = 0x00032283
// lw  t2, 4(t1)        → imm=4, rd=x7:
//                        4<<20 | 6<<15 | 2<<12 | 7<<7 | 0x03  = 0x00432383
// lw  t3, 0(t1)        → imm=0, rd=x28:
//                        0<<20 | 6<<15 | 2<<12 | 28<<7 | 0x03 = 0x00032E03
// lb  t4, 4(t1)        → I-type: imm=4, rs1=x6, f3=000, rd=x29, op=0000011
//                        4<<20 | 6<<15 | 0<<12 | 29<<7 | 0x03 = 0x00430E83
// addi t0, t0, 1       → I-type: imm=1, rs1=x5, f3=000, rd=x5, op=0010011
//                        1<<20 | 5<<15 | 0<<12 | 5<<7 | 0x13  = 0x00128293
// jal  x0, -4          → J-type: imm=-4 (imm[20]=1, imm[19:12]=0xFF,
//                        imm[11]=1, imm[10:1]=0b1111111110), rd=x0, op=1101111
//                                                            = 0xFFDFF06F
// ---------------------------------------------------------------------------
const LUI_T1_FFFF0: u32 = 0xFFFF_0337;
const LW_T0_0_T1: u32 = 0x0003_2283;
const LW_T2_4_T1: u32 = 0x0043_2383;
const LW_T3_0_T1: u32 = 0x0003_2E03;
const LB_T4_4_T1: u32 = 0x0043_0E83;
const ADDI_T0_T0_1: u32 = 0x0012_8293;
const JAL_X0_M4: u32 = 0xFFDF_F06F;

/// Estado del dispositivo de teclado compartido con el hook.
struct Teclado {
    /// Cola de bytes pendientes (lo que "tipeó" el usuario).
    cola: VecDeque<u8>,
    /// Registro de accesos vistos por el hook: (dirección, tamaño).
    eventos: Vec<(u64, usize)>,
}

/// Crea el motor RV32 y mapea código + página MMIO con respaldo real.
fn nueva_maquina() -> Unicorn<'static, ()> {
    let mut uc =
        Unicorn::new(Arch::RISCV, Mode::RISCV32).expect("no se pudo iniciar el emulador");
    uc.mem_map(CODE_BASE, PAGINA, Prot::READ | Prot::EXEC)
        .expect("no se pudo mapear la página de código");
    uc.mem_map(MMIO_BASE, PAGINA, Prot::READ | Prot::WRITE)
        .expect("no se pudo mapear la página MMIO");
    uc
}

/// Vuelca el programa (palabras little-endian) en `CODE_BASE`.
fn cargar(uc: &mut Unicorn<'_, ()>, palabras: &[u32]) {
    let bytes: Vec<u8> = palabras.iter().flat_map(|w| w.to_le_bytes()).collect();
    uc.mem_write(CODE_BASE, &bytes)
        .expect("no se pudo cargar el programa");
}

/// Instala el hook MEM_READ del teclado sobre [CONTROL, DATA+3] (inclusive).
///
/// EL punto del spike: el callback hace `uc.mem_write` con el valor fresco
/// ANTES de que la lectura del guest complete. Si el guest viera el valor
/// viejo, el patrón no serviría.
fn instalar_teclado(uc: &mut Unicorn<'static, ()>, iniciales: &[u8]) -> Rc<RefCell<Teclado>> {
    let estado = Rc::new(RefCell::new(Teclado {
        cola: iniciales.iter().copied().collect(),
        eventos: Vec::new(),
    }));
    let est = Rc::clone(&estado);
    uc.add_mem_hook(
        HookType::MEM_READ,
        CONTROL,
        DATA + 3,
        move |uc, _tipo, addr, size, _val| {
            let mut e = est.borrow_mut();
            e.eventos.push((addr, size));
            // Registro de palabra al que cae el acceso (lb puede llegar
            // desalineado dentro del registro; lo normalizamos).
            let registro = addr & !0x3;
            let valor: u32 = if registro == CONTROL {
                u32::from(!e.cola.is_empty())
            } else {
                // Leer DATA consume el dato (semántica RARS).
                u32::from(e.cola.pop_front().unwrap_or(0))
            };
            // Escribir el valor fresco ANTES de que la lectura complete.
            // mem_write desde la API de host no re-dispara hooks.
            uc.mem_write(registro, &valor.to_le_bytes())
                .expect("mem_write dentro del hook falló");
            true
        },
    )
    .expect("no se pudo instalar el hook MEM_READ");
    estado
}

/// PoC B1 — ¿el guest ve el valor que escribió el hook, o el viejo?
/// Se pre-carga CONTROL con basura (0xDEAD_BEEF); el hook debe pisarla con 1
/// antes de que el `lw` complete.
#[test]
fn poc_b1_guest_ve_valor_fresco_escrito_por_el_hook() {
    let mut uc = nueva_maquina();
    // Basura previa: si el guest leyera "lo que había", t0 = 0xDEADBEEF.
    uc.mem_write(CONTROL, &0xDEAD_BEEFu32.to_le_bytes())
        .expect("no se pudo precargar CONTROL");
    let _estado = instalar_teclado(&mut uc, b"h");

    cargar(&mut uc, &[LUI_T1_FFFF0, LW_T0_0_T1]);
    uc.emu_start(CODE_BASE, CODE_BASE + 8, 0, 0)
        .expect("la emulación falló");

    let t0 = uc.reg_read(xreg(5)).expect("no se pudo leer t0");
    assert_eq!(
        t0, 1,
        "el guest debía ver el valor fresco del hook (1), no el viejo (0xDEADBEEF): t0={t0:#x}"
    );
}

/// PoC B2 — semántica leer-consume verificada en registros del guest:
/// lw CONTROL → 1, lw DATA → 'h', lw CONTROL → 0.
#[test]
fn poc_b2_semantica_leer_consume() {
    let mut uc = nueva_maquina();
    let estado = instalar_teclado(&mut uc, b"h");

    cargar(&mut uc, &[LUI_T1_FFFF0, LW_T0_0_T1, LW_T2_4_T1, LW_T3_0_T1]);
    uc.emu_start(CODE_BASE, CODE_BASE + 16, 0, 0)
        .expect("la emulación falló");

    let t0 = uc.reg_read(xreg(5)).expect("leer t0");
    let t2 = uc.reg_read(xreg(7)).expect("leer t2");
    let t3 = uc.reg_read(xreg(28)).expect("leer t3");
    assert_eq!(t0, 1, "1.ª lectura de CONTROL debía dar 1 (dato listo)");
    assert_eq!(t2, u64::from(b'h'), "lectura de DATA debía dar 'h'");
    assert_eq!(t3, 0, "2.ª lectura de CONTROL debía dar 0 (dato consumido)");
    assert!(
        estado.borrow().cola.is_empty(),
        "la cola debía quedar vacía tras consumir el dato"
    );
}

/// PoC B3 — lb vs lw sobre DATA: el hook debe ver el `size` correcto
/// (1 y 4) y la semántica de consumo se mantiene con ambos anchos.
#[test]
fn poc_b3_lb_y_lw_sobre_data() {
    let mut uc = nueva_maquina();
    let estado = instalar_teclado(&mut uc, b"ho");

    // lb consume 'h', lw consume 'o', CONTROL queda en 0.
    cargar(
        &mut uc,
        &[LUI_T1_FFFF0, LW_T0_0_T1, LB_T4_4_T1, LW_T2_4_T1, LW_T3_0_T1],
    );
    uc.emu_start(CODE_BASE, CODE_BASE + 20, 0, 0)
        .expect("la emulación falló");

    let t0 = uc.reg_read(xreg(5)).expect("leer t0");
    let t4 = uc.reg_read(xreg(29)).expect("leer t4");
    let t2 = uc.reg_read(xreg(7)).expect("leer t2");
    let t3 = uc.reg_read(xreg(28)).expect("leer t3");
    assert_eq!(t0, 1, "CONTROL debía dar 1 con dos datos en cola");
    assert_eq!(t4, u64::from(b'h'), "lb sobre DATA debía dar 'h'");
    assert_eq!(t2, u64::from(b'o'), "lw sobre DATA debía dar 'o'");
    assert_eq!(t3, 0, "CONTROL debía dar 0 tras consumir ambos datos");

    // El hook debe haber visto los tamaños reales de cada acceso.
    let eventos = estado.borrow().eventos.clone();
    assert_eq!(
        eventos,
        vec![(CONTROL, 4), (DATA, 1), (DATA, 4), (CONTROL, 4)],
        "el hook debía ver (dirección, tamaño) de cada acceso, incluido size=1 del lb"
    );
}

/// PoC B4 — ¿el hook sigue disparándose con la caché JIT caliente?
/// Misma secuencia de código ejecutada dos veces sin tocar los hooks.
#[test]
fn poc_b4_hook_dispara_con_jit_caliente() {
    let mut uc = nueva_maquina();
    let estado = instalar_teclado(&mut uc, b"h");
    cargar(&mut uc, &[LUI_T1_FFFF0, LW_T0_0_T1, LW_T2_4_T1, LW_T3_0_T1]);

    // 1.ª corrida: traduce los bloques (JIT frío).
    uc.emu_start(CODE_BASE, CODE_BASE + 16, 0, 0)
        .expect("la 1.ª corrida falló");
    assert_eq!(
        estado.borrow().eventos.len(),
        3,
        "la 1.ª corrida debía registrar 3 accesos MMIO"
    );

    // Re-sembrar el dispositivo y limpiar registros del guest.
    estado.borrow_mut().cola.push_back(b'x');
    for reg in [5u8, 7, 28] {
        uc.reg_write(xreg(reg), 0).expect("limpiar registro");
    }

    // 2.ª corrida: mismo código, bloques ya traducidos (JIT caliente).
    uc.emu_start(CODE_BASE, CODE_BASE + 16, 0, 0)
        .expect("la 2.ª corrida falló");

    assert_eq!(
        estado.borrow().eventos.len(),
        6,
        "con la caché JIT caliente el hook debía disparar igual (3 accesos más)"
    );
    let t0 = uc.reg_read(xreg(5)).expect("leer t0");
    let t2 = uc.reg_read(xreg(7)).expect("leer t2");
    let t3 = uc.reg_read(xreg(28)).expect("leer t3");
    assert_eq!(t0, 1, "2.ª corrida: CONTROL debía dar 1");
    assert_eq!(t2, u64::from(b'x'), "2.ª corrida: DATA debía dar 'x'");
    assert_eq!(t3, 0, "2.ª corrida: CONTROL final debía dar 0");
}

/// Mide instr/s de un bucle aritmético (addi + jal) que NO toca la página
/// MMIO: corre con timeout fijo y se cuentan iteraciones en t0 (2 instr por
/// iteración). Sin `count` para no meter el contador interno de Unicorn.
fn medir_ips(uc: &mut Unicorn<'_, ()>) -> f64 {
    // Calentamiento corto: deja los bloques traducidos en la caché JIT.
    uc.reg_write(xreg(5), 0).expect("limpiar t0");
    uc.emu_start(CODE_BASE, NUNCA, 50_000, 0)
        .expect("calentamiento falló");
    // Medición: 300 ms de pared (timeout de Unicorn en microsegundos).
    uc.reg_write(xreg(5), 0).expect("limpiar t0");
    let inicio = Instant::now();
    uc.emu_start(CODE_BASE, NUNCA, 300_000, 0)
        .expect("medición falló");
    let segundos = inicio.elapsed().as_secs_f64();
    let iteraciones = uc.reg_read(xreg(5)).expect("leer t0");
    (iteraciones * 2) as f64 / segundos
}

/// PoC B5 — costo del hook instalado pero NO accedido: bucle aritmético en
/// otra página, con y sin hook. Cifras por eprintln (la seria es la de
/// release con --nocapture); el umbral es laxo para tolerar builds debug.
#[test]
fn poc_b5_costo_hook_sin_acceso_mmio() {
    let programa = [ADDI_T0_T0_1, JAL_X0_M4];

    // Línea base: sin hook instalado.
    let mut uc_sin = nueva_maquina();
    cargar(&mut uc_sin, &programa);
    let ips_sin = medir_ips(&mut uc_sin);

    // Con hook instalado sobre la página MMIO, que el bucle nunca toca.
    let mut uc_con = nueva_maquina();
    let _estado = instalar_teclado(&mut uc_con, b"");
    cargar(&mut uc_con, &programa);
    let ips_con = medir_ips(&mut uc_con);

    eprintln!("PoC B5: sin hook = {ips_sin:.0} instr/s");
    eprintln!("PoC B5: con hook = {ips_con:.0} instr/s");
    eprintln!("PoC B5: relación con/sin = {:.3}", ips_con / ips_sin);
    assert!(
        _estado.borrow().eventos.is_empty(),
        "el bucle no debía tocar la página MMIO"
    );

    // Umbrales laxos (build debug tolerado): piso absoluto de 1 M instr/s y
    // el hook no puede costar más de 5x cuando ni se accede a su página.
    assert!(ips_sin > 1.0e6, "línea base sospechosamente lenta: {ips_sin:.0} instr/s");
    assert!(
        ips_con > ips_sin * 0.2,
        "el hook sin accesos degradó demasiado: {ips_con:.0} vs {ips_sin:.0} instr/s"
    );
}

/// PoC B6 — mecanismo alternativo del binding: `mmio_map`, cuyo callback de
/// lectura DEVUELVE el valor (no hace falta mem_write). Documentado como
/// plan alternativo; ver la nota del módulo sobre por qué se prefiere el
/// hook con memoria real para este proyecto.
#[test]
fn poc_b6_alternativa_mmio_map_devuelve_valor_directo() {
    let mut uc =
        Unicorn::new(Arch::RISCV, Mode::RISCV32).expect("no se pudo iniciar el emulador");
    uc.mem_map(CODE_BASE, PAGINA, Prot::READ | Prot::EXEC)
        .expect("no se pudo mapear la página de código");

    let cola: Rc<RefCell<VecDeque<u8>>> = Rc::new(RefCell::new(VecDeque::from(vec![b'h'])));
    let c = Rc::clone(&cola);
    // El callback recibe el OFFSET dentro del mapeo, no la dirección absoluta.
    uc.mmio_map_ro(MMIO_BASE, PAGINA, move |_uc, offset, _size| {
        let mut q = c.borrow_mut();
        match offset {
            0 => u64::from(!q.is_empty()),            // CONTROL
            4 => u64::from(q.pop_front().unwrap_or(0)), // DATA (consume)
            _ => 0,
        }
    })
    .expect("no se pudo mapear la región MMIO por callback");

    cargar(&mut uc, &[LUI_T1_FFFF0, LW_T0_0_T1, LW_T2_4_T1, LW_T3_0_T1]);
    uc.emu_start(CODE_BASE, CODE_BASE + 16, 0, 0)
        .expect("la emulación falló");

    let t0 = uc.reg_read(xreg(5)).expect("leer t0");
    let t2 = uc.reg_read(xreg(7)).expect("leer t2");
    let t3 = uc.reg_read(xreg(28)).expect("leer t3");
    assert_eq!(t0, 1, "mmio_map: CONTROL debía dar 1");
    assert_eq!(t2, u64::from(b'h'), "mmio_map: DATA debía dar 'h'");
    assert_eq!(t3, 0, "mmio_map: CONTROL debía dar 0 tras consumir");
}
