# =====================================================
# Ejemplo 10 — Tetris en TIEMPO REAL (teclado MMIO + reloj)
#
# Mismo motor que el Ejemplo 8 (port del Tetris 8086): el
# tablero de 20x10, los bitmaps de las piezas, el generador
# LCG, las colisiones, el fijado, las líneas, el puntaje y
# el dibujo al framebuffer son IDÉNTICOS (las rutinas están
# copiadas tal cual). Lo que cambia es el bucle principal:
#
#   * Ejemplo 8: por turnos. La syscall 5 BLOQUEA el
#     programa hasta que el usuario tipea rotación/columna.
#   * Este ejemplo: tiempo real. El programa nunca espera:
#     cada vuelta PREGUNTA si hay una tecla (polling sobre
#     el teclado MMIO) y sigue de largo si no la hay.
#
# El teclado se ve como dos registros mapeados en memoria
# (mismo esquema que RARS/MARS):
#   RECEIVER_CONTROL (0xFFFF_0000): bit 0 = hay tecla esperando
#   RECEIVER_DATA    (0xFFFF_0004): leerlo devuelve la tecla
#                                   más vieja y LA CONSUME
#
# Controles (cada tecla con su código, a la vista):
#   a (97)  / A (65) / flecha ← (0x80): mover a la izquierda
#   d (100) / D (68) / flecha → (0x81): mover a la derecha
#   w (119) / W (87) / flecha ↑ (0x82): rotar
#   s (115) / S (83) / flecha ↓ (0x83): caída suave (bajar ya)
#   Esc (27): salir
#
# La gravedad usa el reloj de verdad: la syscall 30 da los
# milisegundos actuales y, cuando pasaron `gravedad_ms` desde
# el último descenso, la pieza baja sola un paso. Para no
# quemar CPU dando vueltas vacías, cada vuelta sin eventos
# duerme ~15 ms (syscall 32): polling sí, pero educado.
# =====================================================

        .data
tablero:    .space 200          # 20 filas x 10 columnas, 0 = vacío
semilla:    .word 0x1234        # misma semilla que el Ejemplo 8 (y el 8086)
score:      .word 0

# Gravedad por reloj: cada cuántos milisegundos baja sola la pieza.
# Está en .data (y no como constante dentro del código) a propósito:
# la dificultad se ajusta sin tocar una sola instrucción.
gravedad_ms:     .word 500
# Momento del último descenso (bits bajos de la syscall 30, en ms).
ultimo_descenso: .word 0

# Bitmaps 4x4 (bit 15 = celda superior izquierda, por filas).
# Mismos valores que el original: 4 rotaciones por pieza.
piezas:     .word 0xF000, 0x8888, 0xF000, 0x8888    # I
            .word 0xCC00, 0xCC00, 0xCC00, 0xCC00    # O
            .word 0xE400, 0x4C40, 0x4E00, 0x8C80    # T
            .word 0x6C00, 0x8C40, 0x6C00, 0x8C40    # S
            .word 0xC600, 0x4C80, 0xC600, 0x4C80    # Z
            .word 0x44C0, 0x8E00, 0xC880, 0xE200    # J
            .word 0x88C0, 0xE800, 0xC440, 0x2E00    # L

# Puntos por cantidad de líneas eliminadas de una vez (índice 0..4)
puntos:     .word 0, 100, 300, 500, 800

nombres:    .asciiz "IOTSZJL"
txt_titulo: .asciiz "=== TETRIS RV32 en tiempo real ===\n"
txt_ayuda:  .asciiz "A/D o flechas: mover | W: rotar | S: bajar | Esc: salir\n"
txt_pieza:  .asciiz "Pieza: "
txt_score:  .asciiz "SCORE: "
txt_gover:  .asciiz "¡GAME OVER! El tablero llegó al tope.\n"
txt_fin:    .asciiz "Fin del juego.\n"

        .text
# Registros del estado del juego (misma convención que el Ejemplo 8):
#   s0 = tipo de pieza   s1 = rotación   s2 = columna   s3 = fila
#   s4 = hubo evento en esta vuelta (1 = tecla aplicada o descenso)
main:
        la   a0, txt_titulo
        li   a7, 4
        ecall
        la   a0, txt_ayuda
        li   a7, 4
        ecall

        # El reloj de la gravedad arranca AHORA: si dejáramos el 0
        # inicial de `ultimo_descenso`, la primera resta daría un
        # número gigante y la pieza bajaría de entrada sin esperar.
        li   a7, 30
        ecall
        sw   a0, ultimo_descenso, t0

