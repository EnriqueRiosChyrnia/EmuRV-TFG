# Ejemplo 12 — Manejador de excepciones (trap handler) con CSRs y mret.
#
# Qué muestra: el programa instala su PROPIO manejador de excepciones
# escribiendo su dirección en el CSR mtvec. Cuando algo excepcional pasa
# (acá: un ecall con un número de servicio que EmuRV no conoce), el
# emulador NO corta con error: le pasa el control al manejador, igual que
# el hardware real le pasa el control al sistema operativo.
#
# El manejador hace lo que haría un kernel en miniatura:
#   1. lee mcause (¿QUÉ pasó?)  → 11 = ecall desde modo máquina
#   2. lee mepc   (¿DÓNDE pasó?) → la dirección del ecall
#   3. atiende el pedido (acá: imprime un mensaje)
#   4. avanza mepc 4 bytes — si no, mret volvería AL MISMO ecall
#      y el ciclo se repetiría para siempre (¡probalo comentando
#      las tres líneas del ajuste!)
#   5. mret: volver al programa, al punto siguiente de la excepción.
#
# Los servicios CONOCIDOS (imprimir, leer, terminar…) NUNCA pasan por el
# manejador: los atiende EmuRV directamente, como siempre.

        .data
antes:    .asciiz "antes del ecall desconocido\n"
dentro:   .asciiz "  [manejador] atendiendo el servicio 777 (mcause=11)\n"
despues:  .asciiz "después: el programa siguió como si nada\n"

        .text
main:
        # Instalar el manejador: mtvec ← dirección de «manejador».
        la   t0, manejador
        csrw mtvec, t0

        la   a0, antes
        li   a7, 4
        ecall                  # servicio CONOCIDO: lo atiende EmuRV

        li   a7, 777           # servicio DESCONOCIDO: va al manejador
        ecall

        la   a0, despues       # mret aterriza acá (mepc + 4)
        li   a7, 4
        ecall

        li   a7, 10
        ecall

# --- el manejador de excepciones ---------------------------------------
manejador:
        # (un kernel real guardaría TODOS los registros que toca; acá
        #  usamos t1-t2 y a0/a7 sabiendo que main no los necesita vivos)
        csrr t1, mcause        # qué pasó: 11 = ecall de modo máquina
        csrr t2, mepc          # dónde: la dirección del ecall culpable

        la   a0, dentro
        li   a7, 4
        ecall                  # los servicios conocidos FUNCIONAN acá

        # Ajustar el retorno: mepc ← mepc + 4 (volver DESPUÉS del ecall).
        addi t2, t2, 4
        csrw mepc, t2

        mret                   # volver al programa
