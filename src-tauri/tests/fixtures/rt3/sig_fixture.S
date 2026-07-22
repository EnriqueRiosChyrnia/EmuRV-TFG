.section .text.init
.globl _start
_start:
  la t0, begin_signature
  li t1, 0xdeadbeef
  sw t1, 0(t0)
  li t1, 0xcafe1234
  sw t1, 4(t0)
  ebreak

.data
.align 4
.global begin_signature
begin_signature:
  .word 0, 0
.global end_signature
end_signature:
