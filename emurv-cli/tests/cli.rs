//! RP3 Fase 1 — el CLI como proceso real (env! CARGO_BIN_EXE).
//! stdout del invitado y stderr de diagnósticos se verifican por separado.

use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_emurv-cli"))
}

/// (a) `asm` de 01-hola-mundo produce un ELF32 válido (magic + class).
#[test]
fn asm_genera_elf32() {
    let dir = std::env::temp_dir().join("rp3_asm_elf32");
    let _ = std::fs::create_dir_all(&dir);
    let salida = dir.join("hola.elf");
    let out = cli()
        .args([
            "asm",
            "../ejemplos/01-hola-mundo.asm",
            "-o",
            salida.to_str().unwrap(),
        ])
        .output()
        .expect("lanzar emurv-cli");
    assert!(
        out.status.success(),
        "asm debía salir con 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = std::fs::read(&salida).expect("leer el ELF generado");
    assert_eq!(&bytes[0..4], b"\x7fELF", "magic ELF");
    assert_eq!(bytes[4], 1, "EI_CLASS = ELFCLASS32 (32 bits)");
    let _ = std::fs::remove_file(&salida);
}

/// (b) Fuente con error de sintaxis → exit 2 y mensaje didáctico en
/// español con número de línea por stderr; stdout queda VACÍO.
#[test]
fn error_de_ensamblado_sale_2_con_mensaje_en_espanol() {
    let dir = std::env::temp_dir().join("rp3_asm_err");
    let _ = std::fs::create_dir_all(&dir);
    let fuente = dir.join("roto.s");
    std::fs::write(&fuente, ".text\n  addi x1, x2\n").expect("escribir fuente");
    let out = cli()
        .args(["asm", fuente.to_str().unwrap()])
        .output()
        .expect("lanzar emurv-cli");
    assert_eq!(out.status.code(), Some(2), "error de ensamblado = exit 2");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("línea 2") || stderr.contains("línea"),
        "stderr debía traer el diagnóstico didáctico con línea; fue: {stderr}"
    );
    assert!(out.stdout.is_empty(), "stdout debe quedar limpio");
    let _ = std::fs::remove_file(&fuente);
}

/// (c) Sin argumentos → exit 3 y texto de uso por stderr.
#[test]
fn sin_argumentos_sale_3_con_uso() {
    let out = cli().output().expect("lanzar emurv-cli");
    assert_eq!(out.status.code(), Some(3), "uso inválido = exit 3");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("USO"), "stderr debía traer el uso; fue: {stderr}");
    assert!(out.stdout.is_empty(), "stdout debe quedar limpio");
}

// ---------------------------------------------------------------------------
// RP3 Fase 2 — subcomando run: puente de consola y códigos de salida.
// ---------------------------------------------------------------------------

use std::io::Write;
use std::process::Stdio;
use std::time::Duration;

