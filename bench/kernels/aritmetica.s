# Kernel «aritmetica» — ALU pura en registros, cadena dependiente (RV32IM).
# bench_kernel: a0 = iteraciones → a0 = acumulador final (determinista).
#
# CUENTA NOMINAL: 10 instrucciones por iteración (cuerpo del bucle) +
# 4 de pre/post — nominal = 10*N + 4. Sintaxis común EmuRV/RARS/gas.

bench_kernel:
    li t0, 0                    # acumulador
    li t1, 0x9e3779b9           # constante de mezcla (golden ratio)
bench_loop_a:
    xor t0, t0, t1              # 1
    add t0, t0, a0              # 2
    slli t2, t0, 3              # 3
    add t0, t0, t2              # 4
    srli t2, t0, 7              # 5
    xor t0, t0, t2              # 6
    mul t2, t0, t1              # 7
    add t0, t0, t2              # 8
    addi a0, a0, -1             # 9
    bnez a0, bench_loop_a       # 10
    mv a0, t0
    jr ra
