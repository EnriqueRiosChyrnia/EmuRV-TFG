#!/usr/bin/env bash
# Espejo local del CI (RP-4): ejecuta los MISMOS comandos que los jobs
# test-macos y bundle-macos de .github/workflows/ci.yml, en secuencia.
# Lo que este script verifica hoy en esta máquina es lo que el runner de
# macOS verificará cuando el remote exista (principio espejo: si cambiás
# un comando acá, cambialo en ci.yml — y al revés).
#
# Uso:
#   scripts/ci-local.sh                # todo (job 1 + job 2 con bundle)
#   scripts/ci-local.sh --skip-bundle  # solo el job 1 (el bundle tarda)
set -euo pipefail

cd "$(dirname "$0")/.."

SKIP_BUNDLE=0
if [ "${1:-}" = "--skip-bundle" ]; then
  SKIP_BUNDLE=1
elif [ -n "${1:-}" ]; then
  echo "uso: scripts/ci-local.sh [--skip-bundle]" >&2
  exit 3
fi

seccion() {
  echo
  echo "================================================================"
  echo "== $1"
  echo "================================================================"
}

# --- Job 1: test-macos ------------------------------------------------------

seccion "Frontend (npm ci + tsc + vite build)"
npm ci
npx tsc --noEmit
npx vite build

seccion "Tests y clippy (src-tauri)"
(
  cd src-tauri
  cargo test
  cargo clippy --all-targets -- -D warnings
)

seccion "Tests y clippy (emurv-cli)"
(
  cd emurv-cli
  cargo test
  cargo clippy --all-targets -- -D warnings
)

seccion "Build release del CLI"
(
  cd emurv-cli
  cargo build --release --bin emurv-cli
)

# --- Job 2: bundle-macos ----------------------------------------------------

if [ "$SKIP_BUNDLE" -eq 1 ]; then
  seccion "Bundle: SALTEADO (--skip-bundle)"
else
  seccion "Bundle de la app (npx tauri build)"
  npx tauri build

  seccion "Peso del bundle ≤ 30 MB (RNF-04)"
  bash scripts/check-bundle-size.sh
fi

seccion "CI LOCAL: TODO VERDE"