/// Corre el CLI con stdin dado y guardián anticuelgue propio (patrón
/// con_guardia de tec1): si el proceso no termina en `limite`, lo mata y
/// el test falla con mensaje claro.
fn run_con_stdin(args: &[&str], stdin_data: &str, limite: Duration) -> std::process::Output {
    let mut hijo = cli()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("lanzar emurv-cli");
    hijo.stdin
        .take()
        .expect("stdin del hijo")
        .write_all(stdin_data.as_bytes())
        .expect("escribir stdin");
    // (al soltar el handle, el hijo ve EOF tras consumir lo escrito)
    let inicio = std::time::Instant::now();
    loop {
        match hijo.try_wait().expect("try_wait") {
            Some(_) => break,
            None if inicio.elapsed() > limite => {
                let _ = hijo.kill();
                panic!("emurv-cli no terminó en {limite:?} (args: {args:?})");
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    hijo.wait_with_output().expect("recoger salida")
}

fn limite() -> Duration {
    Duration::from_secs(if cfg!(debug_assertions) { 60 } else { 20 })
}

/// (a) 01-hola-mundo: stdout EXACTO y exit 0 (stdout sin contaminación).
#[test]
fn run_hola_mundo_stdout_exacto() {
    let out = run_con_stdin(&["run", "../ejemplos/01-hola-mundo.asm"], "", limite());
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "¡Hola, mundo!\n",
        "stdout debe ser EXACTAMENTE la salida del programa"
    );
}

/// (b) 02-suma-interactiva con entrada por pipe.
#[test]
fn run_suma_interactiva_con_pipe() {
    let out = run_con_stdin(
        &["run", "../ejemplos/02-suma-interactiva.asm"],
        "8\n34\n",
        limite(),
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("La suma es: 42\n"),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// (c) 03, 04 y 05: salidas conocidas (mismas que los tests de integración).
#[test]
fn run_ejemplos_de_consola_restantes() {
    let out = run_con_stdin(&["run", "../ejemplos/03-contador.asm"], "", limite());
    assert_eq!(out.status.code(), Some(0));
    let esperado: String = (1..=10).map(|n| format!("{n}\n")).collect();
    assert_eq!(String::from_utf8_lossy(&out.stdout), esperado);

    let out = run_con_stdin(&["run", "../ejemplos/04-fibonacci.asm"], "", limite());
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "0 1 1 2 3 5 8 13 21 34 \n");

    let out = run_con_stdin(&["run", "../ejemplos/05-pila-funciones.asm"], "", limite());
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "5! = 120\n");
}

/// (d) EOF prematuro: 02 espera dos números y stdin trae uno.
#[test]
fn run_eof_prematuro_sale_1_con_diagnostico() {
    let out = run_con_stdin(
        &["run", "../ejemplos/02-suma-interactiva.asm"],
        "8\n",
        limite(),
    );
    assert_eq!(out.status.code(), Some(1), "EOF de stdin = exit 1");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("EOF") || stderr.contains("se agotó"),
        "stderr debía diagnosticar el EOF: {stderr}"
    );
}

/// (e) Bucle infinito + --max-instr → exit 124 en tiempo acotado.
#[test]
fn run_bucle_infinito_corta_por_max_instr() {
    let dir = std::env::temp_dir().join("rp3_run_bucle");
    let _ = std::fs::create_dir_all(&dir);
    let fuente = dir.join("bucle.s");
    std::fs::write(&fuente, ".text\nbucle:\n  j bucle\n").expect("escribir fuente");
    let out = run_con_stdin(
        &["run", fuente.to_str().unwrap(), "--max-instr", "1000000"],
        "",
        limite(),
    );
    assert_eq!(out.status.code(), Some(124), "límite agotado = exit 124");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("max-instr"),
        "stderr debía mencionar el límite"
    );
    assert!(out.stdout.is_empty(), "stdout limpio en el bucle infinito");
    let _ = std::fs::remove_file(&fuente);
}

// ---------------------------------------------------------------------------
// RP3 Fase 3 — bordes headless.
// ---------------------------------------------------------------------------

/// 07-franjas termina SOLO (dibuja y sale): exit 0 incluso con límite amplio.
/// 08-tetris-grafico es por turnos (syscall 5): sin stdin termina por EOF
/// con exit 1 y diagnóstico — sin pánico. (Documentado: 07 termina solo;
/// 08 termina por EOF de entrada en headless.)
#[test]
fn run_ejemplos_graficos_acotados() {
    let out = run_con_stdin(
        &["run", "../ejemplos/07-franjas.asm", "--max-instr", "50000000"],
        "",
        limite(),
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "07 termina solo; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_con_stdin(
        &["run", "../ejemplos/08-tetris-grafico.asm", "--max-instr", "50000000"],
        "",
        limite(),
    );
    assert_eq!(out.status.code(), Some(1), "08 sin stdin termina por EOF");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("EOF") || stderr.contains("se agotó"), "stderr: {stderr}");
    assert!(!stderr.contains("panic"), "sin pánico: {stderr}");
}

