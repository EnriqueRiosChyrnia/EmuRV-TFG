#!/usr/bin/env bash
# RT-4 — Benchmark de rendimiento: EmuRV vs qemu-riscv32 vs Spike vs RARS.
#
# Metodología (PLAN-RT4): mismo algoritmo en todos los targets (kernels de
# bench/kernels/, wrappers por target), métrica = M instr/s con cuenta
# NOMINAL de instrucciones (documentada en cada kernel), wall-time mediana
# de N repeticiones. RARS corre con iteraciones reducidas (factor
# documentado) y se le descuenta el arranque de la JVM.
#
# Uso:
#   scripts/bench-rt4.sh build     # genera fuentes y compila binarios
#   scripts/bench-rt4.sh verify    # equivalencia: mismo resultado en los 4
#   scripts/bench-rt4.sh run       # medición completa → bench/out/rt4.csv

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BENCH="$REPO_ROOT/bench"
OUT="$BENCH/out"
RARS_JAR="$REPO_ROOT/tests-riscv/.tools/rars/rars.jar"
EMURV_CLI="$REPO_ROOT/emurv-cli/target/release/emurv-cli"

REPS=5
KERNELS=(aritmetica memoria llamadas mixto)

# Instrucciones nominales por iteración y pre/post (comentario de cada kernel)
instr_por_iter() { case "$1" in aritmetica) echo 10;; memoria) echo 9;; llamadas) echo 13;; mixto) echo 11;; esac; }

# Iteraciones de medición (~500M instr nominales en los targets rápidos);
# RARS: /100 (intérprete Java, mismo métrica MIPS — factor documentado).
iter_rapido() { case "$1" in aritmetica) echo 50000000;; memoria) echo 55000000;; llamadas) echo 38000000;; mixto) echo 45000000;; esac; }
iter_rars()   { echo $(( $(iter_rapido "$1") / 100 )); }
ITER_VERIFY=100000

genera() { # $1 kernel, $2 template, $3 iteraciones, $4 salida
    sed "s/@ITER@/$3/" "$BENCH/wrappers/$2" > "$4"
    cat "$BENCH/kernels/$1.s" >> "$4"
}

build() {
    mkdir -p "$OUT"
    (cd "$REPO_ROOT/emurv-cli" && cargo build --release --quiet)
    for k in "${KERNELS[@]}"; do
        for modo in verify fast rars; do
            case "$modo" in
                verify) it=$ITER_VERIFY ;;
                fast)   it=$(iter_rapido "$k") ;;
                rars)   it=$(iter_rars "$k") ;;
            esac
            genera "$k" emurv-rars.tpl.asm "$it" "$OUT/$k-$modo.asm"
            genera "$k" bare-qemu.tpl.S    "$it" "$OUT/$k-$modo-qemu.S"
            genera "$k" bare-spike.tpl.S   "$it" "$OUT/$k-$modo-spike.S"
            riscv64-elf-gcc -march=rv32im -mabi=ilp32 -static -nostdlib -nostartfiles \
                -T "$BENCH/wrappers/spike-link.ld" \
                "$OUT/$k-$modo-qemu.S" -o "$OUT/$k-$modo-qemu.elf"
            riscv64-elf-gcc -march=rv32im -mabi=ilp32 -static -nostdlib -nostartfiles \
                -T "$BENCH/wrappers/spike-link.ld" \
                "$OUT/$k-$modo-spike.S" -o "$OUT/$k-$modo-spike.elf"
        done
    done
    # programa vacío para el baseline de arranque de la JVM/RARS
    printf '.text\nmain:\n    li a7, 10\n    ecall\n' > "$OUT/vacio.asm"
    echo "build OK ($OUT)"
}

# --- corredores: imprimen el resultado decimal por stdout ---
corre_emurv() { "$EMURV_CLI" run "$1" 2>/dev/null; }
corre_rars()  { java -jar "$RARS_JAR" nc "$1" 2>/dev/null | tr -d '[:space:]'; echo; }
corre_qemu()  { # system-mode + semihosting (user-mode no existe en macOS)
    qemu-system-riscv32 -M virt -bios none -nographic -semihosting -kernel "$1" 2>&1 | tr -d '\r'
}
corre_spike() { # signature → decimal
    local sig; sig="$(mktemp)"
    spike --isa=rv32im +signature="$sig" +signature-granularity=4 "$1" >/dev/null 2>&1
    python3 -c "print(int(open('$sig').readline().strip(),16))"
    rm -f "$sig"
}

