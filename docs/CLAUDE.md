# CLAUDE.md — Instrucciones operativas para Claude Code

## Contexto
Prototipo técnico de TFG (FIUNI, Ing. en Computación): entorno educativo moderno de programación en ensamblador RISC-V (RV32), concebido como reemplazo de emu8086. Primera iteración: macOS Apple Silicon. La arquitectura debe quedar preparada para compilar después en Windows y Linux, pero NO se compilan esos targets ahora.

## Reglas de trabajo (no negociables)
1. Lee SPEC.md, ARQUITECTURA.md y PLAN.md completos antes de escribir una sola línea de código.
2. Trabaja por fases según PLAN.md. No mezcles fases. No expandas el alcance: si se te ocurre una mejora fuera de SPEC.md, regístrala en BITACORA.md bajo "PROPUESTAS FUERA DE ALCANCE" y seguí con el plan.
3. Git: `git init` al comienzo. Commit como mínimo al cierre de cada fase. Mensajes en español, formato `fase-N: descripción breve`.
4. Tests: el núcleo (ensamblador, máquina, syscalls, export ELF, source_map) debe tener tests unitarios en Rust. PROHIBIDO cerrar una fase con tests en rojo.
5. BITACORA.md (en la raíz): registrá cada decisión técnica no trivial con: fecha, decisión, alternativas consideradas, nivel de confianza [Seguro / Probable / Suposición]. Este archivo es material de tesis: escribilo como si lo fuera a leer un tribunal.
6. UI íntegramente en español, pero con textos externalizados en archivos de traducción (i18n preparado, no strings hardcodeados). El español es la lengua primaria, no una traducción.
7. Si una dependencia falla (en particular los bindings de Keystone), aplicá el plan B documentado en ARQUITECTURA.md §Riesgos y registralo. No te quedes bloqueado.
8. Cierre del proyecto: `cargo test` verde + `cargo clippy` sin warnings + `cargo tauri build` exitoso + checklist de requisitos RF-01..RF-10 con evidencia en BITACORA.md.

## Convenciones
- Rust: rustfmt, clippy limpio.
- Frontend: TypeScript estricto. Preferencia: Svelte o vanilla TS + CodeMirror 6. Nada de frameworks pesados sin justificación registrada.
- Identificadores de código en inglés; comentarios, docs y UI en español.
- Rendimiento es requisito de primera clase (ver RNF en SPEC.md): nunca refresques la UI instrucción por instrucción durante ejecución continua.

## Roadmap profesional (post-v0.1)
Existe `docs/ROADMAP-PRO.md`: extensiones aprobadas por el autor (RP-1..RP-8) con un
protocolo de trabajo propio (plan → prompt → ejecución en otra terminal → verificación
y actualización de docs). Si el autor pide "creá el plan para el siguiente punto" o
nombra un RP-N, leé ese archivo y seguí su protocolo al pie de la letra. Los planes
RP se escriben como `docs/PLAN-RP<N>-<slug>.md` y los commits de su ejecución usan
formato `rp<N>-fase-M: descripción`. La regla 2 de este archivo (no expandir alcance)
aplica dentro de cada plan RP, no contra el roadmap.
