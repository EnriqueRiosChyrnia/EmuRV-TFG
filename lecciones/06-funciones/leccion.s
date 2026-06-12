# ==========================================================
# Lección 06 — Funciones y pila
# ==========================================================
# OBJETIVOS — al terminar esta lección vas a poder:
#   * llamar una función con jal y volver con ret
#   * explicar qué guarda ra y por qué no hay que perderlo
#   * pasar argumentos en a0…a7 y devolver el resultado en a0
#   * escribir un prólogo y un epílogo que guardan ra en la
#     pila (y explicar QUÉ se rompe si no lo hacés)
# PANELES PARA OBSERVAR: «Registros» (vigilá ra, sp y a0) y
#   «Memoria» — apretá el botón «pila (sp)» para saltar a la
#   zona de la pila: arranca en 0x7FFFFFF0 y crece hacia
#   ABAJO (direcciones más chicas). Y la «Consola», claro.
# CÓMO USARLA: «Ensamblar», apretá «pila (sp)» en el panel de
#   Memoria, y después «Paso» bloque por bloque. Si te
#   perdés, «Reiniciar» y arrancás de nuevo.
# ----------------------------------------------------------

        .text
main:
# Una función es un bloque de código con nombre que podés
# llamar desde donde quieras. La llamada sigue una convención:
#   * los argumentos van en a0…a7
#   * el resultado vuelve en a0
#   * ra ("return address") guarda adónde hay que VOLVER
# Guardamos el 7 en s0 porque a0 va a ser pisado por la
# función (los sN, por convención, sobreviven a las llamadas).
        li   s0, 7
        mv   a0, s0          # argumento: a0 = 7

# jal = "jump and link" hace DOS cosas en un solo Paso:
#   1) guarda en ra la dirección de la instrucción SIGUIENTE
#   2) salta a la etiqueta
# Dale un Paso y mirá «Registros»: ra se resalta (¡ya no es
# 0!) y el programa sigue en «duplicar», más abajo. Seguí
# con Paso hasta el ret y vas a volver justo acá.
        jal  duplicar        # a0 ← duplicar(7) = 14
        mv   s1, a0          # s1 = 14 (el resultado)

# Imprimimos "doble de 7 = 14" combinando los servicios que
# ya conocés (4 = cadena, 1 = entero, 11 = carácter).
        la   a0, txt_doble
        li   a7, 4
        ecall
        mv   a0, s0
        li   a7, 1
        ecall
        la   a0, txt_igual
        li   a7, 4
        ecall
        mv   a0, s1
        li   a7, 1
        ecall
        li   a0, 10
        li   a7, 11
        ecall

# Ahora una función que LLAMA a otra. Antes de dar el Paso
# sobre este jal, anotá el valor de ra que va a quedar: lo
# vas a ver aparecer en la pila, dentro de «cuadruple».
        mv   a0, s0          # argumento: a0 = 7
        jal  cuadruple       # a0 ← cuadruple(7) = 28
        mv   s1, a0

# Y lo mostramos: "cuádruple de 7 = 28".
        la   a0, txt_cuad
        li   a7, 4
        ecall
        mv   a0, s0
        li   a7, 1
        ecall
        la   a0, txt_igual
        li   a7, 4
        ecall
        mv   a0, s1
        li   a7, 1
        ecall
        li   a0, 10
        li   a7, 11
        ecall

        li   a7, 10          # fin del programa
        ecall

# --- FUNCIÓN HOJA: duplicar(n) → 2·n ------------------------
# "Hoja" = no llama a ninguna otra función. Como acá adentro
# nadie pisa ra, NO necesita tocar la pila: hace su cuenta y
# vuelve. ret es en el fondo jalr zero, ra, 0: "saltá a la
# dirección que dice ra". Por eso ra es el camino de vuelta.
duplicar:
        add  a0, a0, a0      # el resultado queda en a0
        ret                  # Paso acá: el PC vuelve al jal+4

# --- FUNCIÓN NO HOJA: cuadruple(n) → 4·n --------------------
# Esta SÍ llama a otra (duplicar, dos veces), y ahí está el
# peligro: cada jal de adentro PISA ra con SU dirección de
# vuelta. Si no guardáramos el ra original, el ret final
# saltaría adonde apuntó el último jal: a sí mismo, una y
# otra vez — bucle infinito, y a main no se vuelve nunca.
# La solución es el PRÓLOGO: bajar sp y guardar ra en la
# pila. Esos bytes reservados son el "marco" (frame) de la
# función: su espacio privado mientras trabaja.
cuadruple:
        addi sp, sp, -4      # PRÓLOGO: reservo 4 bytes (sp BAJA)
        sw   ra, 0(sp)       # …y guardo ra en ese lugar

# ¡Mirá el panel de Memoria AHORA! En 0x7FFFFFEC (cuatro
# bytes abajo del 0x7FFFFFF0 inicial) aparecieron los 4
# bytes de ra, byte bajo primero (little-endian, lección
# 03). Compará: es el mismo valor que muestra ra en
# «Registros». La vuelta a main quedó a salvo en la pila.
        jal  duplicar        # a0 = 2·n — ¡y ra acaba de cambiar!
        jal  duplicar        # a0 = 4·n
        lw   ra, 0(sp)       # EPÍLOGO: recupero el ra original
        addi sp, sp, 4       # devuelvo los 4 bytes (sp SUBE)
        ret                  # ahora sí: de vuelta a main

        .data
txt_doble: .asciiz "doble de "
txt_cuad:  .asciiz "cuádruple de "
txt_igual: .asciiz " = "

# ----------------------------------------------------------
# EJERCICIOS (modificá el código y volvé a ensamblar):
#   1. Cambiá el 7 de s0 por otro número y verificá que la
#      Consola imprima su doble y su cuádruple.
#   2. Escribí una función hoja «triple» (a0 ← 3·n; te
#      alcanza con dos add) y llamala desde main para
#      imprimir también el triple de s0.
#   3. (desafío) Poné # delante de las 4 líneas del prólogo
#      y el epílogo de «cuadruple» y recorré con «Paso»:
#      ¿adónde salta el ret final? Pista: mirá ra en
#      «Registros» después del segundo jal — quedó apuntando
#      al propio ret, y de ahí no se sale. «Reiniciar» para
#      rescatarlo, y devolvele el prólogo.
# PARA PROFUNDIZAR: el ejemplo «Pila y funciones» (abrilo
#   desde el botón «Ejemplos») calcula 5! con una función
#   RECURSIVA: cada llamada guarda su ra y su n en la pila.
#   Con lo que viste acá ya podés leerlo paso a paso.
# ==========================================================