/// 09 y 10 (tiempo real): en headless la cola MMIO está vacía y duermen en
/// su bucle de polling — el corte llega por --timeout-s, exit 124 limpio.
#[test]
fn run_tiempo_real_corta_por_timeout() {
    for ejemplo in ["../ejemplos/09-eco-teclado.asm", "../ejemplos/10-tetris-tiempo-real.asm"] {
        let out = run_con_stdin(&["run", ejemplo, "--timeout-s", "1"], "", limite());
        assert_eq!(
            out.status.code(),
            Some(124),
            "{ejemplo} debía cortar por timeout; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!stderr.contains("panic"), "{ejemplo} sin pánico: {stderr}");
    }
}

/// ebreak en headless: aviso por stderr, continúa, exit 0, stdout intacto.
#[test]
fn run_ebreak_headless_avisa_y_continua() {
    let dir = std::env::temp_dir().join("rp3_run_ebreak");
    let _ = std::fs::create_dir_all(&dir);
    let fuente = dir.join("pausa.s");
    std::fs::write(
        &fuente,
        ".data\nmsg: .asciiz \"antes-\"\nmsg2: .asciiz \"despues\\n\"\n.text\n  li a7, 4\n  la a0, msg\n  ecall\n  ebreak\n  la a0, msg2\n  ecall\n  li a7, 10\n  ecall\n",
    )
    .expect("escribir fuente");
    let out = run_con_stdin(&["run", fuente.to_str().unwrap()], "", limite());
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "antes-despues\n",
        "la salida del programa debe quedar intacta"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("ebreak en 0x") && stderr.contains("continuando"),
        "stderr debía avisar la pausa: {stderr}"
    );
    let _ = std::fs::remove_file(&fuente);
}

/// --timeout-s con un programa que duerme (syscall 32, 60 s): corta en 124
/// sin esperar el sueño completo (stop_request interrumpe el sleep).
#[test]
fn run_timeout_corta_sleep_largo() {
    let dir = std::env::temp_dir().join("rp3_run_sleep");
    let _ = std::fs::create_dir_all(&dir);
    let fuente = dir.join("dormilon.s");
    std::fs::write(
        &fuente,
        ".text\n  li a0, 60000\n  li a7, 32\n  ecall\n  li a7, 10\n  ecall\n",
    )
    .expect("escribir fuente");
    let inicio = std::time::Instant::now();
    let out = run_con_stdin(
        &["run", fuente.to_str().unwrap(), "--timeout-s", "1"],
        "",
        limite(),
    );
    assert_eq!(out.status.code(), Some(124));
    assert!(
        inicio.elapsed() < Duration::from_secs(15),
        "el corte no debe esperar los 60 s del sleep"
    );
    let _ = std::fs::remove_file(&fuente);
}

/// Rutas malas: entrada inexistente y salida sin permiso → exit 1 con
/// mensaje claro, sin pánico ni backtrace.
#[test]
fn rutas_invalidas_salen_1_sin_panico() {
    let out = cli()
        .args(["run", "/no/existe/jamas.s"])
        .output()
        .expect("lanzar");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no se pudo leer"), "stderr: {stderr}");
    assert!(!stderr.contains("panic") && !stderr.contains("backtrace"), "{stderr}");

    let out = cli()
        .args([
            "asm",
            "../ejemplos/01-hola-mundo.asm",
            "-o",
            "/no/existe/jamas/salida.elf",
        ])
        .output()
        .expect("lanzar");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no se pudo escribir"), "stderr: {stderr}");
    assert!(!stderr.contains("panic"), "{stderr}");
}

// ---------------------------------------------------------------------------
// RP5 Fase 4 — ejemplo multi-archivo por CLI.
// ---------------------------------------------------------------------------

/// `run` del ejemplo 11 (usa .include, .eqv, .macro y %hi/%lo): salida
/// exacta y exit 0. El CLI resuelve los .include relativo al main.s.
#[test]
fn run_ejemplo_multiarchivo() {
    let out = run_con_stdin(&["run", "../ejemplos/11-multiarchivo/main.s"], "", limite());
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "¡Hola desde varios archivos!\n¡Hola desde varios archivos!\n¡Hola desde varios archivos!\nListo.\n"
    );
}