nueva_pieza:
        call random_pieza       # a0 <- tipo (0..6), mismo LCG del Ejemplo 8
        mv   s0, a0
        li   s1, 0              # rotación inicial
        li   s2, 3              # columna inicial: el centro del tablero
        li   s3, 0              # la pieza entra por la fila 0

        # Si la pieza nueva ya choca apenas nace, el tablero llegó
        # al tope: no hay lugar ni para empezar a caer.
        li   a0, 0
        mv   a1, s2
        mv   a2, s0
        mv   a3, s1
        call colisiona
        bnez a0, game_over

        la   a0, txt_pieza      # "Pieza: X" en la consola
        li   a7, 4
        ecall
        la   t0, nombres
        add  t0, t0, s0
        lbu  a0, 0(t0)
        li   a7, 11
        ecall
        li   a0, '\n'
        ecall
        call redibujar

# ================ BUCLE PRINCIPAL (tiempo real) ================
# Cada vuelta hace tres cosas y NUNCA se queda bloqueada:
#   1) polling del teclado MMIO: ¿hay tecla? se procesa; si no, sigo
#   2) gravedad por reloj: ¿venció el plazo? la pieza baja un paso
#   3) si la vuelta fue vacía, dormir ~15 ms para no quemar CPU
bucle:
        li   s4, 0              # arranca la vuelta: sin eventos todavía

        # --- 1) teclado: polling NO bloqueante --------------------
        # CONTROL (bit 0) avisa si hay una tecla en la cola. Esta
        # lectura no espera nada: si da 0 seguimos jugando. Compará
        # con la syscall 5 del Ejemplo 8, que congela el programa
        # hasta que el usuario tipee algo.
        li   t0, 0xFFFF_0000
        lw   t1, 0(t0)          # RECEIVER_CONTROL
        beqz t1, gravedad       # sin tecla: directo a la gravedad
        lw   t2, 4(t0)          # RECEIVER_DATA: leerla LA CONSUME

        # Tabla de despacho: el código de cada tecla, a la vista.
        li   t3, 27             # Esc: salir limpio
        beq  t2, t3, despedida
        li   t3, 'a'            # 97
        beq  t2, t3, mover_izq
        li   t3, 'A'            # 65
        beq  t2, t3, mover_izq
        li   t3, 0x80           # flecha izquierda
        beq  t2, t3, mover_izq
        li   t3, 'd'            # 100
        beq  t2, t3, mover_der
        li   t3, 'D'            # 68
        beq  t2, t3, mover_der
        li   t3, 0x81           # flecha derecha
        beq  t2, t3, mover_der
        li   t3, 'w'            # 119
        beq  t2, t3, rotar
        li   t3, 'W'            # 87
        beq  t2, t3, rotar
        li   t3, 0x82           # flecha arriba
        beq  t2, t3, rotar
        li   t3, 's'            # 115
        beq  t2, t3, bajar
        li   t3, 'S'            # 83
        beq  t2, t3, bajar
        li   t3, 0x83           # flecha abajo
        beq  t2, t3, bajar
        j    gravedad           # cualquier otra tecla se ignora

# Cada movimiento se prueba primero sobre la posición hipotética con
# `colisiona` (la MISMA rutina del Ejemplo 8): si choca, la tecla se
# ignora y la pieza se queda donde estaba.
mover_izq:
        mv   a0, s3
        addi a1, s2, -1         # ¿entra una columna a la izquierda?
        mv   a2, s0
        mv   a3, s1
        call colisiona
        bnez a0, gravedad
        addi s2, s2, -1
        li   s4, 1
        call redibujar
        j    gravedad

mover_der:
        mv   a0, s3
        addi a1, s2, 1          # ¿entra una columna a la derecha?
        mv   a2, s0
        mv   a3, s1
        call colisiona
        bnez a0, gravedad
        addi s2, s2, 1
        li   s4, 1
        call redibujar
        j    gravedad

rotar:
        mv   a0, s3
        mv   a1, s2
        mv   a2, s0
        addi a3, s1, 1
        andi a3, a3, 3          # la rotación da la vuelta: 3 -> 0
        call colisiona
        bnez a0, gravedad
        addi s1, s1, 1
        andi s1, s1, 3
        li   s4, 1
        call redibujar
        j    gravedad

