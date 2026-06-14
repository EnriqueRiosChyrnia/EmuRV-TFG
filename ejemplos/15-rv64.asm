# =====================================================
# Ejemplo 15 — RV64 (enteros de 64 bits)
# IMPORTANTE: elegí «RV64 (64 bits)» en el selector de la barra
# ANTES de ensamblar (con RV32 estas instrucciones dan error).
#
# Muestra lo que agrega RV64 sobre RV32:
#   - registros de 64 bits (mirá t0/t2 en el panel: 16 dígitos hex)
#   - slli con desplazamiento de hasta 63 (valores que no caben en 32 bits)
#   - sd/ld: guardar y recuperar 8 bytes (doubleword)
#   - addiw: una instrucción «W» que opera en 32 bits y sign-extiende a 64
# Syscalls: 1 (imprimir entero), 4 (cadena), 10 (terminar)
# =====================================================

        .data
val:    .space 8                 # 8 bytes para un doubleword (val está alineado)
titulo: .asciiz "addiw (W, 32 bits) = "
salto:  .asciiz "\n"

        .text
main:
        # --- un valor que NO cabe en 32 bits: 2^40 + 0x7B ---
        li    t0, 1
        slli  t0, t0, 40         # t0 = 0x0000_0100_0000_0000 (2^40)
        addi  t0, t0, 0x7B       # t0 = 0x0000_0100_0000_007B

        # --- guardar y recuperar los 8 bytes (sd/ld) ---
        la    t1, val
        sd    t0, 0(t1)          # escribe el doubleword completo
        ld    t2, 0(t1)          # t2 == t0 (round-trip de 64 bits)

        # --- instrucción W: opera en 32 bits y sign-extiende el resultado ---
        li    t3, 100
        addiw t3, t3, -7         # t3 = 93

        # imprimir "addiw (W, 32 bits) = 93"
        la    a0, titulo
        li    a7, 4
        ecall
        mv    a0, t3
        li    a7, 1
        ecall
        la    a0, salto
        li    a7, 4
        ecall

        li    a7, 10             # terminar
        ecall
