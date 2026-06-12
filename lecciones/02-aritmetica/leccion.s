# ==========================================================
# Lección 02 — Aritmética y lógica
# ==========================================================
# OBJETIVOS — al terminar esta lección vas a poder:
#   * sumar, restar, multiplicar y dividir en registros
#   * explicar el complemento a 2 (por qué 0x7FFFFFFF + 1
#     da un número NEGATIVO)
#   * usar and/or/xor para manipular bits
#   * distinguir srl (corrimiento lógico) de sra (aritmético)
# PANELES PARA OBSERVAR: «Registros» — la columna Decimal
#   muestra los números CON signo; la Hexadecimal, los bits
#   crudos. Compará las dos todo el tiempo.
# CÓMO USARLA: «Ensamblar» y «Paso», bloque por bloque.
# ----------------------------------------------------------

        .text
main:
# --- Suma, resta, multiplicación, división, resto ---------
        li   t0, 20
        li   t1, 6
        add  t2, t0, t1      # t2 = 26
        sub  t3, t0, t1      # t3 = 14
        mul  t4, t0, t1      # t4 = 120
        div  t5, t0, t1      # t5 = 3   (división ENTERA)
        rem  t6, t0, t1      # t6 = 2   (el resto de 20÷6)

# Imprimimos la división y el resto: "3 resto 2"
        mv   a0, t5
        li   a7, 1
        ecall
        la   a0, resto_txt
        li   a7, 4
        ecall
        mv   a0, t6
        li   a7, 1
        ecall
        li   a0, 10
        li   a7, 11
        ecall

# --- Complemento a 2: el experimento -----------------------
# 0x7FFFFFFF es el entero POSITIVO más grande de 32 bits
# (mirálo en decimal: 2147483647). Sumale 1 con un Paso y…
# ¡el panel muestra -2147483648! No es un bug: con 32 bits,
# el bit más alto es el SIGNO. Esto es complemento a 2.
        li   s0, 0x7FFFFFFF
        addi s0, s0, 1       # ¡mirá la columna Decimal AHORA!
        mv   a0, s0
        li   a7, 1
        ecall
        li   a0, 10
        li   a7, 11
        ecall

# --- Bits: and / or / xor ----------------------------------
# En hexadecimal cada dígito son 4 bits: F = 1111, 0 = 0000.
        li   s1, 0xF0
        li   s2, 0x3C
        and  s3, s1, s2      # 0x30: solo los bits que están en LOS DOS
        or   s4, s1, s2      # 0xFC: los bits que están en ALGUNO
        xor  s5, s1, s2      # 0xCC: los bits DIFERENTES
        mv   a0, s3
        li   a7, 1
        ecall
        li   a0, 32          # espacio
        li   a7, 11
        ecall
        mv   a0, s4
        li   a7, 1
        ecall
        li   a0, 32
        li   a7, 11
        ecall
        mv   a0, s5
        li   a7, 1
        ecall
        li   a0, 10
        li   a7, 11
        ecall

# --- srl vs sra: la diferencia que importa con negativos ---
# Correr a la derecha divide por 2… pero ¿qué entra por la
# izquierda? srli mete CEROS (lógico); srai COPIA EL SIGNO
# (aritmético). (La «i» final = la cantidad es un inmediato;
# sra/srl sin «i» esperan la cantidad en un registro.)
# Con -8: srai da -4 (correcto); srli, un positivo GIGANTE.
        li   s6, -8
        srai s7, s6, 1       # -4  ✓ división por 2 con signo
        srli s8, s6, 1       # 2147483644  ✗ ¡mirá el panel!
        mv   a0, s7
        li   a7, 1
        ecall
        li   a0, 10
        li   a7, 11
        ecall

        li   a7, 10
        ecall

        .data
resto_txt: .asciiz " resto "

# ----------------------------------------------------------
# EJERCICIOS (modificá el código y volvé a ensamblar):
#   1. Calculá 17 ÷ 5 cambiando t0 y t1: ¿qué imprime y por
#      qué el resto no es decimal?
#   2. ¿Qué da 0x80000000 - 1? Predecí el decimal ANTES de
#      mirar el panel (pista: es el espejo del experimento).
#   3. (desafío) Multiplicá s1 por 8 SIN usar mul (pista:
#      correr bits a la IZQUIERDA con slli multiplica por 2
#      en cada corrimiento).
# ==========================================================
