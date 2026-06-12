# ==========================================================
# Lección 04 — Condicionales: saltos que deciden
# ==========================================================
# OBJETIVOS — al terminar esta lección vas a poder:
#   * explicar que en ensamblador NO existe el «if»: se
#     construye saltando POR ENCIMA de bloques de código
#   * usar beq/bne/blt/bge para comparar y saltar
#   * explicar por qué existen bltu/bgeu (comparar SIN
#     signo) — el complemento a 2 de la lección 02 vuelve
#   * armar un if/else completo: el máximo de dos números
# PANELES PARA OBSERVAR: «Registros» — esta vez la estrella
#   es el pc (el contador de programa: la dirección de la
#   PRÓXIMA instrucción). Con cada Paso normal avanza de a 4;
#   cuando un salto SE TOMA, pega un brinco. La «Consola» te
#   confirma qué rama se ejecutó.
# CÓMO USARLA: «Ensamblar» y «Paso», bloque por bloque, con
#   un ojo fijo en el pc del panel de Registros.
# ----------------------------------------------------------

        .text
main:
# En ensamblador no hay «if», «else» ni llaves. Hay SALTOS
# CONDICIONALES: comparan dos registros y, si la condición
# se cumple, cambian el pc. Si no se cumple, no pasa nada:
# el pc sigue de largo a la instrucción siguiente (+4).

# --- beq y bne: ¿iguales o distintos? -----------------------
        li   t0, 5
        li   t1, 7

# beq = "branch if equal": salta si son IGUALES. Como 5 ≠ 7,
# este salto NO se toma. Dale Paso mirando el pc en el panel
# de Registros: avanza de a 4, como cualquier instrucción.
        beq  t0, t1, gemelos

# bne = "branch if not equal", el opuesto. 5 ≠ 7 es VERDADERO,
# así que este SÍ se toma. Paso: ¡el pc brinca más de 4!
# Está saltando POR ENCIMA del bloque «gemelos».
        bne  t0, t1, distintos

# Este bloque quedó muerto: ningún camino llega acá. Fijate
# que la Consola JAMÁS muestra su texto.
gemelos:
        la   a0, nunca_txt
        li   a7, 4
        ecall

distintos:
        la   a0, distintos_txt
        li   a7, 4
        ecall

# --- Programa estrella: el máximo de dos números ------------
# En un lenguaje de alto nivel escribirías:
#   if (a >= b) { max = a; } else { max = b; }
# El truco en asm es saltar cuando la condición es FALSA:
# «a >= b» es falso justo cuando a < b, así que usamos
# blt = "branch if less than" (CON signo) para irnos al else.
        li   s0, 12          # a
        li   s1, 33          # b
        blt  s0, s1, else_max   # ¿a < b? 12 < 33: SÍ → al else
        mv   s2, s0          # rama del if: max = a
        j    fin_max         # j = salto INCONDICIONAL: siempre
                             # se toma. Sin él, caeríamos dentro
                             # del else y b pisaría el máximo.
else_max:
        mv   s2, s1          # rama del else: max = b
fin_max:

# (bge = "branch if greater or equal" es el espejo de blt:
# el mismo if/else sale saltando a la rama del if con bge.
# Lo vas a probar en el ejercicio 2.)

# La Consola muestra: "el máximo entre 12 y 33 es 33"
        la   a0, max_txt
        li   a7, 4
        ecall
        mv   a0, s2
        li   a7, 1
        ecall
        li   a0, 10
        li   a7, 11
        ecall

# --- La trampa: blt vs bltu ---------------------------------
# Acordate de la lección 02: los MISMOS 32 bits significan
# -1 con signo… o 4294967295 sin signo (0xFFFFFFFF, todos los
# bits en 1 — miralo en la columna Hexadecimal de s3).
        li   s3, -1
        li   s4, 5

# Con signo: -1 < 5 es VERDADERO → blt salta a s_menor.
        blt  s3, s4, s_menor
        la   a0, s_no_txt    # rama muerta (en este caso)
        li   a7, 4
        ecall
        j    prueba_u
s_menor:
        la   a0, s_si_txt
        li   a7, 4
        ecall

prueba_u:
# Sin signo: bltu lee los mismos bits como 4294967295, que NO
# es menor que 5 → NO salta y cae en la rama de abajo. Dale
# Paso y compará el pc con el blt de recién: mismo par de
# registros, decisión OPUESTA. Para esto existen bltu y bgeu:
# direcciones de memoria y tamaños nunca son negativos, y
# compararlos con signo daría cualquier cosa.
        bltu s3, s4, u_menor
        la   a0, u_no_txt    # cae acá: la rama "NO es menor"
        li   a7, 4
        ecall
        j    fin
u_menor:
        la   a0, u_si_txt
        li   a7, 4
        ecall

fin:
        li   a7, 10
        ecall

        .data
nunca_txt:     .asciiz "esto no se imprime nunca\n"
distintos_txt: .asciiz "5 y 7 son distintos\n"
max_txt:       .asciiz "el máximo entre 12 y 33 es "
s_si_txt:      .asciiz "blt (con signo): -1 es menor que 5\n"
s_no_txt:      .asciiz "blt (con signo): -1 NO es menor que 5\n"
u_si_txt:      .asciiz "bltu (sin signo): -1 es menor que 5\n"
u_no_txt:      .asciiz "bltu (sin signo): -1 NO es menor que 5 (son 4294967295)\n"

# ----------------------------------------------------------
# EJERCICIOS (modificá el código y volvé a ensamblar):
#   1. Cambiá a = 50 (li s0, 50) y verificá que el máximo
#      ahora es 50: el blt ya NO salta y se ejecuta la rama
#      del if. Mirá el pc justo en ese Paso.
#   2. Reescribí el máximo usando bge en vez de blt: saltá a
#      la rama del if cuando a >= b. La Consola tiene que
#      mostrar exactamente lo mismo.
#   3. (desafío) Extendé el if/else para que también guarde
#      el MÍNIMO en s3 y lo imprima en otra línea. Pista: no
#      hace falta comparar de nuevo — cada rama ya sabe quién
#      es quién: donde max = a, el mínimo es b (y al revés).
# ==========================================================