verify() {
    local fallo=0
    for k in "${KERNELS[@]}"; do
        local e r q s
        e=$(corre_emurv "$OUT/$k-verify.asm")
        r=$(corre_rars  "$OUT/$k-verify.asm")
        q=$(corre_qemu  "$OUT/$k-verify-qemu.elf")
        s=$(corre_spike "$OUT/$k-verify-spike.elf")
        # RARS imprime el int con signo; normalizar a u32
        r=$(python3 -c "print($r & 0xffffffff)")
        e=$(python3 -c "print($e & 0xffffffff)")
        q=$(python3 -c "print($q & 0xffffffff)")
        if [ "$e" = "$r" ] && [ "$e" = "$q" ] && [ "$e" = "$s" ]; then
            echo "$k: EQUIVALENTE ($e)"
        else
            echo "$k: DISCREPANCIA emurv=$e rars=$r qemu=$q spike=$s"
            fallo=1
        fi
    done
    exit $fallo
}

# mediana de $REPS corridas de "$@" (wall time, segundos con 3 decimales)
mediana() {
    local tiempos=()
    for _ in $(seq 1 $REPS); do
        local t0 t1
        t0=$(python3 -c 'import time; print(time.monotonic())')
        "$@" >/dev/null 2>&1
        t1=$(python3 -c 'import time; print(time.monotonic())')
        tiempos+=("$(python3 -c "print($t1-$t0)")")
    done
    python3 -c "
import statistics, sys
ts = sorted(float(x) for x in sys.argv[1:])
print(f'{statistics.median(ts):.3f} {ts[0]:.3f} {ts[-1]:.3f}')" "${tiempos[@]}"
}

run() {
    local csv="$OUT/rt4.csv"
    echo "kernel,emulador,iteraciones,instr_nominales,mediana_s,min_s,max_s,mips" > "$csv"
    # baseline JVM+RARS (programa vacío)
    local base_rars
    base_rars=$(mediana java -jar "$RARS_JAR" nc "$OUT/vacio.asm" | awk '{print $1}')
    echo "baseline RARS (JVM + ensamblado, programa vacío): ${base_rars}s"

    for k in "${KERNELS[@]}"; do
        local ipi n_fast n_rars nominal
        ipi=$(instr_por_iter "$k")
        n_fast=$(iter_rapido "$k"); n_rars=$(iter_rars "$k")

        for emu in emurv qemu spike rars; do
            local n=$n_fast cmd=()
            case "$emu" in
                emurv) cmd=(corre_emurv "$OUT/$k-fast.asm") ;;
                qemu)  cmd=(corre_qemu  "$OUT/$k-fast-qemu.elf") ;;
                spike) cmd=(corre_spike "$OUT/$k-fast-spike.elf") ;;
                rars)  n=$n_rars; cmd=(corre_rars "$OUT/$k-rars.asm") ;;
            esac
            nominal=$(python3 -c "print($ipi*$n)")
            read -r med mn mx <<< "$(mediana "${cmd[@]}")"
            local neto="$med"
            [ "$emu" = rars ] && neto=$(python3 -c "print(max($med-$base_rars, 0.001))")
            local mips
            mips=$(python3 -c "print(f'{$nominal/$neto/1e6:.1f}')")
            echo "$k,$emu,$n,$nominal,$med,$mn,$mx,$mips" | tee -a "$csv"
        done
    done
    echo; echo "CSV: $csv"
    echo "versiones:"
    "$EMURV_CLI" --version 2>/dev/null || true
    qemu-system-riscv32 --version | head -1
    spike --help 2>&1 | head -1 || true
    echo "RARS 1.6 ($(shasum -a 256 "$RARS_JAR" | cut -c1-12)…) / $(java -version 2>&1 | head -1)"
    sysctl -n machdep.cpu.brand_string; sw_vers -productVersion
}

case "${1:-}" in
    build) build ;;
    verify) verify ;;
    run) run ;;
    *) echo "uso: $0 build|verify|run" >&2; exit 1 ;;
esac
