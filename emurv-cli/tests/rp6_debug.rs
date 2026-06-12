//! RP6 Fase 2 — `emurv-cli debug` como proceso real, protocolo RSP
//! verificado sin gdb externo (el test ES el cliente).

mod rsp_common;
use rsp_common::{puerto_libre, Rsp};

use emurv_lib::core::TEXT_BASE;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn cli_debug(archivo: &str, port: u16) -> Child {
    Command::new(env!("CARGO_BIN_EXE_emurv-cli"))
        .args(["debug", archivo, "--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("lanzar emurv-cli debug")
}

/// Espera al hijo con tope; lo mata si se pasa (guardián anticuelgue).
fn esperar(mut hijo: Child, limite: Duration) -> std::process::Output {
    let inicio = Instant::now();
    loop {
        match hijo.try_wait().expect("try_wait") {
            Some(_) => return hijo.wait_with_output().expect("output"),
            None if inicio.elapsed() > limite => {
                let _ = hijo.kill();
                panic!("emurv-cli debug no terminó en {limite:?}");
            }
            None => std::thread::sleep(Duration::from_millis(30)),
        }
    }
}

fn limite() -> Duration {
    Duration::from_secs(if cfg!(debug_assertions) { 60 } else { 25 })
}

fn carpeta_con(nombre: &str, fuente: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rp6_{nombre}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("carpeta");
    let ruta = dir.join("prog.s");
    std::fs::write(&ruta, fuente).expect("escribir fuente");
    ruta
}

/// (a)+(b) steps coherentes con la semántica conocida + lectura de .data.
#[test]
fn debug_steps_y_memoria() {
    let fuente = "
.data
dato: .word 0xCAFEBABE
.text
  li t0, 5
  li t1, 7
  li a7, 10
  ecall
";
    let ruta = carpeta_con("steps", fuente);
    let port = puerto_libre();
    let hijo = cli_debug(ruta.to_str().unwrap(), port);

    let mut rsp = Rsp::conectar(port);
    let _ = rsp.enviar("qSupported:swbreak+");

    // Dos steps: t0=5 y t1=7 (li chicos = 1 instrucción cada uno).
    let r = rsp.enviar("s");
    assert!(r.starts_with('S') || r.starts_with('T'), "step 1: {r}");
    let r = rsp.enviar("s");
    assert!(r.starts_with('S') || r.starts_with('T'), "step 2: {r}");
    let g = rsp.enviar("g");
    assert_eq!(Rsp::reg_de_g(&g, 5), 5, "t0 tras 2 steps");
    assert_eq!(Rsp::reg_de_g(&g, 6), 7, "t1 tras 2 steps");
    assert_eq!(Rsp::reg_de_g(&g, 32), TEXT_BASE + 8, "pc tras 2 steps");

    // m: .data conocida (DATA_BASE del proyecto).
    let m = rsp.enviar(&format!("m{:x},4", emurv_lib::core::DATA_BASE));
    assert_eq!(m.to_lowercase(), "bebafeca", ".word 0xCAFEBABE little-endian");

    // continuar hasta el final: el proceso debe terminar con exit 0.
    let _ = rsp.s.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = std::io::Write::write_all(&mut rsp.s, b"$c#63");
    let out = esperar(hijo, limite());
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
}

/// (c)+(d) Z0 detiene EXACTO en la dirección de una etiqueta (calculada con
/// Program.symbols ensamblando el mismo fuente acá) y z0 lo libera.
#[test]
fn debug_breakpoint_en_etiqueta() {
    let fuente = "
.text
  li t0, 1
  li t1, 2
meta:
  li t2, 3
  li a7, 10
  ecall
";
    let ruta = carpeta_con("bp", fuente);
    let programa = emurv_lib::core::assembler::assemble(fuente).expect("ensambla");
    let dir_meta = programa.symbols["meta"];

    let port = puerto_libre();
    let hijo = cli_debug(ruta.to_str().unwrap(), port);
    let mut rsp = Rsp::conectar(port);
    let _ = rsp.enviar("qSupported:swbreak+");

    assert_eq!(rsp.enviar(&format!("Z0,{dir_meta:x},4")), "OK");
    let r = rsp.enviar("c");
    assert!(r.starts_with('S') || r.starts_with('T'), "stop: {r}");
    let g = rsp.enviar("g");
    assert_eq!(Rsp::reg_de_g(&g, 32), dir_meta, "pc clavado en «meta»");
    assert_eq!(Rsp::reg_de_g(&g, 5), 1, "t0 ya ejecutado");
    assert_eq!(Rsp::reg_de_g(&g, 7), 0, "t2 todavía no");

    // quitar y correr hasta el final.
    assert_eq!(rsp.enviar(&format!("z0,{dir_meta:x},4")), "OK");
    let _ = rsp.s.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = std::io::Write::write_all(&mut rsp.s, b"$c#63");
    let out = esperar(hijo, limite());
    assert_eq!(out.status.code(), Some(0));
}

/// (e) Syscalls bajo el depurador: la salida del invitado llega a stdout
/// del proceso debug; los mensajes del stub van por stderr.
#[test]
fn debug_syscalls_imprimen_en_stdout() {
    let fuente = "
.data
msg: .asciiz \"hola desde gdb\\n\"
.text
  la a0, msg
  li a7, 4
  ecall
  li a7, 10
  ecall
";
    let ruta = carpeta_con("syscalls", fuente);
    let port = puerto_libre();
    let hijo = cli_debug(ruta.to_str().unwrap(), port);
    let mut rsp = Rsp::conectar(port);
    let _ = rsp.enviar("qSupported:swbreak+");
    let _ = rsp.s.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = std::io::Write::write_all(&mut rsp.s, b"$c#63");
    let out = esperar(hijo, limite());
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "hola desde gdb\n",
        "stdout EXCLUSIVO del invitado"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("esperando conexión gdb"), "stderr: {stderr}");
}

/// Puerto ocupado → error claro, exit 1 (sin pánico).
#[test]
fn debug_puerto_ocupado_sale_1() {
    let fuente = ".text\n  li a7, 10\n  ecall\n";
    let ruta = carpeta_con("puerto", fuente);
    let ocupante = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = ocupante.local_addr().unwrap().port();
    let hijo = cli_debug(ruta.to_str().unwrap(), port);
    let out = esperar(hijo, limite());
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no se pudo usar el puerto"), "{stderr}");
    assert!(!stderr.contains("panic"), "{stderr}");
    drop(ocupante);
}

// ---------------------------------------------------------------------------
// RP6 Fase 3 — convivencia con EmuRV real.
// ---------------------------------------------------------------------------

/// Teclado MMIO bajo el stub: el hook MEM_READ sigue vivo. El host lee
/// CONTROL con `m` (página con respaldo: NO consume) y el guest, al hacer
/// lw, ve 0 (cola vacía en headless).
#[test]
fn debug_convive_con_teclado_mmio() {
    let fuente = "
.text
  li t1, 0xFFFF0000
  lw t0, 0(t1)
  li a7, 10
  ecall
";
    let ruta = carpeta_con("mmio", fuente);
    let port = puerto_libre();
    let hijo = cli_debug(ruta.to_str().unwrap(), port);
    let mut rsp = Rsp::conectar(port);
    let _ = rsp.enviar("qSupported:swbreak+");

    // Lectura de host ANTES de que el guest toque la página.
    let m = rsp.enviar("mffff0000,4");
    assert_eq!(m, "00000000", "CONTROL=0 visto por el host: {m}");

    // step ×2: li (lui) + lw — el hook MEM_READ del teclado atiende el lw.
    let _ = rsp.enviar("s");
    let _ = rsp.enviar("s");
    let g = rsp.enviar("g");
    assert_eq!(Rsp::reg_de_g(&g, 5), 0, "t0 = CONTROL = 0 (cola vacía)");

    let _ = rsp.s.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = std::io::Write::write_all(&mut rsp.s, b"$c#63");
    let out = esperar(hijo, limite());
    assert_eq!(out.status.code(), Some(0));
}

/// Multi-archivo (RP-5) bajo el stub: breakpoint en la dirección de una
/// etiqueta definida DENTRO de un include (saludar, en utiles.s).
#[test]
fn debug_multiarchivo_breakpoint_en_etiqueta_de_include() {
    let main = "../ejemplos/11-multiarchivo/main.s";
    let fuente = std::fs::read_to_string(main).expect("leer main.s");
    let base = std::path::Path::new(main).parent();
    let programa =
        emurv_lib::core::assembler::assemble_with_includes(&fuente, base, "main.s")
            .expect("ensambla");
    let dir_saludar = programa.symbols["saludar"];

    let port = puerto_libre();
    let hijo = cli_debug(main, port);
    let mut rsp = Rsp::conectar(port);
    let _ = rsp.enviar("qSupported:swbreak+");

    assert_eq!(rsp.enviar(&format!("Z0,{dir_saludar:x},4")), "OK");
    // saludar se llama 3 veces (VECES de constantes.s): 3 paradas.
    for i in 1..=3 {
        let r = rsp.enviar("c");
        assert!(r.starts_with('S') || r.starts_with('T'), "parada {i}: {r}");
        let g = rsp.enviar("g");
        assert_eq!(
            Rsp::reg_de_g(&g, 32),
            dir_saludar,
            "parada {i} clavada en saludar (etiqueta del include)"
        );
    }
    assert_eq!(rsp.enviar(&format!("z0,{dir_saludar:x},4")), "OK");
    let _ = rsp.s.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = std::io::Write::write_all(&mut rsp.s, b"$c#63");
    let out = esperar(hijo, limite());
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "¡Hola desde varios archivos!\n¡Hola desde varios archivos!\n¡Hola desde varios archivos!\nListo.\n"
    );
}