/// `asm` del ejemplo 11 produce un ELF32 válido.
#[test]
fn asm_ejemplo_multiarchivo() {
    let dir = std::env::temp_dir().join("rp5_asm_multi");
    let _ = std::fs::create_dir_all(&dir);
    let salida = dir.join("multi.elf");
    let out = cli()
        .args([
            "asm",
            "../ejemplos/11-multiarchivo/main.s",
            "-o",
            salida.to_str().unwrap(),
        ])
        .output()
        .expect("lanzar emurv-cli");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = std::fs::read(&salida).expect("leer ELF");
    assert_eq!(&bytes[0..4], b"\x7fELF");
    assert_eq!(bytes[4], 1, "ELFCLASS32");
    let _ = std::fs::remove_file(&salida);
}

/// RP8: el ejemplo del trap handler — ecall desconocido → manejador del
/// guest → mret — corre por CLI con la salida exacta (criterio del ROADMAP).
#[test]
fn run_ejemplo_trap_handler() {
    let out = run_con_stdin(&["run", "../ejemplos/12-trap-handler.asm"], "", limite());
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "antes del ecall desconocido\n  [manejador] atendiendo el servicio 777 (mcause=11)\ndespués: el programa siguió como si nada\n"
    );
}

// ---------------------------------------------------------------------------
// RP10 Fase 1 — exit2 (93): el código del invitado llega al shell, acotado.
// ---------------------------------------------------------------------------

/// Escribe un fuente temporal y devuelve su ruta (cada test usa la suya).
fn fuente_temporal(nombre: &str, contenido: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("rp10_exit2");
    let _ = std::fs::create_dir_all(&dir);
    let ruta = dir.join(nombre);
    std::fs::write(&ruta, contenido).expect("escribir fuente temporal");
    ruta
}

/// EL criterio del ROADMAP, como proceso real: `echo $?` vería el 7.
#[test]
fn exit2_codigo_real_del_proceso() {
    let f = fuente_temporal("siete.s", ".text\n  li a0, 7\n  li a7, 93\n  ecall\n");
    let out = run_con_stdin(&["run", f.to_str().unwrap()], "", limite());
    assert_eq!(out.status.code(), Some(7), "exit code real del invitado");
    assert!(out.stdout.is_empty(), "stdout limpio");
    assert!(out.stderr.is_empty(), "sin avisos para un código en rango");
}

/// Borde superior permitido: 125 pasa sin aviso.
#[test]
fn exit2_125_pasa_sin_aviso() {
    let f = fuente_temporal("tope.s", ".text\n  li a0, 125\n  li a7, 93\n  ecall\n");
    let out = run_con_stdin(&["run", f.to_str().unwrap()], "", limite());
    assert_eq!(out.status.code(), Some(125));
    assert!(out.stderr.is_empty(), "125 está dentro del rango seguro");
}

/// 126 excede el rango seguro del shell: se acota a 125 con aviso.
#[test]
fn exit2_126_se_acota_con_aviso() {
    let f = fuente_temporal("exceso.s", ".text\n  li a0, 126\n  li a7, 93\n  ecall\n");
    let out = run_con_stdin(&["run", f.to_str().unwrap()], "", limite());
    assert_eq!(out.status.code(), Some(125), "acotado al tope");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("125"), "el aviso explica el acote: {stderr}");
}

/// Negativo: «& 0xFF» estilo Unix (-1 → 255) y DESPUÉS el tope → 125 con
/// aviso. Documenta el diseño acordado: ningún código negativo escapa.
#[test]
fn exit2_negativo_estilo_unix() {
    let f = fuente_temporal("neg.s", ".text\n  li a0, -1\n  li a7, 93\n  ecall\n");
    let out = run_con_stdin(&["run", f.to_str().unwrap()], "", limite());
    assert_eq!(out.status.code(), Some(125), "-1 & 0xFF = 255 → acotado");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("255"), "el aviso muestra el valor enmascarado: {stderr}");
}

/// Regresión: syscall 10 sigue saliendo con 0 aunque a0 tenga basura.
#[test]
fn exit_10_sigue_dando_cero_como_proceso() {
    let f = fuente_temporal("diez.s", ".text\n  li a0, 99\n  li a7, 10\n  ecall\n");
    let out = run_con_stdin(&["run", f.to_str().unwrap()], "", limite());
    assert_eq!(out.status.code(), Some(0));
}

