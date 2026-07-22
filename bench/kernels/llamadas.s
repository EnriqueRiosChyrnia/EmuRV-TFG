# Kernel «llamadas» — call/ret con uso de pila (RV32IM).
# bench_kernel: a0 = iteraciones → a0 = acumulador (determinista).
#
# CUENTA NOMINAL: 5 instrucciones por iteración en el bucle (mv/jal/add/
# addi/bnez) + 8 por llamada a bench_hoja = 13 por iteración; prólogo 4 +
# epílogo 5 — nominal = 13*N + 9.

bench_kernel:
    addi sp, sp, -8
    sw ra, 4(sp)
    sw s0, 0(sp)
    li s0, 0                    # acumulador
bench_loop_c:
    mv a1, a0                   # 1
    jal bench_hoja              # 2
    add s0, s0, a1              # 3
    addi a0, a0, -1             # 4
    bnez a0, bench_loop_c       # 5
    mv a0, s0
    lw s0, 0(sp)
    lw ra, 4(sp)
    addi sp, sp, 8
    jr ra

# Hoja: 8 instrucciones incluida la vuelta (prólogo/epilogo de pila reales).
bench_hoja:
    addi sp, sp, -8             # 1
    sw a1, 4(sp)                # 2
    slli a1, a1, 1              # 3
    xor a1, a1, a0              # 4
    lw t2, 4(sp)                # 5
    add a1, a1, t2              # 6
    addi sp, sp, 8              # 7
    jr ra                       # 8