/// RP11: el stub gdb funciona IGUAL sobre un ELF importado — breakpoint
/// en la dirección de un símbolo que viene del .symtab del binario.
#[test]
fn debug_sobre_elf_importado_breakpoint_por_simbolo() {
    let fuente = "
.text
  li t0, 1
  li t1, 2
meta:
  li t2, 3
  li a7, 10
  ecall
";
    // Exportar el ELF (round-trip) y quedarnos SOLO con el binario.
    let programa = emurv_lib::core::assembler::assemble(fuente).expect("ensambla");
    let bytes = emurv_lib::core::elf_export::export_elf(&programa).expect("exporta");
    let dir = std::env::temp_dir().join("rp11_debug_elf");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("carpeta");
    let ruta = dir.join("prog.elf");
    std::fs::write(&ruta, &bytes).expect("escribir elf");

    // El símbolo sale del .symtab IMPORTADO (no del fuente): el contrato
    // del plan RP11 («breakpoint por símbolo del .symtab importado»).
    let importado = emurv_lib::core::elf_import::import_elf(&bytes).expect("importa");
    let dir_meta = importado.symbols["meta"];

    let port = puerto_libre();
    let hijo = cli_debug(ruta.to_str().unwrap(), port);
    let mut rsp = Rsp::conectar(port);
    let _ = rsp.enviar("qSupported:swbreak+");

    assert_eq!(rsp.enviar(&format!("Z0,{dir_meta:x},4")), "OK");
    let r = rsp.enviar("c");
    assert!(r.starts_with('S') || r.starts_with('T'), "stop: {r}");
    let g = rsp.enviar("g");
    assert_eq!(Rsp::reg_de_g(&g, 32), dir_meta, "pc clavado en «meta»");
    assert_eq!(Rsp::reg_de_g(&g, 5), 1, "t0 ya ejecutado");
    assert_eq!(Rsp::reg_de_g(&g, 7), 0, "t2 todavía no");

    assert_eq!(rsp.enviar(&format!("z0,{dir_meta:x},4")), "OK");
    let _ = rsp.s.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = std::io::Write::write_all(&mut rsp.s, b"$c#63");
    let out = esperar(hijo, limite());
    assert_eq!(out.status.code(), Some(0));
}