// ---------------------------------------------------------------------------
// RP10 Fase 2 — programa combinado: lee con 8 al heap de 9 y lo imprime.
// ---------------------------------------------------------------------------

#[test]
fn readstring_y_sbrk_combinados_por_pipe() {
    let f = fuente_temporal(
        "nombre.s",
        ".text
  li a0, 32
  li a7, 9          # sbrk: a0 = buffer en el heap
  ecall
  mv s0, a0
  li a1, 32
  li a7, 8          # leer cadena al buffer
  ecall
  mv a0, s0
  li a7, 4          # imprimirla
  ecall
  li a7, 10
  ecall
",
    );
    let out = run_con_stdin(&["run", f.to_str().unwrap()], "Ana\n", limite());
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "Ana",
        "la cadena guardada en el heap se imprime SIN salto extra (la 8 no \
         agrega el '\\n' de fgets — apartamiento documentado)"
    );
}

// ---------------------------------------------------------------------------
// RP10 Fase 3 — syscall 60 en pipe: NADA de ANSI (los diffs de CI no se
// ensucian; la detección de TTY hace el resto en una terminal real).
// ---------------------------------------------------------------------------

#[test]
fn syscall_60_en_pipe_no_emite_ansi() {
    let f = fuente_temporal(
        "limpiar.s",
        ".text
  li a0, 65
  li a7, 11
  ecall
  li a7, 60
  ecall
  li a0, 66
  li a7, 11
  ecall
  li a7, 10
  ecall
",
    );
    let out = run_con_stdin(&["run", f.to_str().unwrap()], "", limite());
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "AB",
        "en pipe el 60 es invisible: solo la salida del invitado"
    );
    assert!(
        !out.stdout.contains(&0x1B),
        "ni un byte ESC en el stdout de un pipe"
    );
}

// ---------------------------------------------------------------------------
// RP11 Fase 2 — run de ELF importados (detección por magic).
// ---------------------------------------------------------------------------

/// Round-trip por procesos: `asm` exporta el ELF y `run` lo ejecuta con la
/// MISMA salida que el fuente. La extensión no importa: detecta por magic.
#[test]
fn run_elf_round_trip_misma_salida() {
    let dir = std::env::temp_dir().join("rp11_roundtrip");
    let _ = std::fs::create_dir_all(&dir);
    let fuente = dir.join("prog.s");
    std::fs::write(
        &fuente,
        ".data\nmsj: .asciiz \"hola elf\\n\"\n.text\n  la a0, msj\n  li a7, 4\n  ecall\n  li a7, 10\n  ecall\n",
    )
    .expect("fuente");
    // Sin extensión .elf a propósito: la detección es por magic.
    let binario = dir.join("prog.bin");

    let out = run_con_stdin(
        &["asm", fuente.to_str().unwrap(), "-o", binario.to_str().unwrap()],
        "",
        limite(),
    );
    assert_eq!(out.status.code(), Some(0), "asm: {}", String::from_utf8_lossy(&out.stderr));

    let del_fuente = run_con_stdin(&["run", fuente.to_str().unwrap()], "", limite());
    let del_binario = run_con_stdin(&["run", binario.to_str().unwrap()], "", limite());
    assert_eq!(del_binario.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&del_binario.stdout),
        String::from_utf8_lossy(&del_fuente.stdout),
        "misma salida fuente vs ELF"
    );
    assert_eq!(String::from_utf8_lossy(&del_binario.stdout), "hola elf\n");
}

