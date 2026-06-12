# ==========================================================
# Lección 09 — E/S proyectada en memoria y «tiempo real»
# ==========================================================
# OBJETIVOS — al terminar esta lección vas a poder:
#   * explicar qué es la E/S proyectada en memoria (MMIO):
#     direcciones que NO son memoria común, son DISPOSITIVOS
#   * pintar píxeles en la pantalla gráfica con sb
#   * leer el teclado por encuesta (polling) con el protocolo
#     CONTROL/DATA — y explicar por qué leer DATA CONSUME
#   * escribir un bucle de encuesta con límite de vueltas,
#     que termina solo aunque nadie toque una tecla
# PANELES PARA OBSERVAR: «Pantalla (320×240)» — ahí aparece
#   lo que pintás — y «Consola». En «Memoria» podés escribir
#   90000000 en el campo Dirección y apretar «Ir»: vas a VER
#   los bytes de los píxeles que pintaste. (Mirar 0xFFFF0000
#   desde el panel NO consume teclas: espiar es gratis.)
# CÓMO USARLA: «Ensamblar» y «Ejecutar» (esta lección es de
#   tiempo real: con «Paso» el reloj te gana; podés repasarla
#   a paso DESPUÉS). Mientras corre la encuesta (~1,5
#   segundos) hacé CLICK sobre la pantalla (aparece «Teclado
#   capturado») y tocá letras: cada tecla deja una marquita
#   de color. Si no tocás nada, el programa termina solo y
#   la Consola muestra SIEMPRE lo mismo.
# ----------------------------------------------------------

        .text
main:
# Hasta hoy, sw y lw (lección 03) hablaban con memoria común:
# lo que guardás queda quieto ahí, y no pasa nada más. Pero
# hay direcciones ESPECIALES donde del otro lado no hay
# memoria: hay un DISPOSITIVO. Leer o escribir ahí tiene
# EFECTOS (pintar un punto, sacar una tecla de una cola).
# Eso es la E/S proyectada en memoria (MMIO): las mismas
# instrucciones de siempre, pero el mapa de direcciones
# decide con QUIÉN estás hablando. Hoy usamos dos vecinos:
        li   s0, 0xFFFF_0000    # teclado: CONTROL en +0, DATA en +4
        li   s1, 0x9000_0000    # pantalla: framebuffer de 320×240

# Primero, un sw en el registro de control de video le avisa
# al emulador que este programa usa la pantalla (así la UI
# muestra el panel). Es un sw común… a una dirección especial.
        li   t0, 0x9003_0000
        li   t1, 1
        sw   t1, 0(t0)

# --- Pintar: la pantalla es un arreglo de bytes -------------
# Desde 0x90000000 hay 320×240 bytes, UN byte por píxel, y el
# valor del byte elige un color de la paleta (0 = negro,
# 1 = azul, 2 = verde, 3 = cian, 4 = rojo…). El píxel (x, y)
# vive en base + y*320 + x. Pintamos una rayita roja de 8
# píxeles en (10, 5): escribís el byte con sb… ¡y el punto
# APARECE en «Pantalla (320×240)»! Eso jamás pasa en la
# memoria de la lección 03: acá el byte ES el píxel.
        li   t1, 5              # y = 5
        li   t2, 320
        mul  t3, t1, t2         # t3 = y × 320
        addi t3, t3, 10         # … + x  (x = 10)
        add  t3, s1, t3         # t3 = dirección del píxel (10, 5)
        li   t4, 4              # color 4 = rojo
        li   t5, 8              # largo de la raya, en píxeles
raya:
        sb   t4, 0(t3)          # pintar UN píxel
        addi t3, t3, 1          # el vecino de la derecha
        addi t5, t5, -1
        bnez t5, raya

        la   a0, pintado        # mensaje FIJO por la Consola
        li   a7, 4
        ecall

