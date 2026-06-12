# ==========================================================
# Lección 08 — Constantes, includes y macros
# ==========================================================
# OBJETIVOS — al terminar esta lección vas a poder:
#   * repartir un programa en varios archivos con .include
#     (y explicar POR QUÉ conviene)
#   * ponerle nombre a los números con .eqv y usarlos en
#     lugar de valores sueltos
#   * escribir una macro con parámetros (.macro/.end_macro)
#     y usarla varias veces
#   * explicar la diferencia entre macro (se COPIA en cada
#     uso) y función (UNA copia a la que se SALTA con jal)
# PANELES PARA OBSERVAR: «Memoria» — después de Ensamblar,
#   apretá su botón «.text»: ese es el programa ya traducido,
#   y ahí se ven las copias que va dejando la macro.
#   «Registros» (vigilá ra: la función lo usa, la macro no)
#   y la «Consola» para la salida.
# CÓMO USARLA: este proyecto son TRES archivos que viven en
#   la MISMA carpeta: main.s (este), constantes.s y utiles.s.
#   La forma directa de correrlo es la terminal:
#       emurv-cli run main.s
#   (el menú «Ejemplos» de la app embebe archivos sueltos;
#   los proyectos multi-archivo van por el CLI, o abriendo
#   main.s en la app: los .include se encuentran solos
#   porque los otros dos archivos están al lado). En la app:
#   abrí main.s, «Ensamblar» y «Paso» bloque por bloque,
#   leyendo los comentarios.
# ----------------------------------------------------------

# ¿POR QUÉ varios archivos? Por orden: los números con
# nombre van todos juntos en constantes.s (si mañana cambia
# el precio, tocás UNA línea de UN archivo), las
# herramientas reutilizables van en utiles.s, y este archivo
# queda corto: solo el programa. La directiva
# .include "archivo.s" pega acá el texto de ese archivo,
# como si lo copiaras a mano; la ruta se busca en la carpeta
# de ESTE archivo. Las constantes van primero porque .eqv
# tiene que definirse ANTES de usarse.

.include "constantes.s"     # solo nombres: NO agrega código

        .text
# El procesador SIEMPRE arranca en la PRIMERA instrucción de
# .text. Como utiles.s sí trae código (la función), este
# salto lo esquiva: el primer Paso te deja directo en main.
        j    main

.include "utiles.s"         # la macro y la función viven acá

# OJO al usar «Paso»: el editor muestra UN solo archivo, así
# que cuando la ejecución entre al código de utiles.s, la
# línea resaltada va a ser la del .include de acá arriba.
# Para el editor, todo lo que un include trae «vive» en la
# línea donde lo incluiste.

# --- El programa: la cuenta de entradas de cine -------------
# Cada renglón de la salida es el mismo dúo: la macro
# «escribir» imprime el cartel y la función «entero_nl»
# imprime el número con su salto de línea.
main:
# «escribir txt_precio» NO es una instrucción: es la macro de
# utiles.s. Al Ensamblar, sus 3 instrucciones se COPIARON acá
# (compará en «Memoria» → «.text»). Por eso esta línea queda
# resaltada TRES Pasos seguidos: estás ejecutando la copia.
        escribir txt_precio
# PRECIO_ENTRADA tampoco existe para el procesador: el
# ensamblador ya lo cambió por 1500 (mirá a0 en «Registros»).
        li   a0, PRECIO_ENTRADA
# La función es lo contrario de la macro: hay UNA sola copia
# de su código, y jal SALTA hasta ella (lección 06). Mirá ra
# en «Registros»: con la macro no cambió; con jal, sí.
        jal  entero_nl

# Lo mismo para la cantidad. Segunda copia de la macro: la
# receta es la misma, solo cambió el parámetro %etiqueta.
        escribir txt_cantidad
        li   a0, ENTRADAS
        jal  entero_nl

# El total: precio × cantidad con mul (lección 02). Las dos
# constantes vienen de constantes.s, así que el cálculo se
# entiende leyéndolo, sin números mágicos.
        escribir txt_total
        li   t0, PRECIO_ENTRADA
        li   t1, ENTRADAS
        mul  a0, t0, t1
        jal  entero_nl

# Despedida (cuarta copia de la macro) y fin del programa.
# Fijate qué bien se lee «li a7, SYS_SALIR» comparado con un
# 10 suelto: el nombre dice la intención.
        escribir txt_gracias
        li   a7, SYS_SALIR
        ecall

        .data
txt_precio:   .asciiz "precio de una entrada: "
txt_cantidad: .asciiz "entradas compradas: "
txt_total:    .asciiz "total a pagar: "
txt_gracias:  .asciiz "gracias por su compra\n"

# ----------------------------------------------------------
# EJERCICIOS (modificá el código y volvé a ensamblar):
#   1. Subí el precio: cambiá PRECIO_ENTRADA a 2000 en
#      constantes.s SIN tocar este archivo, y verificá en la
#      Consola que el total se recalcule solo (6000).
#   2. Agregá en constantes.s la constante DESCUENTO con
#      valor 500, acá un cartel nuevo en .data («total con
#      descuento: ») y un bloque que lo imprima antes de la
#      despedida: macro con el cartel, calcular el total,
#      restarle DESCUENTO con sub (lección 02) y jal
#      entero_nl.
#   3. (desafío) Escribí en utiles.s una segunda macro
#      «renglon» con DOS parámetros que haga el dúo completo
#      (cartel + número + salto de línea), y reemplazá con
#      ella los dos primeros bloques de main. Pista: se
#      declara «.macro renglon (%etiqueta, %valor)», y
#      adentro una macro puede usar otra macro («escribir
#      %etiqueta»), cargar «li a0, %valor» y hasta hacer
#      jal entero_nl.
# PARA PROFUNDIZAR: el proyecto ejemplos/11-multiarchivo
#   (corrélo con «emurv-cli run ejemplos/11-multiarchivo/
#   main.s») usa estas mismas piezas con dos giros: el
#   include de código va AL FINAL del archivo — la otra
#   manera de proteger el arranque, sin j inicial — y la
#   función carga direcciones con %hi/%lo, que es la
#   pseudo-instrucción «la» escrita a mano.
# ==========================================================
