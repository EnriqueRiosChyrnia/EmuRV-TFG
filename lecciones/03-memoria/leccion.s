# ==========================================================
# Lección 03 — Memoria: .data, cargas y almacenamientos
# ==========================================================
# OBJETIVOS — al terminar esta lección vas a poder:
#   * declarar datos con .word, .byte y .asciiz
#   * leer y escribir memoria con lw/sw y lb/lbu/sb
#   * explicar qué es little-endian VIENDO los bytes
#   * navegar el panel de Memoria hasta .data
# PANELES PARA OBSERVAR: «Memoria» — apretá el botón «.data»
#   para saltar a 0x10010000, donde viven tus variables. El
#   panel muestra cada byte en hexadecimal y su ASCII al lado.
# CÓMO USARLA: «Ensamblar», apretá «.data» en el panel de
#   Memoria, y después «Paso» mirando cómo cambian los bytes.
# ----------------------------------------------------------

        .data
numero:   .word  0x11223344    # 4 bytes juntos (una "palabra")
chico:    .byte  7             # 1 byte solo
texto:    .asciiz "hola"       # 5 bytes: h,o,l,a y el 0 final
espacio:  .word  0             # lugar para escribir después

        .text
main:
# --- Leer una palabra ---------------------------------------
# la carga la DIRECCIÓN; lw trae los 4 BYTES que viven ahí.
        la   t0, numero
        lw   t1, 0(t0)       # t1 = 0x11223344

# --- Little-endian: el experimento --------------------------
# Mirá el panel de Memoria en 0x10010000: los bytes aparecen
# como 44 33 22 11 — ¡el byte BAJO va primero! Eso es
# "little-endian". Ahora leamos byte por byte con lbu:
        lbu  t2, 0(t0)       # t2 = 0x44 (68)  ← el PRIMERO en memoria
        lbu  t3, 3(t0)       # t3 = 0x11 (17)  ← el último
        mv   a0, t2
        li   a7, 1
        ecall
        li   a0, 32
        li   a7, 11
        ecall
        mv   a0, t3
        li   a7, 1
        ecall
        li   a0, 10
        li   a7, 11
        ecall

# --- lb vs lbu: el signo otra vez ---------------------------
# lb extiende el SIGNO del byte (un byte ≥ 0x80 se vuelve
# negativo); lbu lo trata sin signo. Con 0x44 dan lo mismo;
# con bytes altos, no. Acordate de la lección 02.

# --- Escribir memoria: sw y sb ------------------------------
# sw guarda los 4 bytes de un registro en la dirección dada.
# Dale Paso y MIRÁ el panel de Memoria: los bytes de
# «espacio» cambian a 2A 00 00 00 (42 en little-endian).
        li   t4, 42
        la   t5, espacio
        sw   t4, 0(t5)

# Y lo volvemos a leer para demostrar que quedó guardado:
        lw   a0, 0(t5)
        li   a7, 1
        ecall
        li   a0, 10
        li   a7, 11
        ecall

# --- Cadenas: la memoria también es texto -------------------
# «texto» son bytes ASCII: 0x68='h'… El servicio 4 los
# imprime hasta encontrar el byte 0 (por eso .asciiz lo
# agrega solo). Buscá la 'h' (68) en el panel de Memoria.
        la   a0, texto
        li   a7, 4
        ecall
        li   a0, 10
        li   a7, 11
        ecall

        li   a7, 10
        ecall

# ----------------------------------------------------------
# EJERCICIOS (modificá el código y volvé a ensamblar):
#   1. Cambiá «numero» a 0xAABBCCDD y predecí QUÉ byte va a
#      imprimir cada lbu antes de correr. Ojo: 0xDD con lb
#      (con signo) daría negativo, con lbu no — probá los dos.
#   2. Escribí tu año de nacimiento en «espacio» y leelo.
#   3. (desafío) Cambiá la 'h' de «texto» por 'H' usando
#      li + sb (la H mayúscula es el código 72) y volvé a
#      imprimir la cadena: debería decir "Hola".
# ==========================================================
