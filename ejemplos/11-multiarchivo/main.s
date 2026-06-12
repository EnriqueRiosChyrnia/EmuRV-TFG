# Ejemplo 11 — programa multi-archivo (.include, .eqv, .macro, %hi/%lo).
# Ensamblar y ejecutar desde la terminal:
#   emurv-cli run ejemplos/11-multiarchivo/main.s
# (En la app de escritorio: Abrir este archivo — los .include se resuelven
#  relativos a la carpeta del archivo guardado.)

.include "constantes.s"     # constantes .eqv + macro (no emiten código)

        .data
saludo:    .asciiz "¡Hola desde varios archivos!\n"
despedida: .asciiz "Listo.\n"

        .text
main:
        li   s0, VECES      # contador de saludos (constante de otro archivo)
bucle:
        jal  saludar        # rutina definida en utiles.s (incluido abajo)
        addi s0, s0, -1
        bnez s0, bucle

        imprimir despedida  # macro definida en constantes.s

        li   a7, SYS_EXIT
        ecall

.include "utiles.s"         # rutinas con código: SIEMPRE después de main
