# ==========================================================
# Lección 05 — Bucles: repetir sin copiar y pegar
# ==========================================================
# OBJETIVOS — al terminar esta lección vas a poder:
#   * armar un bucle con una etiqueta y un salto hacia ATRÁS
#     (ble/blt): el while/for del ensamblador
#   * usar un contador (t0) y un acumulador que crece vuelta
#     a vuelta
#   * anidar un bucle adentro de otro sin perderte
#   * frenar en LA VUELTA QUE QUIERAS con un breakpoint
#     condicional, en vez de apretar «Paso» cincuenta veces
# PANELES PARA OBSERVAR: «Registros» (t0 sube de a 1 en cada
#   vuelta; t1 acumula la suma), «Depuración» (ahí viven los
#   breakpoints y sus condiciones) y la «Consola» al final.
# CÓMO USARLA: «Ensamblar» y «Paso» las primeras vueltas del
#   bucle, hasta que el ida-y-vuelta se ponga aburrido. AHÍ
#   está la gracia: seguí los 4 pasos del bloque «EL TRUCO»
#   para que la máquina corra sola y frene justo donde vos
#   quieras.
# ----------------------------------------------------------

        .text
main:
# Un bucle es un salto hacia ATRÁS: una etiqueta marca el
# comienzo y una instrucción de salto decide, al final, si
# volver o seguir. Vamos a sumar 1+2+…+10 con tres registros:
# t0 el contador (1, 2, 3…), t1 el acumulador (arranca en 0)
# y t2 el tope. Tres Pasos y los ves cargados en el panel.
        li   t0, 1
        li   t1, 0
        li   t2, 10

# --- EL TRUCO: frenar SOLO en la vuelta 5 -------------------
# Antes de seguir a los Pasos, prepará esto:
#   1. Click en el margen izquierdo del editor (a la
#      izquierda del número de línea) sobre la línea del
#      cuerpo, «add t1, t1, t0»: aparece un punto rojo.
#   2. Abrí el panel «Depuración»: el breakpoint figura en
#      la lista «Breakpoints» con su línea y un campo
#      «Condición».
#   3. Escribí en ese campo: t0 == 5
#      (registro, operador, valor — el panel la valida al
#      escribirla; si te equivocás, el error aparece ahí).
#   4. «Reiniciar» y «Ejecutar»: el programa corre solo y
#      frena UNA única vez — «Detenido en breakpoint» —
#      justo cuando t0 vale 5. Mirá «Registros»: t0 = 5 y
#      t1 = 10 (la suma 1+2+3+4; el cuerpo de la vuelta 5
#      todavía no se ejecutó). Las vueltas con t0 distinto
#      de 5 pasaron de largo sin frenar.
# Después sacá el punto rojo (otro click) y seguí con
# «Ejecutar» hasta el final.
vuelta:
        add  t1, t1, t0      # cuerpo: t1 ← t1 + t0
        addi t0, t0, 1       # contador + 1

# ble = "branch if less or equal": si t0 ≤ t2, el pc salta
# hacia ATRÁS, a «vuelta». Si no, sigue de largo: el bucle
# terminó. Es un while (t0 <= 10) hecho a mano. Con «Paso»,
# mirá el vaivén: t1 pasa por 1, 3, 6, 10, 15…
        ble  t0, t2, vuelta

# Al salir, t0 vale 11 (por eso la condición falló) y t1, 55.
# Lo anunciamos: cadena, número y salto de línea.
        la   a0, suma_txt
        li   a7, 4
        ecall
        mv   a0, t1
        li   a7, 1
        ecall                # la Consola muestra: suma 1..10 = 55
        li   a0, 10
        li   a7, 11
        ecall

# --- Bucle ANIDADO: la tabla del 3 --------------------------
# Un bucle adentro de otro: el EXTERNO recorre las filas
# (s1 = 1…5) y el INTERNO calcula 3 × s1 sin usar mul,
# sumando 3 una vez por cada unidad de s1 (multiplicar ES
# sumar repetido).
        li   s0, 3           # el número de la tabla
        li   s1, 1           # fila actual
fila:
# El interno arranca DE CERO en cada fila: s2 acumula el
# producto y s3 cuenta las sumas. Mirá s2 en «Registros»:
# en la fila 4 va a pasar por 3, 6, 9 y 12.
        li   s2, 0
        li   s3, 0
sumar:
        add  s2, s2, s0
        addi s3, s3, 1
        blt  s3, s1, sumar   # interno: ¿ya sumé s1 veces?

# Imprimimos la fila completa, «3 x 2 = 6»: número, " x ",
# fila, " = ", producto y salto de línea.
        mv   a0, s0
        li   a7, 1
        ecall
        la   a0, por_txt
        li   a7, 4
        ecall
        mv   a0, s1
        li   a7, 1
        ecall
        la   a0, igual_txt
        li   a7, 4
        ecall
        mv   a0, s2
        li   a7, 1
        ecall
        li   a0, 10
        li   a7, 11
        ecall

# El externo decide si hay otra fila. Fijate con «Paso» que
# este salto vuelve MÁS atrás que el del interno: hasta
# «fila», donde el interno renace con sus li en cero.
        addi s1, s1, 1
        li   s4, 5
        ble  s1, s4, fila    # externo: ¿ya van 5 filas?

        li   a7, 10
        ecall

        .data
suma_txt:  .asciiz "suma 1..10 = "
por_txt:   .asciiz " x "
igual_txt: .asciiz " = "

# ----------------------------------------------------------
# EJERCICIOS (modificá el código y volvé a ensamblar):
#   1. Subí el tope t2 a 100 y predecí la suma ANTES de
#      correr (Gauss la calculó a los 9 años: 5050). Usá el
#      breakpoint con condición t0 == 50 para espiar el
#      acumulador a mitad de camino.
#   2. Convertí la tabla del 3 en la tabla del 7 con 10
#      filas: solo hay que tocar DOS números (¿cuáles?).
#   3. (desafío) Calculá 5! (5×4×3×2×1 = 120) con UN solo
#      bucle e imprimilo. Pista: el acumulador arranca en 1
#      —no en 0— y el cuerpo usa mul en vez de add.
# ==========================================================