bajar:
        # Caída suave: el MISMO paso que da la gravedad, solo que ya
        # mismo. También reinicia el plazo: acabamos de bajar, así
        # que el reloj de la gravedad arranca de nuevo.
        li   a7, 30
        ecall
        sw   a0, ultimo_descenso, t3
        j    paso_abajo

gravedad:
        # --- 2) gravedad por reloj --------------------------------
        # syscall 30: milisegundos desde 1970 (bajos en a0, altos en
        # a1). Con los 32 bits bajos alcanza: la resta ahora-antes es
        # correcta aunque el contador dé la vuelta, y esa vuelta pasa
        # cada 2^32 ms ≈ 49 días — irrelevante para una partida.
        li   a7, 30
        ecall
        lw   t0, ultimo_descenso
        sub  t1, a0, t0         # ms transcurridos desde el último descenso
        lw   t2, gravedad_ms
        bltu t1, t2, descanso   # el plazo no venció: nada que hacer
        sw   a0, ultimo_descenso, t3

paso_abajo:
        # Un paso hacia abajo: si la fila de abajo está libre, baja;
        # si choca, la pieza se apoya y se fija donde está.
        addi a0, s3, 1
        mv   a1, s2
        mv   a2, s0
        mv   a3, s1
        call colisiona
        bnez a0, fijar_pieza
        addi s3, s3, 1
        li   s4, 1
        call redibujar

descanso:
        # --- 3) dormir si la vuelta fue vacía ---------------------
        # Sin esto, el bucle daría millones de vueltas por segundo
        # preguntando "¿hay tecla? ¿venció el plazo?". Dormir 15 ms
        # son ~60 consultas por segundo: de sobra para que el juego
        # responda al instante, sin derrochar CPU. Polling educado.
        bnez s4, bucle          # hubo cambios: otra vuelta ya mismo
        li   a0, 15
        li   a7, 32             # dormir a0 milisegundos
        ecall
        j    bucle

fijar_pieza:
        # Abajo está ocupado: la pieza pasa a ser parte del tablero
        # (misma rutina `fijar` del Ejemplo 8), se procesan líneas
        # completas y nace la pieza siguiente.
        mv   a0, s3
        mv   a1, s2
        mv   a2, s0
        mv   a3, s1
        call fijar
        beqz s3, game_over      # se apoyó tocando el tope: tablero lleno
        call procesar_lineas
        j    nueva_pieza

game_over:
        call dibujar_tablero
        la   a0, txt_gover
        li   a7, 4
        ecall
        j    fin

despedida:
        call dibujar_tablero
        la   a0, txt_fin
        li   a7, 4
        ecall

fin:
        la   a0, txt_score      # puntaje final
        li   a7, 4
        ecall
        lw   a0, score
        li   a7, 1
        ecall
        li   a0, '\n'
        li   a7, 11
        ecall
        li   a7, 10             # terminar
        ecall

# -----------------------------------------------------
# redibujar: tablero fijo + pieza activa.
# Se llama tras CADA cambio visible (tecla aplicada o descenso).
# dibujar_tablero ya marca el control de video (frame listo); la
# pieza se pinta después sobre la misma VRAM, así que el próximo
# volcado a pantalla la incluye.
# -----------------------------------------------------
redibujar:
        addi sp, sp, -4
        sw   ra, 0(sp)
        call dibujar_tablero
        call dibujar_pieza
        lw   ra, 0(sp)
        addi sp, sp, 4
        ret

# -----------------------------------------------------
# dibujar_pieza: pinta la pieza ACTIVA (s0..s3) en el framebuffer,
# encima del tablero ya dibujado. NO toca tablero[]: la pieza en
# vuelo existe solo en pantalla; recién al fijarse pasa a la
# memoria del tablero (igual que en el Ejemplo 8).
# Misma cuenta de píxeles que dibujar_tablero: bloques de 11x11 en
# una grilla de 12x12 con el tablero desde x=8.
# -----------------------------------------------------
dibujar_pieza:
        addi sp, sp, -12
        sw   ra, 8(sp)
        sw   s4, 4(sp)          # se preservan: el bucle principal
        sw   s5, 0(sp)          # usa s4 como bandera de eventos
        mv   a0, s0
        mv   a1, s1
        call bitmap_de
        mv   s4, a0             # bitmap de la pieza
        addi s5, s0, 2          # color = tipo + 2 (mismo código que fijar)

        li   t0, 0              # i = índice de bit (0..15)
