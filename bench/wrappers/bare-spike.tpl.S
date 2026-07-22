# Wrapper bare-metal para Spike (RT-4): el resultado va a la signature
# (+signature de Spike la vuelca) y la salida es por HTIF tohost.
# @ITER@ lo sustituye scripts/bench-rt4.sh; el kernel se concatena debajo.

.globl _start

.text
_start:
    la sp, bench_stack_top      # bare metal: la pila no viene inicializada
    li a0, @ITER@
    call bench_kernel
    la t0, begin_signature
    sw a0, 0(t0)
    fence
    li t0, 1                    # tohost = 1 → exit(0)
    la t1, tohost
    sw t0, 0(t1)
1:  j 1b

.data
bench_buf: .space 4096
.align 4
bench_stack: .space 4096
bench_stack_top:
.align 4
.globl begin_signature
begin_signature: .word 0
.globl end_signature
end_signature:

.section .tohost,"aw",@progbits
.align 3
.globl tohost
tohost:   .dword 0
.globl fromhost
fromhost: .dword 0

# --- kernel concatenado debajo por el script ---
