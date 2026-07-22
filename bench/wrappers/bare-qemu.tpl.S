# Wrapper bare-metal para qemu-system-riscv32 -M virt (RT-4).
# brew qemu en macOS NO trae los emuladores user-mode (son Linux-only):
# se usa system-mode con SEMIHOSTING — SYS_WRITE0 para imprimir el
# resultado decimal y SYS_EXIT para terminar qemu.
# @ITER@ lo sustituye scripts/bench-rt4.sh; el kernel se concatena debajo.

.globl _start

.data
bench_buf: .space 4096
conv_buf:  .space 13
.align 4
bench_stack: .space 4096
bench_stack_top:

.text
# Llamada de semihosting RISC-V: secuencia mágica exacta (norvc).
.macro SEMIHOST
    .option push
    .option norvc
    slli zero, zero, 0x1f
    ebreak
    srai zero, zero, 7
    .option pop
.endm

_start:
    la sp, bench_stack_top      # bare metal: la pila no viene inicializada
    li a0, @ITER@
    call bench_kernel

    # itoa decimal sin signo hacia atrás + '\n' + NUL (SYS_WRITE0 pide cadena NUL)
    la t0, conv_buf+12
    sb zero, 0(t0)
    addi t0, t0, -1
    li t2, 10
    sb t2, 0(t0)                # '\n'
    li t3, 10
1:  addi t0, t0, -1
    remu t2, a0, t3
    addi t2, t2, 48
    sb t2, 0(t0)
    divu a0, a0, t3
    bnez a0, 1b

    # SYS_WRITE0 (0x04): a1 = cadena NUL-terminada
    mv a1, t0
    li a0, 0x04
    SEMIHOST

    # SYS_EXIT (0x18): a1 = ADP_Stopped_ApplicationExit
    li a0, 0x18
    li a1, 0x20026
    SEMIHOST
1:  j 1b

# --- kernel concatenado debajo por el script ---