# --- El teclado: protocolo CONTROL/DATA ---------------------
# El teclado son DOS palabras en memoria (igual que en RARS):
#   CONTROL (0xFFFF0000): el bit 0 vale 1 si hay una tecla
#       esperando. Leerlo es inocuo: preguntá mil veces.
#   DATA    (0xFFFF0004): el código de la tecla más vieja.
#   ¡LEERLO LA CONSUME! El dispositivo la saca de su cola:
#   por eso se lee UNA sola vez y se guarda en un registro.
# El plan: 30 vueltas de encuesta. En cada vuelta preguntamos
# si hay tecla; si hay, la consumimos y dejamos una marca de
# color en la fila 20 (la columna avanza tecla a tecla, y el
# color sale de los 3 bits bajos del código: +1 para que
# nunca dé 0, negro). Haya o no tecla, dormimos 50 ms con el
# servicio 32 (polling educado: sin dormir, el bucle daría
# millones de vueltas por segundo quemando CPU). 30 vueltas
# × 50 ms ≈ 1,5 segundos de encuesta y el programa TERMINA
# SOLO: sin teclas, la corrida es siempre idéntica.
        li   s2, 0              # columna de la próxima marca
        li   t0, 6400           # fila 20: 20 × 320
        add  s3, s1, t0         # s3 = comienzo de esa fila
        li   s4, 30             # vueltas que quedan
        li   s5, 0              # teclas atendidas

vuelta:
        lw   t0, 0(s0)          # t0 ← CONTROL
        andi t0, t0, 1          # nos quedamos con el bit 0
        beqz t0, dormir         # ¿0? no hay tecla: a dormir
        lw   t1, 4(s0)          # t1 ← DATA (¡esta lectura CONSUME!)
        addi s5, s5, 1          # una tecla más atendida
        andi t2, t1, 7          # 3 bits bajos del código (0..7)
        addi t2, t2, 1          # color 1..8: nunca negro
        add  t3, s3, s2         # dirección de la marca
        sb   t2, 0(t3)          # cuatro píxeles en columna:
        sb   t2, 320(t3)        # +320 = un píxel más ABAJO
        sb   t2, 640(t3)        # (¿te suena? base + y*320 + x)
        sb   t2, 960(t3)
        addi s2, s2, 1          # la próxima marca, a la derecha
dormir:
        li   a0, 50             # dormir 50 ms entre encuestas
        li   a7, 32
        ecall
        addi s4, s4, -1         # una vuelta menos…
        bnez s4, vuelta         # …y quedan s4: seguimos

# --- Cierre: concurrencia APARENTE --------------------------
# Releé lo que armaste: UN solo procesador que pinta, pregunta
# por el teclado, duerme y vuelve a empezar. No hay magia ni
# «hilos»: la sensación de que atiende todo A LA VEZ es
# concurrencia aparente — revisar el teclado entre pincelada
# y pincelada, tan rápido que nadie lo nota. Así funciona un
# juego. En el menú «Ejemplos» tenés cuatro para profundizar:
# «Franjas de color (pantalla)» y «Tetris gráfico» (solo
# dibujo), «Eco de teclado (tiempo real)» (este mismo polling,
# sin límite de vueltas) y «Tetris en tiempo real» (polling +
# gravedad por reloj con el servicio 30).
        la   a0, fin_txt
        li   a7, 4
        ecall
        mv   a0, s5             # sin teclas imprime 0, SIEMPRE
        li   a7, 1
        ecall
        li   a0, 10
        li   a7, 11
        ecall

        li   a7, 10
        ecall

        .data
pintado: .asciiz "raya roja pintada: mirá «Pantalla (320×240)»\n"
fin_txt: .asciiz "encuesta terminada, teclas atendidas: "

# ----------------------------------------------------------
# EJERCICIOS (modificá el código y volvé a ensamblar):
#   1. Pintá la raya VERDE (color 2) y de 20 píxeles de
#      largo. ¿Qué dos li tenés que tocar?
#   2. Llevá la raya al centro: x = 156, y = 120. Usá la
#      fórmula base + y*320 + x (son el li t1, 5 y el
#      addi t3, t3, 10 del cálculo).
#   3. (desafío) Eco por la «Consola»: si la tecla es
#      imprimible (código entre 32 y 126), imprimila con el
#      servicio 11 además de pintar la marca. Pista: dos
#      saltos blt/bgt como en la lección 04, justo después
#      de leer DATA; el ejemplo «Eco de teclado (tiempo
#      real)» lo hace completo. Ojo: tecleando cambia lo que
#      se imprime (¡obvio!), pero la corrida SIN teclas
#      tiene que seguir mostrando siempre lo mismo.
# ==========================================================
