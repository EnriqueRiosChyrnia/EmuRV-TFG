# Wrapper EmuRV/RARS (RT-4) — EL MISMO fuente corre en ambos: los syscalls
# de EmuRV son compatibles RARS a propósito (print_int=1, exit=10).
# @ITER@ lo sustituye scripts/bench-rt4.sh; el kernel se concatena debajo.

.data
bench_buf: .space 4096

.text
main:
    li a0, @ITER@
    jal bench_kernel
    li a7, 1                    # print_int(a0)
    ecall
    li a7, 10                   # exit(0)
    ecall

# --- kernel concatenado debajo por el script ---