dp_loop:
        li   t1, 15
        sub  t1, t1, t0
        srl  t2, s4, t1
        andi t2, t2, 1
        beqz t2, dp_sig
        srli t3, t0, 2          # fila dentro del 4x4 = i / 4
        andi t4, t0, 3          # col  dentro del 4x4 = i % 4
        add  t3, t3, s3         # fila absoluta
        add  t4, t4, s2         # col absoluta
        # píxel superior izquierdo: 0x90000000 + (fila*12)*320 + 8 + col*12
        li   t5, 12
        mul  t6, t3, t5
        li   t5, 320
        mul  t6, t6, t5
        li   t5, 12
        mul  a3, t4, t5
        add  t6, t6, a3
        addi t6, t6, 8
        li   t5, 0x90000000
        add  t6, t6, t5
        # pintar el bloque de 11x11 píxeles
        li   a3, 0              # dy
dp_dy:
        li   a5, 320
        mul  a4, a3, a5
        add  a4, a4, t6         # inicio de la línea dy del bloque
        li   a5, 0              # dx
dp_dx:
        add  t5, a4, a5
        sb   s5, 0(t5)
        addi a5, a5, 1
        li   t5, 11
        blt  a5, t5, dp_dx
        addi a3, a3, 1
        li   t5, 11
        blt  a3, t5, dp_dy
dp_sig:
        addi t0, t0, 1
        li   t1, 16
        blt  t0, t1, dp_loop
        lw   ra, 8(sp)
        lw   s4, 4(sp)
        lw   s5, 0(sp)
        addi sp, sp, 12
        ret

# =====================================================
# De acá para abajo, TODO es copia literal del Ejemplo 8:
# el motor del juego no cambió ni una instrucción.
# =====================================================

# -----------------------------------------------------
# random_pieza: a0 <- 0..6
# LCG de 16 bits idéntico al original 8086:
#   semilla = (semilla * 25173 + 13849) & 0xFFFF
#   pieza   = semilla % 7
# -----------------------------------------------------
random_pieza:
        lw   t0, semilla
        li   t1, 25173
        mul  t0, t0, t1
        li   t1, 13849
        add  t0, t0, t1
        li   t1, 0xFFFF         # el 8086 trabajaba en AX (16 bits)
        and  t0, t0, t1
        sw   t0, semilla, t2
        li   t1, 7
        remu a0, t0, t1
        ret

# -----------------------------------------------------
# bitmap_de: a0 = tipo, a1 = rotación -> a0 = bitmap de 16 bits
# piezas[(tipo*4 + rot)] como en obtener_bitmap_pieza
# -----------------------------------------------------
bitmap_de:
        slli t0, a0, 2
        add  t0, t0, a1         # tipo*4 + rot
        slli t0, t0, 2          # índice * 4 bytes (.word)
        la   t1, piezas
        add  t0, t0, t1
        lw   a0, 0(t0)
        ret

# -----------------------------------------------------
# colisiona: a0=fila, a1=col, a2=tipo, a3=rot -> a0 = 1 choca / 0 libre
# Recorre los 16 bits del bitmap; para cada bit en 1:
#   fuera del tablero (fila>=20 o col>=10) o celda ocupada -> colisión.
# Mismo algoritmo que chequear_colision del original.
# -----------------------------------------------------
colisiona:
        addi sp, sp, -16        # marco: ra + s4..s6
        sw   ra, 12(sp)
        sw   s4, 8(sp)
        sw   s5, 4(sp)
        sw   s6, 0(sp)
        mv   s4, a0             # fila base
        mv   s5, a1             # col base
        mv   a0, a2
        mv   a1, a3
        call bitmap_de
        mv   s6, a0             # bitmap

        li   t0, 0              # i = índice de bit (0..15)
