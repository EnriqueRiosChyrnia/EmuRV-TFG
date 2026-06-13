# PLAN-RP4 — CI con GitHub Actions (autoría local; primer run diferido)

Punto RP-4 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

## Contexto y restricción clave

- El repo es SOLO LOCAL por decisión del autor (2026-06-12): el remote de GitHub se
  creará al final del proyecto. Por lo tanto este punto se ejecuta en dos mitades:
  - **Ahora (este plan):** autoría del workflow, validación estática, y réplica local
    de cada paso del CI para demostrar que los comandos pasan.
  - **Al crear el remote (fuera de este plan):** push, primer run real, badge, y el
    reporte empírico de qué falla en Windows/Linux. Queda como checklist registrado.
- Estructura del repo (post RP-3): DOS crates sin workspace — `src-tauri/` (app Tauri,
  lib `emurv_lib` + tests TEC/FB/RNF/RP1) y `emurv-cli/` (CLI headless, depende de la
  lib por path; sus 13 tests de integración corren los ejemplos como procesos).
  Frontend: TypeScript estricto + Vite (`npm ci`, `tsc`, `vite build`).
- Unicorn Engine compila vía CMake — solo probado en macOS ARM. La matriz
  Windows/Linux existe precisamente para revelar qué rompe (Cap. VI §6.2.1).

## Alcance

- SÍ: `.github/workflows/ci.yml`, script espejo local, validación estática
  (actionlint), documentación de dependencias de build por plataforma, checklist
  de activación al crear el remote.
- NO: crear el remote, push, badge funcional (queda placeholder), releases firmados,
  cargo-audit/dependabot (proponer en BITACORA si parece valioso), workspace Cargo
  (la estructura de dos crates quedó decidida en RP-3 por RNF-04 — no tocarla).

## Diseño acordado (no re-litigar)

- **Principio espejo:** los jobs del CI ejecutan los MISMOS comandos que un script
  local `scripts/ci-local.sh`. Lo que el script verifica hoy en esta máquina es lo
  que el runner de macOS verificará al activarse. Sin lógica exclusiva del YAML más
  allá del wiring (checkout, toolchains, caché, matriz).
- **Jobs:**
  1. `test-macos` (macos-latest, OBLIGATORIO verde): toolchain Rust estable +
     `npm ci` + `tsc --noEmit` + `vite build` + `cargo test` y `cargo clippy --
     -D warnings` en `src-tauri/` Y en `emurv-cli/` + `cargo build --release --bin
     emurv-cli`.
  2. `bundle-macos` (macos-latest, obligatorio): `cargo tauri build` — el gate real
     de empaquetado; verificar peso del bundle ≤ 30 MB (RNF-04) con un paso que
     falle si se excede (eso habría cazado la regresión de RP-3 automáticamente).
  3. `build-linux` (ubuntu-latest, `continue-on-error: true`, experimental):
     instalar deps del sistema (cmake, pkg-config, libwebkit2gtk-4.1-dev,
     libayatana-appindicator3-dev, librsvg2-dev, libxdo-dev, libssl-dev,
     build-essential) + `cargo build` de ambos crates + `cargo test -p` del CLI si
     el build pasa. Su VALOR es el log de fallos como insumo del port (§6.2.1).
  4. `build-windows` (windows-latest, `continue-on-error: true`, experimental):
     `cargo build` de ambos crates. Mismo propósito.
- **Triggers:** `push` y `pull_request` a `main` y `feature/**`.
- **Caché:** `Swatinem/rust-cache@v2` por crate + caché de npm — sin caché, cada
  run compila Unicorn desde cero (~minutos).
- **Validación sin remote:** `actionlint` (instalar vía `brew install actionlint`;
  si brew falla, descargar el binario de su release — registrar el método en
  BITACORA). El YAML además se parsea con Python como chequeo mínimo.

## Fase 1 — Workflow + validación estática

