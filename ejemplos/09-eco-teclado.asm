# =====================================================
# Ejemplo 9 — Eco de teclado (dispositivo MMIO)
#
# Primera toma de contacto con la entrada por DISPOSITIVO:
# en vez de pedirle datos al sistema con una syscall, el
# programa habla directo con el "hardware" del teclado a
# través de dos registros mapeados en memoria (MMIO), igual
# que en RARS/MARS:
#
#   RECEIVER_CONTROL (0xFFFF_0000): el bit 0 vale 1 cuando
#       hay una tecla esperando ser leída.
#   RECEIVER_DATA    (0xFFFF_0004): tiene el código de la
#       tecla más vieja; LEERLO LA CONSUME (el dispositivo
#       la saca de su cola y, si no quedan más, CONTROL
#       vuelve a 0).
#
# El programa hace POLLING (encuesta): le pregunta al
# dispositivo una y otra vez "¿hay tecla?" hasta que la
# respuesta es sí. Cada tecla recibida dibuja un bloque de
# color en la pantalla gráfica y, si es un carácter que la
# consola puede mostrar (imprimible o Enter), también se
# imprime como eco. La tecla Esc (código 27) termina.
# Syscalls: 11 (imprimir carácter), 32 (dormir), 10 (terminar)
# =====================================================

        .text
main:
        li   s0, 0xFFFF_0000    # s0 ← base del teclado MMIO (CONTROL en +0, DATA en +4)
        li   s1, 0x9000_0000    # s1 ← base del framebuffer (pantalla de 320x240)
        li   s2, 0              # s2 ← columna (en píxeles) del próximo bloque

        # Avisarle al emulador que este programa usa la pantalla:
        # se escribe 1 en el registro de control de video.
        li   t0, 0x9003_0000    # registro de control de video
        li   t1, 1
        sw   t1, 0(t0)          # 1 = "voy a dibujar en pantalla"

# ------------------- bucle de POLLING -------------------
# Primero se PREGUNTA (leer CONTROL) y solo si NO hay tecla se
# duerme. El orden importa: si hay teclas encoladas se atienden
# todas de corrido, sin pausas artificiales en el medio.
poll:
        lw   t0, 0(s0)          # t0 ← RECEIVER_CONTROL (bit 0 = ¿hay tecla?)
        bnez t0, hay_tecla      # ¿bit 0 en 1? → hay dato listo, ir a leerlo

        # No hay tecla. Si volviéramos a preguntar de inmediato, este
        # bucle giraría millones de veces por segundo quemando CPU al
        # 100% sin hacer nada útil (espera activa pura). Dormir 10 ms
        # entre preguntas no se nota para una persona tipeando y deja
        # descansar al procesador.
        li   a0, 10             # a0 ← milisegundos a dormir
        li   a7, 32             # servicio 32: dormir a0 ms
        ecall
        j    poll               # al despertar, preguntar de nuevo

# ------------------- atender la tecla -------------------
hay_tecla:
        lw   s3, 4(s0)          # s3 ← RECEIVER_DATA. ¡Esta lectura CONSUME
                                # la tecla! Por eso se lee UNA sola vez y se
                                # guarda en un registro para usarla después.

        li   t0, 27             # 27 = código ASCII de la tecla Esc
        beq  s3, t0, fin        # con Esc se termina el programa

        # Eco SOLO de lo que la consola puede mostrar: los caracteres
        # imprimibles (códigos 32 a 126) y el Enter (10, salto de línea).
        # Las demás teclas —flechas (0x80-0x83), Backspace (8), etc.—
        # mandarían caracteres de control que ensucian la consola, así
        # que NO se imprimen; igual dibujan su bloque de color abajo.
        li   t0, 10
        beq  s3, t0, eco        # Enter sí: imprime el salto de línea
        li   t0, 32
        blt  s3, t0, dibujar    # código < 32 → control: sin eco
        li   t0, 126
        bgt  s3, t0, dibujar    # código > 126 → no imprimible: sin eco
eco:
        mv   a0, s3             # a0 ← código de la tecla recibida
        li   a7, 11             # servicio 11: imprimir carácter (el "eco")
        ecall

        # Color del bloque a partir del código de la tecla:
        # (código & 7) + 1 da un valor entre 1 y 8 — nunca 0,
        # porque el color 0 de la paleta es negro y no se vería.
dibujar:
        andi t0, s3, 7          # t0 ← los 3 bits bajos del código (0..7)
        addi t0, t0, 1          # t0 ← color final (1..8)

        # Dibujar un bloque de 8x8 píxeles en la fila superior de la
        # pantalla, en la columna s2. La pantalla es lineal: el píxel
        # (x, y) vive en base + y*320 + x (un byte de color por píxel).
        add  t1, s1, s2         # t1 ← esquina superior izquierda del bloque
        li   t2, 8              # t2 ← filas del bloque que faltan
fila:
        mv   t4, t1             # t4 ← puntero al primer píxel de esta fila
        li   t3, 8              # t3 ← píxeles que faltan en esta fila
pixel:
        sb   t0, 0(t4)          # pintar un píxel con el color elegido
        addi t4, t4, 1          # avanzar al píxel de la derecha
        addi t3, t3, -1
        bnez t3, pixel          # ¿quedan píxeles en la fila? → seguir
        addi t1, t1, 320        # bajar una fila (la pantalla mide 320 de ancho)
        addi t2, t2, -1
        bnez t2, fila           # ¿quedan filas? → seguir

        # El próximo bloque va 8 píxeles a la derecha; al llegar al
        # borde derecho (columna 320) se vuelve a empezar desde 0.
        addi s2, s2, 8
        li   t0, 320
        blt  s2, t0, poll       # todavía entra en la fila → esperar otra tecla
        li   s2, 0              # se llenó la fila: arrancar de nuevo
        j    poll

# ------------------------ fin ---------------------------
fin:
        li   a7, 10             # servicio 10: terminar el programa
        ecall