co_loop:
        li   t1, 15
        sub  t1, t1, t0         # bit 15-i (de arriba-izquierda a abajo-derecha)
        srl  t2, s6, t1
        andi t2, t2, 1
        beqz t2, co_sig

        srli t3, t0, 2          # fila dentro del 4x4 = i / 4
        andi t4, t0, 3          # col  dentro del 4x4 = i % 4
        add  t3, t3, s4         # fila absoluta
        add  t4, t4, s5         # col absoluta
        li   t5, 20
        bgeu t3, t5, co_choca   # fuera por abajo
        li   t5, 10
        bgeu t4, t5, co_choca   # fuera por el costado
        mul  t6, t3, t5         # offset = fila*10 + col
        add  t6, t6, t4
        la   t5, tablero
        add  t6, t6, t5
        lbu  t6, 0(t6)
        bnez t6, co_choca       # celda ya ocupada

co_sig:
        addi t0, t0, 1
        li   t1, 16
        blt  t0, t1, co_loop
        li   a0, 0
        j    co_fin
co_choca:
        li   a0, 1
co_fin:
        lw   ra, 12(sp)
        lw   s4, 8(sp)
        lw   s5, 4(sp)
        lw   s6, 0(sp)
        addi sp, sp, 16
        ret

# -----------------------------------------------------
# fijar: a0=fila, a1=col, a2=tipo, a3=rot
# Graba los bits de la pieza en tablero[] (como fijar_pieza_en_tablero).
# A diferencia del port por turnos (que marcaba 1), acá la marca es
# tipo+2: el índice de color CGA de la pieza (2..8), que después
# dibujar_tablero usa directamente. Sigue siendo no-cero, así que
# colisiona y linea_completa funcionan igual.
# -----------------------------------------------------
fijar:
        addi sp, sp, -24
        sw   ra, 20(sp)
        sw   s4, 16(sp)
        sw   s5, 12(sp)
        sw   s6, 8(sp)
        sw   s7, 4(sp)
        mv   s4, a0
        mv   s5, a1
        addi s7, a2, 2          # color = tipo + 2 (paleta CGA: 2..8)
        mv   a0, a2
        mv   a1, a3
        call bitmap_de
        mv   s6, a0

        li   t0, 0
fj_loop:
        li   t1, 15
        sub  t1, t1, t0
        srl  t2, s6, t1
        andi t2, t2, 1
        beqz t2, fj_sig
        srli t3, t0, 2
        andi t4, t0, 3
        add  t3, t3, s4
        add  t4, t4, s5
        li   t5, 10
        mul  t6, t3, t5
        add  t6, t6, t4
        la   t5, tablero
        add  t6, t6, t5
        sb   s7, 0(t6)          # marca = color de la pieza
fj_sig:
        addi t0, t0, 1
        li   t1, 16
        blt  t0, t1, fj_loop
        lw   ra, 20(sp)
        lw   s4, 16(sp)
        lw   s5, 12(sp)
        lw   s6, 8(sp)
        lw   s7, 4(sp)
        addi sp, sp, 24
        ret

# -----------------------------------------------------
# linea_completa: a0 = fila -> a0 = 1 completa / 0 no
# -----------------------------------------------------
linea_completa:
        li   t0, 10
        mul  t1, a0, t0
        la   t2, tablero
        add  t1, t1, t2         # puntero al inicio de la fila
        li   t3, 0
lc_loop:
        lbu  t4, 0(t1)
        beqz t4, lc_no          # una celda vacía alcanza
        addi t1, t1, 1
        addi t3, t3, 1
        blt  t3, t0, lc_loop
        li   a0, 1
        ret
lc_no:
        li   a0, 0
        ret

# -----------------------------------------------------
# eliminar_linea: a0 = fila
# Baja todas las filas superiores una posición y vacía la fila 0
# (mismo algoritmo que el original: copia fila r-1 sobre fila r).
# -----------------------------------------------------
eliminar_linea:
        mv   t0, a0             # r = fila a sobrescribir
el_outer:
        beqz t0, el_top
        li   t1, 10
        mul  t2, t0, t1         # destino = tablero + r*10
        la   t3, tablero
        add  t2, t2, t3
        addi t4, t2, -10        # origen = fila de arriba
        li   t5, 10
el_copy:
        lbu  t6, 0(t4)
        sb   t6, 0(t2)
        addi t4, t4, 1
        addi t2, t2, 1
        addi t5, t5, -1
        bnez t5, el_copy
        addi t0, t0, -1
        j    el_outer
el_top:
        la   t1, tablero        # vaciar la fila 0
        li   t2, 10
