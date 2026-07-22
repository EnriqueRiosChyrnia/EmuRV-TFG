# Kernel «memoria» — carga/almacenamiento sobre un arreglo de 4 KiB (RV32IM).
# bench_kernel: a0 = iteraciones → a0 = suma de control (determinista).
# Requiere el símbolo bench_buf (4096 bytes, inicializado a 0) definido por
# el wrapper.
#
# CUENTA NOMINAL: 9 instrucciones por iteración + 6 de pre/post —
# nominal = 9*N + 6.

bench_kernel:
    li t0, 0                    # suma de control
    li t3, 0                    # índice de byte (recorre 0..4092 y vuelve)
    li t1, 4092                 # máscara (andi no admite inmediato > 2047)
    la t4, bench_buf
bench_loop_m:
    add t5, t4, t3              # 1  dirección = base + índice
    lw t2, 0(t5)                # 2
    add t2, t2, a0              # 3
    sw t2, 0(t5)                # 4
    add t0, t0, t2              # 5
    addi t3, t3, 4              # 6
    and t3, t3, t1              # 7  envuelve dentro de los 4 KiB
    addi a0, a0, -1             # 8
    bnez a0, bench_loop_m       # 9
    mv a0, t0
    jr ra