/// La fixture REAL de gcc corre por CLI: "42\n" y exit 0 (sale con 93).
#[test]
fn run_fixture_gnu() {
    let out = run_con_stdin(
        &["run", "../src-tauri/tests/fixtures/rp11/fixture.elf"],
        "",
        limite(),
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

/// Un archivo que NO es ELF ni ensambla → exit 2 con diagnóstico claro.
#[test]
fn run_binario_no_elf_sale_2() {
    let dir = std::env::temp_dir().join("rp11_noelf");
    let _ = std::fs::create_dir_all(&dir);
    let ruta = dir.join("basura.elf");
    std::fs::write(&ruta, b"\x00\x01\x02no soy un elf").expect("escribir");
    let out = run_con_stdin(&["run", ruta.to_str().unwrap()], "", limite());
    assert_eq!(out.status.code(), Some(2));
}

// ---------------------------------------------------------------------------
// RP11 Fase 3 — subcomando disasm: listado estable por stdout.
// ---------------------------------------------------------------------------

/// Salida EXACTA y estable (la CI la vigila); fuente y ELF dan lo mismo.
#[test]
fn disasm_salida_estable_fuente_y_elf() {
    let f = fuente_temporal("dis.s", ".text\n  li a7, 10\n  ecall\n");
    let esperado = "00400000:  00a00893  addi a7, zero, 0xa\n\
                    00400004:  00000073  ecall\n";

    let out = run_con_stdin(&["disasm", f.to_str().unwrap()], "", limite());
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stdout), esperado);

    // El MISMO programa como ELF: listado idéntico.
    let elf = std::env::temp_dir().join("rp10_exit2").join("dis.elf");
    let out = run_con_stdin(
        &["asm", f.to_str().unwrap(), "-o", elf.to_str().unwrap()],
        "",
        limite(),
    );
    assert_eq!(out.status.code(), Some(0));
    let out = run_con_stdin(&["disasm", elf.to_str().unwrap()], "", limite());
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stdout), esperado);
}

/// disasm sin argumentos → uso inválido (exit 3).
#[test]
fn disasm_sin_argumentos_sale_3() {
    let out = run_con_stdin(&["disasm"], "", limite());
    assert_eq!(out.status.code(), Some(3));
}

// ---------------------------------------------------------------------------
// RP12 Fase 4 — ejemplo 13 (flotantes) por CLI + disasm de F/D.
// ---------------------------------------------------------------------------

/// El ejemplo 13 por pipe: promedio de 3 notas con bits IEEE exactos
/// (7.5 + 8.0 + 10.0 = 25.5 / 3 = 8.5, todos representables en f32).
#[test]
fn run_ejemplo_flotantes() {
    let out = run_con_stdin(
        &["run", "../ejemplos/13-flotantes.asm"],
        "7.5\n8.0\n10.0\n",
        limite(),
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).trim_end().ends_with("Promedio: 8.5"),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// disasm decodifica el repertorio F/D (capstone arch_riscv lo cubre): el
/// listado del ejemplo 13 trae fcvt/fadd/fdiv/fmv reconocibles.
#[test]
fn disasm_decodifica_fd() {
    let out = run_con_stdin(&["disasm", "../ejemplos/13-flotantes.asm"], "", limite());
    assert_eq!(out.status.code(), Some(0));
    let listado = String::from_utf8_lossy(&out.stdout);
    for esperado in ["fcvt.s.w", "fadd.s", "fdiv.s", "fmv.s"] {
        assert!(listado.contains(esperado), "falta {esperado} en:\n{listado}");
    }
}

// ---------------------------------------------------------------------------
// RP13 Fase 4 — ejemplo 14 (atómicas + comprimidas) por CLI.
// ---------------------------------------------------------------------------

/// El ejemplo 14 corre: contador atómico (amoadd + lr/sc) que termina en
/// 100, con instrucciones C de 2 bytes mezcladas (el motor las ejecuta).
#[test]
fn run_ejemplo_atomicas() {
    let out = run_con_stdin(&["run", "../ejemplos/14-atomicas.asm"], "", limite());
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "100\n");
}

/// disasm del ejemplo 14: lista A (amoadd.w/lr.w/sc.w) y C (c.li…) con
/// longitud variable, sin colgarse.
#[test]
fn disasm_ejemplo_atomicas_mixto() {
    let out = run_con_stdin(&["disasm", "../ejemplos/14-atomicas.asm"], "", limite());
    assert_eq!(out.status.code(), Some(0));
    let listado = String::from_utf8_lossy(&out.stdout);
    assert!(listado.contains("amoadd.w"), "{listado}");
    assert!(listado.contains("c.li"), "{listado}");
}
