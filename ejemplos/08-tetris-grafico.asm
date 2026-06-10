# =====================================================
# Ejemplo 8 — Tetris gráfico (port de un Tetris 8086)
#
# Port a RV32IM de un Tetris escrito para emu8086 en la
# materia Microprocesadores. La LÓGICA es la misma que la
# del Tetris por turnos de v0.1 (y que la del 8086 original):
#   - tablero de 20x10 en memoria (byte por celda)
#   - 7 tetriminos como bitmaps de 16 bits (4x4, 4 rotaciones)
#   - generador pseudoaleatorio LCG (mismas constantes)
#   - colisión probada sobre una posición hipotética
#   - eliminación de líneas con desplazamiento hacia abajo
#   - puntaje: 100/300/500/800 por 1/2/3/4 líneas
#
# Lo que cambia es la SALIDA: igual que el 8086 original
# escribía en la memoria de video (B800h), esta versión
# dibuja el tablero en la pantalla gráfica de EmuRV
# (framebuffer en 0x90000000, 320x240, un byte por píxel):
# cada celda es un bloque de 12x12 píxeles y cada pieza
# conserva su color. La entrada sigue siendo por turnos
# en la consola: rotación y columna; -1 termina.
# =====================================================

        .data
tablero:    .space 200          # 20 filas x 10 columnas, 0 = vacío
semilla:    .word 0x1234        # misma semilla que el original
score:      .word 0

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
txt_titulo: .asciiz "=== TETRIS RV32 grafico (port del 8086) ===\n"
txt_score:  .asciiz "SCORE: "
txt_pieza:  .asciiz "Pieza: "
txt_rot:    .asciiz "Rotación (0-3, -1 sale): "
txt_col:    .asciiz "Columna (0-9, -1 sale): "
txt_inval:  .asciiz "Ahí no entra. Probá otra columna.\n"
txt_gover:  .asciiz "¡GAME OVER! El tablero llegó al tope.\n"
txt_fin:    .asciiz "Fin del juego.\n"

        .text
# Registros del estado del juego (convención de este programa):
#   s0 = tipo de pieza   s1 = rotación   s2 = columna   s3 = fila
main:
        la   a0, txt_titulo
        li   a7, 4
        ecall

turno:
        call dibujar_tablero

        call random_pieza       # a0 <- tipo (0..6), estilo LCG del original
        mv   s0, a0

        la   a0, txt_pieza      # "Pieza: X" + vista previa 4x4
        li   a7, 4
        ecall
        la   t0, nombres
        add  t0, t0, s0
        lbu  a0, 0(t0)
        li   a7, 11
        ecall
        li   a0, '\n'
        ecall
        mv   a0, s0
        call preview

pedir_rot:
        la   a0, txt_rot
        li   a7, 4
        ecall
        li   a7, 5              # leer entero
        ecall
        bltz a0, despedida      # -1: salir
        andi s1, a0, 3          # rotación mod 4 (como AND AL,03h)

pedir_col:
        la   a0, txt_col
        li   a7, 4
        ecall
        li   a7, 5
        ecall
        bltz a0, despedida
        li   t0, 10
        bgeu a0, t0, col_invalida
        mv   s2, a0

        # ¿La pieza entra en la fila 0 con esa columna/rotación?
        # (equivalente a chequear_colision con test_fila = 0)
        li   a0, 0
        mv   a1, s2
        mv   a2, s0
        mv   a3, s1
        call colisiona
        bnez a0, col_invalida

        # Caída dura: bajar mientras la fila siguiente esté libre
        li   s3, 0
caer:
        addi a0, s3, 1
        mv   a1, s2
        mv   a2, s0
        mv   a3, s1
        call colisiona
        bnez a0, reposo
        addi s3, s3, 1
        j    caer

reposo:
        mv   a0, s3             # fijar la pieza donde quedó apoyada
        mv   a1, s2
        mv   a2, s0
        mv   a3, s1
        call fijar
        beqz s3, game_over      # se apoyó tocando el tope: tablero lleno
        call procesar_lineas
        j    turno

col_invalida:
        la   a0, txt_inval
        li   a7, 4
        ecall
        j    pedir_col

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
# preview: a0 = tipo — imprime el bitmap 4x4 (rotación 0)
# en la consola, como ayuda para elegir rotación y columna.
# -----------------------------------------------------
preview:
        addi sp, sp, -8
        sw   ra, 4(sp)
        sw   s4, 0(sp)
        li   a1, 0
        call bitmap_de
        mv   s4, a0

        li   t0, 0              # i = 0..15
pv_loop:
        li   t1, 15
        sub  t1, t1, t0
        srl  t2, s4, t1
        andi t2, t2, 1
        li   a0, '.'
        beqz t2, pv_pinta
        li   a0, '#'
pv_pinta:
        li   a7, 11
        ecall
        addi t0, t0, 1
        andi t3, t0, 3          # fin de fila del 4x4
        bnez t3, pv_sigue
        li   a0, '\n'
        ecall
pv_sigue:
        li   t1, 16
        blt  t0, t1, pv_loop
        lw   ra, 4(sp)
        lw   s4, 0(sp)
        addi sp, sp, 8
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
