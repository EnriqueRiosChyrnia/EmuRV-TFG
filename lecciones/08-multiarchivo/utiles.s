# utiles.s — la caja de herramientas del proyecto: una macro
# («escribir») y una función («entero_nl»). main.s lo
# incluye justo después de su salto inicial, así este código
# queda en memoria pero no en el punto de arranque.

# --- MACRO: escribir %etiqueta ------------------------------
# Entre .macro y .end_macro todavía no hay código: es una
# RECETA. Cada vez que main escribe «escribir algo», el
# ensamblador COPIA estas 3 instrucciones ahí, poniendo
# «algo» donde dice %etiqueta. Sin saltos y sin tocar ra.
.macro escribir (%etiqueta)
        la   a0, %etiqueta
        li   a7, SYS_CADENA
        ecall
.end_macro

# --- FUNCIÓN: entero_nl -------------------------------------
# Imprime el entero que llega en a0 y un salto de línea. De
# esta hay UNA sola copia en memoria: se llega con jal y se
# vuelve con ret (lección 06). Es función hoja —no llama a
# nadie—, así que no necesita guardar ra en la pila.
entero_nl:
        li   a7, SYS_ENTERO
        ecall
        li   a0, NUEVA_LINEA
        li   a7, SYS_CARACTER
        ecall
        ret