1. Escribir `.github/workflows/ci.yml` con los 4 jobs del diseño.
2. Validar: `actionlint` sin errores + parseo YAML limpio.
3. Badge en README con placeholder `OWNER/REPO` y comentario «activar al crear el
   remote» (no inventar URL real).

**Compuerta F1:** actionlint limpio + YAML parsea + los comandos de los jobs son
EXACTAMENTE los del script de la Fase 2 (revisión cruzada línea por línea).
Commit `rp4-fase-1: workflow ci.yml validado estáticamente`.

## Fase 2 — Script espejo local

1. `scripts/ci-local.sh` (bash, `set -euo pipefail`): replica los jobs 1 y 2 de
   macOS en secuencia, con salida clara por sección. Acepta `--skip-bundle` para
   correr solo el job 1 (el bundle tarda).
2. Chequeo de peso del bundle: el mismo paso/script que usa el job `bundle-macos`
   (un one-liner con `du` + umbral 30 MB) — compartido, no duplicado.
3. Correrlo COMPLETO en esta máquina: debe terminar verde.

**Compuerta F2:** `scripts/ci-local.sh` termina exit 0 con todas las secciones
verdes (incluido bundle ≤ 30 MB). Pegar el resumen de salida en BITACORA.
Commit `rp4-fase-2: script espejo ci-local verde`.

## Fase 3 — Piernas experimentales (Linux/Windows) y deps documentadas

1. Revisar los jobs 3 y 4 contra la documentación oficial de Tauri v2 para deps de
   sistema Linux (la lista del diseño puede tener drift — verificar contra
   https://tauri.app/start/prerequisites/ y registrar la lista final).
2. README del repo (o docs/ARQUITECTURA.md §port, donde mejor encaje): tabla de
   dependencias de build por plataforma (macOS: Xcode CLT + CMake; Linux: lista
   verificada; Windows: MSVC + CMake), citando que Linux/Windows están SIN PROBAR.
3. `actionlint` de nuevo tras los cambios.

**Compuerta F3:** actionlint limpio + deps documentadas con fuente.
Commit `rp4-fase-3: piernas experimentales y deps por plataforma`.

## Fase 4 — Documentación, checklist de activación y cierre

1. README: sección «Integración continua» — qué corre el CI, qué es experimental,
   cómo correr el espejo local (`scripts/ci-local.sh`).
2. **Checklist de activación** (en `docs/ROADMAP-PRO.md` NO — escribirla en
   BITACORA y en un bloque al final de `.github/workflows/ci.yml` como comentario):
   al crear el remote → (a) reemplazar placeholder del badge, (b) push y verificar
   los 4 jobs, (c) registrar en BITACORA los fallos de Linux/Windows como insumo
   del port (§6.2.1), (d) proteger `main` con el job obligatorio si se desea.
3. BITACORA: decisiones (matriz elegida, caché, umbral 30 MB automatizado, método
   de instalación de actionlint), niveles de confianza.
4. `docs/ROADMAP-PRO.md`: NO tocar — lo gestiona la sesión coordinadora.

**Compuerta F4 (cierre):** `cargo test` verde en ambos crates + clippy limpio en
ambos + `scripts/ci-local.sh` verde + actionlint limpio.
Commit `rp4-fase-4: docs CI y checklist de activación`.

## Criterios de aceptación (ajustados a la restricción sin remote)

- Workflow completo y validado estáticamente (actionlint). ✓ F1/F3.
- Cada comando del job obligatorio demostrado verde en local vía script espejo,
  incluido el gate automatizado de peso ≤ 30 MB. ✓ F2.
- Deps de build por plataforma documentadas con fuente. ✓ F3.
- Checklist de activación registrada para el momento de crear el remote. ✓ F4.
- DIFERIDO (no es deuda de este plan): primer run real, badge verde, reporte
  empírico Windows/Linux — ocurre cuando el autor cree el remote al final del
  proyecto.
