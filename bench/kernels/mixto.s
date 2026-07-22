# Kernel «mixto» — mezcla realista: memoria + ALU + mul + saltos (RV32IM).
# Caminata pseudoaleatoria (LCG) sobre un arreglo de 4 KiB.
# bench_kernel: a0 = iteraciones → a0 = suma de control (determinista).
# Requiere bench_buf (4096 bytes, inicializado a 0) del wrapper.
#
# CUENTA NOMINAL: 11 instrucciones por iteración + 7 de pre/post —
# nominal = 11*N + 7.

bench_kernel:
    li t0, 0                    # suma de control
    li t3, 12345                # semilla LCG
    li t6, 1103515245           # multiplicador LCG (glibc)
    li t1, 4092                 # máscara (andi no admite inmediato > 2047)
    la t4, bench_buf
bench_loop_x:
    mul t3, t3, t6              # 1  LCG: semilla *= a
    addi t3, t3, 1013           # 2       += c (impar, cabe en imm12)
    srli t5, t3, 16             # 3  tomar bits altos
    and t5, t5, t1              # 4  índice alineado dentro de 4 KiB
    add t5, t4, t5              # 5
    lw t2, 0(t5)                # 6
    add t2, t2, t3              # 7
    sw t2, 0(t5)                # 8
    add t0, t0, t2              # 9
    addi a0, a0, -1             # 10
    bnez a0, bench_loop_x       # 11
    mv a0, t0
    jr ra
