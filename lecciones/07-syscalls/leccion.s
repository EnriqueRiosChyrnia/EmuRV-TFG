# ==========================================================
# Lección 07 — Syscalls y entrada/salida
# ==========================================================
# OBJETIVOS — al terminar esta lección vas a poder:
#   * explicar qué es una syscall (llamada al sistema) y
#     cómo se pide con a7 + ecall
#   * usar la tabla COMPLETA de servicios de EmuRV
#   * leer un número del usuario y VALIDARLO en un bucle
#     (si está fuera de rango, se vuelve a pedir)
#   * dormir con el servicio 32 y comprobar con el 30 que
#     el reloj de verdad avanzó
#   * leer TEXTO (servicio 8), pedirle memoria al sistema
#     (servicio 9) y salir con un código propio (93)
# PANELES PARA OBSERVAR: «Consola» — ahí aparecen tus
#   mensajes y, cuando el programa pide leer (servicio 5),
#   la Consola muestra un campo de entrada para que escribas
#   el número. También «Registros»: a0 y a7 son los
#   protagonistas de hoy.
# CÓMO USARLA: «Ensamblar» y después «Paso», bloque por
#   bloque. Cuando llegues al ecall del servicio 5, el
#   programa se queda esperando: escribí un número en el
#   campo de la Consola y confirmalo para que siga.
#   («Ejecutar» también sirve: corre solo hasta que la
#   Consola te pida la entrada.)
# ----------------------------------------------------------
# LA TABLA DE SERVICIOS DISPONIBLES (se elige con a7):
#   a7 = 1   imprimir entero      (el valor va en a0)
#   a7 = 4   imprimir cadena      (a0 = dirección del texto)
#   a7 = 5   leer entero          (devuelve el valor en a0)
#   a7 = 8   leer cadena          (a0 = buffer, a1 = tamaño)
#   a7 = 9   pedir memoria (sbrk) (a0 = bytes → a0 = dirección)
#   a7 = 10  salir del programa
#   a7 = 11  imprimir carácter    (el código ASCII va en a0)
#   a7 = 30  hora actual en ms    (a0 = parte baja, a1 = alta)
#   a7 = 32  dormir a0 milisegundos
#   a7 = 60  limpiar la consola   (extensión de EmuRV, no RARS)
#   a7 = 93  salir con código     (a0 = el código de salida)
# La receta es SIEMPRE la misma: argumentos en a0 (y a1 si
# hace falta), número de servicio en a7, y ecall = «sistema,
# hacé esto por mí». Es el mismo patrón desde la lección 01.
# ----------------------------------------------------------

        .text
main:
# --- El ciclo leer → validar → procesar ---------------------
# Casi todo programa interactivo hace esto: pide un dato,
# lo revisa, y si está mal lo vuelve a pedir. La etiqueta
# «pedir» es adonde saltamos cada vez que hay que reintentar.
pedir:
        la   a0, pregunta    # servicio 4: imprimir la pregunta
        li   a7, 4
        ecall

# Servicio 5: leer un entero. El programa SE FRENA acá hasta
# que escribas algo en el campo de la Consola. Lo que
# escribiste aparece en a0 — miralo en el panel de Registros.
        li   a7, 5
        ecall
        mv   s0, a0          # lo guardamos: a0 se pisa enseguida

# Validación con los saltos de la lección 04: si s0 < 1 o
# s0 > 10 está fuera de rango → avisamos y volvemos a pedir.
        li   t0, 1
        blt  s0, t0, fuera   # ¿menor que 1? → fuera
        li   t0, 10
        bgt  s0, t0, fuera   # ¿mayor que 10? → fuera
        j    valido          # pasó las dos pruebas: seguimos

fuera:
        la   a0, error_txt   # avisamos…
        li   a7, 4
        ecall
        j    pedir           # …y AL BUCLE: se pide de nuevo

# --- Procesar: imprimir el doble ----------------------------
# slli corre los bits 1 lugar a la izquierda = multiplicar
# por 2 (¿te acordás del desafío de la lección 02?).
valido:
        la   a0, doble_txt
        li   a7, 4
        ecall
        slli a0, s0, 1       # a0 = s0 × 2
        li   a7, 1           # servicio 1: imprimir entero
        ecall
        li   a0, 10          # salto de línea (servicio 11)
        li   a7, 11
        ecall

