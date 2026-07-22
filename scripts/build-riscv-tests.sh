#!/usr/bin/env bash
# RT-2 — Compila los riscv-tests oficiales contra el env EmuRV y los
# vendoriza como fixtures del harness rt2_riscv_tests.
#
# La CI NO ejecuta este script: consume los ELFs commiteados en
# src-tauri/tests/fixtures/riscv-tests/. Este script es el procedimiento
# reproducible que los regenera (material de tesis: procedencia en
# MANIFEST.txt).
#
# Requisitos: riscv64-elf-gcc (brew install riscv64-elf-gcc), git.
# Uso: scripts/build-riscv-tests.sh [suite...]   (sin args: las 6 suites RV32)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_DIR="$REPO_ROOT/tests-riscv/env-emurv"
UPSTREAM="$REPO_ROOT/tests-riscv/.upstream/riscv-tests"
OUT_ROOT="$REPO_ROOT/src-tauri/tests/fixtures/riscv-tests"

# Commit pineado de riscv-software-src/riscv-tests (2026-06-03).
RISCV_TESTS_URL="https://github.com/riscv-software-src/riscv-tests.git"
RISCV_TESTS_COMMIT="34e6b6d1e7936b526075432fb730d89148623484"

GCC="${RISCV_GCC:-riscv64-elf-gcc}"

command -v "$GCC" >/dev/null || {
    echo "ERROR: no se encontró $GCC (brew install riscv64-elf-gcc)" >&2
    exit 1
}

# march/mabi por suite. GCC >= 12 exige Zicsr/Zifencei explícitas
# (los tests usan CSRs siempre —mtvec del env— y fence.i en rv32ui).
march_de() {
    case "$1" in
        rv32ui) echo "rv32i_zicsr_zifencei" ;;
        rv32um) echo "rv32im_zicsr" ;;
        rv32ua) echo "rv32ia_zicsr" ;;
        rv32uc) echo "rv32ic_zicsr" ;;
        rv32uf) echo "rv32if_zicsr" ;;
        rv32ud) echo "rv32ifd_zicsr" ;;
        *) echo "ERROR: suite desconocida: $1" >&2; exit 1 ;;
    esac
}
mabi_de() {
    case "$1" in
        rv32uf) echo "ilp32f" ;;
        rv32ud) echo "ilp32d" ;;
        *) echo "ilp32" ;;
    esac
}

SUITES=("$@")
[ ${#SUITES[@]} -eq 0 ] && SUITES=(rv32ui rv32um rv32ua rv32uc rv32uf rv32ud)

# --- Upstream pineado ---
if [ ! -d "$UPSTREAM/.git" ]; then
    mkdir -p "$(dirname "$UPSTREAM")"
    git clone --filter=blob:none "$RISCV_TESTS_URL" "$UPSTREAM"
fi
git -C "$UPSTREAM" fetch --quiet origin "$RISCV_TESTS_COMMIT" 2>/dev/null || true
git -C "$UPSTREAM" checkout --quiet "$RISCV_TESTS_COMMIT"

# --- Compilación ---
CFLAGS_COMUNES=(-static -mcmodel=medany -fvisibility=hidden
                -nostdlib -nostartfiles
                -I "$ENV_DIR" -I "$UPSTREAM/isa/macros/scalar"
                -T "$ENV_DIR/link.ld")

total=0
for suite in "${SUITES[@]}"; do
    march="$(march_de "$suite")"
    mabi="$(mabi_de "$suite")"
    outdir="$OUT_ROOT/$suite"
    rm -rf "$outdir"
    mkdir -p "$outdir"

    # Lista autoritativa de tests: la variable <suite>_sc_tests del Makefrag.
    tests="$(awk -v var="${suite}_sc_tests" '
        $0 ~ "^"var" = \\\\$" {dentro=1; next}
        dentro && /^\s*$/ {dentro=0}
        dentro {gsub(/\\/,""); printf "%s ", $0}
    ' "$UPSTREAM/isa/$suite/Makefrag")"
    [ -n "$tests" ] || { echo "ERROR: Makefrag de $suite sin tests" >&2; exit 1; }

    for t in $tests; do
        "$GCC" -march="$march" -mabi="$mabi" "${CFLAGS_COMUNES[@]}" \
            "$UPSTREAM/isa/$suite/$t.S" -o "$outdir/$t.elf"
        total=$((total + 1))
    done
    echo "$suite: $(echo "$tests" | wc -w | tr -d ' ') tests compilados (march=$march mabi=$mabi)"
done

# --- Manifiesto de procedencia ---
{
    echo "# Fixtures riscv-tests para el harness RT-2 (rt2_riscv_tests.rs)"
    echo "# Regenerar con: scripts/build-riscv-tests.sh"
    echo "origen: $RISCV_TESTS_URL"
    echo "commit: $RISCV_TESTS_COMMIT"
    echo "toolchain: $("$GCC" --version | head -1)"
    echo "env: tests-riscv/env-emurv (link.ld + riscv_test.h propios de EmuRV)"
    echo "fecha: $(date +%Y-%m-%d)"
    echo
    (cd "$OUT_ROOT" && find . -name '*.elf' | sort | xargs shasum -a 256)
} > "$OUT_ROOT/MANIFEST.txt"

echo "OK: $total ELFs en $OUT_ROOT (MANIFEST.txt actualizado)"
