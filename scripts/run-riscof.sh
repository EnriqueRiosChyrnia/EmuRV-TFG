#!/usr/bin/env bash
# RT-3 — Compliance formal con RISCOF: riscv-arch-test sobre EmuRV (DUT)
# contra el modelo dorado Sail (referencia).
#
# La CI NO ejecuta esto (toolchain Python+Sail pesada para el runner macOS):
# se corre local y los resultados quedan en docs/RESULTADOS-RISCOF.md.
# Este script es el procedimiento reproducible.
#
# Uso:
#   scripts/run-riscof.sh setup    # venv + Sail prebuilt + arch-test pineados
#   scripts/run-riscof.sh run      # riscof run (requiere setup previo y F2/F3)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RISCV_DIR="$REPO_ROOT/tests-riscv"
VENV="$RISCV_DIR/.venv"
TOOLS="$RISCV_DIR/.tools"
ARCH_TEST="$RISCV_DIR/.upstream/riscv-arch-test"
PLUGIN_DIR="$RISCV_DIR/riscof-plugin-emurv"
WORK_DIR="$RISCV_DIR/.riscof-work"

# Versiones pineadas (BITACORA RT-3)
RISCOF_VERSION="1.25.3"
SAIL_RELEASE="0.13"
SAIL_URL="https://github.com/riscv/sail-riscv/releases/download/$SAIL_RELEASE/sail-riscv-Mac-arm64.tar.gz"
SAIL_SHA256="72e86de7f4cb5025a47509f22eab26aca1422b0ea81c4a3ed01150de97b06c8e"
# riscv-arch-test: lo clona `riscof arch-test --clone` a la versión compatible
# con RISCOF; la corrida de referencia usó ctp-release-e9514aa-2025-12-28
# (commit 281d71ef3d61e32111217b20305ea3ef9b1582e2).

setup() {
    # venv con riscof pineado
    if [ ! -x "$VENV/bin/riscof" ]; then
        python3 -m venv "$VENV"
        "$VENV/bin/pip" install --quiet "riscof==$RISCOF_VERSION"
    fi
    "$VENV/bin/riscof" --version

    # Sail prebuilt con sha256 verificado
    if [ ! -x "$TOOLS/sail-riscv-Mac-arm64/bin/sail_riscv_sim" ]; then
        mkdir -p "$TOOLS"
        curl -sL -o "$TOOLS/sail.tar.gz" "$SAIL_URL"
        echo "$SAIL_SHA256  $TOOLS/sail.tar.gz" | shasum -a 256 -c -
        tar xzf "$TOOLS/sail.tar.gz" -C "$TOOLS"
        xattr -d com.apple.quarantine "$TOOLS/sail-riscv-Mac-arm64/bin/sail_riscv_sim" 2>/dev/null || true
    fi
    "$TOOLS/sail-riscv-Mac-arm64/bin/sail_riscv_sim" --version

    # arch-test a la versión que RISCOF declara compatible
    if [ ! -d "$ARCH_TEST/.git" ]; then
        "$VENV/bin/riscof" arch-test --clone --dir "$ARCH_TEST"
    fi
    git -C "$ARCH_TEST" log -1 --format="arch-test: %H (%ad)" --date=short

    # runner DUT en release (el core C de Unicorn en debug es lentísimo)
    (cd "$REPO_ROOT/src-tauri" && cargo build --release --features riscof-runner --bin riscof_runner)
    echo "setup OK"
}

run() {
    # $1 = sub-suite de rv32i_m (I, M, C, ...). Se corre de a una para no
    # arrastrar suites fuera de alcance (privilege ejercita M-mode que EmuRV
    # delega en políticas didácticas — ver PLAN-RT3).
    local suite="${1:?uso: run <I|M|C|...>}"
    [ -x "$VENV/bin/riscof" ] || { echo "falta setup" >&2; exit 1; }
    mkdir -p "$WORK_DIR"
    cd "$PLUGIN_DIR"
    "$VENV/bin/riscof" run \
        --config config.ini \
        --suite "$ARCH_TEST/riscv-test-suite/rv32i_m/$suite" \
        --env "$ARCH_TEST/riscv-test-suite/env" \
        --work-dir "$WORK_DIR/$suite" \
        --no-browser
}

case "${1:-}" in
    setup) setup ;;
    run) shift; run "${1:-}" ;;
    *) echo "uso: $0 setup|run <suite>" >&2; exit 1 ;;
esac