# --- El reloj: servicios 30 y 32 ----------------------------
# El servicio 30 devuelve la hora en milisegundos: parte
# baja en a0, parte alta en a1 (¡no entra en 32 bits!).
# Tomamos la hora ANTES de dormir y la guardamos.
        li   a7, 30
        ecall
        mv   s1, a0          # s1 = hora ANTES (parte baja)

# Servicio 32: dormir. a0 = cuántos milisegundos. El
# programa queda pausado 100 ms — un parpadeo.
        li   a0, 100
        li   a7, 32
        ecall

# Hora DESPUÉS, restamos, y comprobamos con bge que pasaron
# 100 ms o más. OJO: nunca imprimas la hora cruda — cambia
# en cada corrida. Por eso imprimimos un mensaje FIJO.
        li   a7, 30
        ecall
        sub  t0, a0, s1      # t0 = después − antes
        li   t1, 100
        bge  t0, t1, avanzo  # ¿pasaron ≥ 100 ms? → avisar
        j    fin             # (no debería pasar nunca)

avanzo:
        la   a0, reloj_txt
        li   a7, 4
        ecall

# --- Texto y memoria: servicios 8 y 9 -----------------------
# El servicio 9 (sbrk) le pide memoria al sistema: cargás en
# a0 CUÁNTOS bytes querés y te devuelve en a0 la DIRECCIÓN de
# tu bloque (un pedazo del «heap», que arranca en 0x10040000
# — buscalo en el panel «Memoria» después del ecall).
        li   a0, 32          # quiero 32 bytes
        li   a7, 9
        ecall
        mv   s2, a0          # s2 = mi buffer (¡a0 se pisa!)

# El servicio 8 lee una LÍNEA entera (no solo números): a0 =
# dónde guardarla, a1 = tamaño del buffer. Guarda a lo sumo
# a1−1 letras más un byte 0 final (como .asciiz): si escribís
# de más, corta solo. Y queda lista para imprimir con el 4.
        la   a0, nombre_txt
        li   a7, 4
        ecall
        mv   a0, s2          # leer adentro de MI bloque…
        li   a1, 32          # …que mide 32 bytes
        li   a7, 8
        ecall
        la   a0, hola_txt
        li   a7, 4
        ecall
        mv   a0, s2          # la cadena leída, como cualquier otra
        li   a7, 4
        ecall
        la   a0, cierre_txt
        li   a7, 4
        ecall

# Servicio 93: salir CON CÓDIGO (a0). El 10 siempre sale con
# 0; con el 93 vos elegís — en la terminal se ve con
# «echo $?» después de emurv-cli run. Hoy elegimos 0 (todo
# bien); en los ejercicios vas a devolver otros.
fin:
        li   a0, 0
        li   a7, 93
        ecall

        .data
pregunta:  .asciiz "Ingresá un número del 1 al 10: "
error_txt: .asciiz "Fuera de rango, probá de nuevo.\n"
doble_txt: .asciiz "El doble es "
reloj_txt: .asciiz "el reloj avanzó: dormimos 100 ms y se notó\n"
nombre_txt: .asciiz "¿Cómo te llamás? "
hola_txt:  .asciiz "¡Hola, "
cierre_txt: .asciiz "!\n"

# ----------------------------------------------------------
# EJERCICIOS (modificá el código y volvé a ensamblar):
#   1. Cambiá el rango a 1..100 (tocá el li t0, 10) y probá
#      con 50: ya no debería decir «fuera de rango».
#   2. Salí con el número elegido como código: mové s0 a a0
#      antes del servicio 93, corré por terminal y comprobalo
#      con «echo $?» (con 7 tiene que mostrar 7).
#   3. (desafío) Hacé que el programa imprima la tabla del
#      número elegido: «n, 2n, 3n… 10n» separados por
#      espacios. Pista: un contador que arranca en 1, un
#      mul (lección 02), y un bgt que corta cuando el
#      contador pasa de 10 — es el mismo bucle de validación
#      pero al revés: repetir MIENTRAS está en rango.
# ==========================================================