el_clr:
        sb   x0, 0(t1)
        addi t1, t1, 1
        addi t2, t2, -1
        bnez t2, el_clr
        ret

# -----------------------------------------------------
# procesar_lineas: elimina todas las líneas completas y suma el puntaje
# 1=100, 2=300, 3=500, 4=800 (tabla `puntos`, como el original).
# Tras eliminar una fila NO se avanza: la misma posición tiene ahora
# la fila que bajó (el mismo "OJO" del código 8086).
# -----------------------------------------------------
procesar_lineas:
        addi sp, sp, -12
        sw   ra, 8(sp)
        sw   s4, 4(sp)
        sw   s5, 0(sp)
        li   s4, 0              # fila
        li   s5, 0              # eliminadas
pl_scan:
        li   t0, 20
        bgeu s4, t0, pl_puntos
        mv   a0, s4
        call linea_completa
        beqz a0, pl_next
        mv   a0, s4
        call eliminar_linea
        addi s5, s5, 1
        j    pl_scan            # misma fila otra vez
pl_next:
        addi s4, s4, 1
        j    pl_scan
pl_puntos:
        li   t0, 4              # 4 o más líneas pagan como 4
        blt  s5, t0, pl_idx
        li   s5, 4
pl_idx:
        slli t0, s5, 2
        la   t1, puntos
        add  t0, t0, t1
        lw   t2, 0(t0)          # puntos a sumar
        lw   t3, score
        add  t3, t3, t2
        sw   t3, score, t4
        lw   ra, 8(sp)
        lw   s4, 4(sp)
        lw   s5, 0(sp)
        addi sp, sp, 12
        ret

# -----------------------------------------------------
# dibujar_tablero: vuelca tablero[] a la pantalla gráfica.
# Cada celda es un bloque de 11x11 píxeles dentro de una grilla de
# 12x12 (1 píxel de separación): el tablero ocupa 120x240 desde x=8.
# Bordes laterales blancos en x=5..6 y x=129..130. Al final escribe 1
# en el registro de control (0x90030000): frame listo (vsync).
# El valor de cada celda ES el índice de color (0 = vacío = negro).
# Rutina hoja: no llama a nada, no necesita guardar ra.
# -----------------------------------------------------
dibujar_tablero:
        # bordes laterales (color 15 = blanco)
        li   t0, 0              # y
        li   t1, 0x90000000
        li   t2, 15
db_borde:
        li   t3, 320
        mul  t4, t0, t3
        add  t4, t4, t1         # inicio de la fila y del framebuffer
        sb   t2, 5(t4)
        sb   t2, 6(t4)
        sb   t2, 129(t4)
        sb   t2, 130(t4)
        addi t0, t0, 1
        li   t3, 240
        blt  t0, t3, db_borde

        # celdas: 20 filas x 10 columnas
        li   t0, 0              # fila del tablero
db_fila:
        li   t1, 0              # columna del tablero
db_col:
        # color = tablero[fila*10 + col]
        li   t2, 10
        mul  t3, t0, t2
        add  t3, t3, t1
        la   t2, tablero
        add  t2, t2, t3
        lbu  t2, 0(t2)
        # píxel superior izquierdo: 0x90000000 + (fila*12)*320 + 8 + col*12
        li   t4, 12
        mul  t5, t0, t4
        li   t4, 320
        mul  t5, t5, t4
        li   t4, 12
        mul  t6, t1, t4
        add  t5, t5, t6
        addi t5, t5, 8
        li   t4, 0x90000000
        add  t5, t5, t4
        # pintar el bloque de 11x11 píxeles
        li   t6, 0              # dy
db_dy:
        li   a4, 320
        mul  a3, t6, a4
        add  a3, a3, t5         # inicio de la línea dy del bloque
        li   a4, 0              # dx
db_dx:
        add  a5, a3, a4
        sb   t2, 0(a5)
        addi a4, a4, 1
        li   a5, 11
        blt  a4, a5, db_dx
        addi t6, t6, 1
        li   a5, 11
        blt  t6, a5, db_dy
        # siguiente celda
        addi t1, t1, 1
        li   a5, 10
        blt  t1, a5, db_col
        addi t0, t0, 1
        li   a5, 20
        blt  t0, a5, db_fila

        # frame listo: solicitar redibujado (vsync)
        li   t0, 0x90030000
        li   t1, 1
        sw   t1, 0(t0)
        ret
