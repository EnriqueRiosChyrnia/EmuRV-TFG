# ==========================================================
# Lección 10 — Traps: la semilla de un sistema operativo
# ==========================================================
# OBJETIVOS — al terminar esta lección vas a poder:
#   * explicar qué es una excepción (trap): un evento que
#     interrumpe al programa y le pasa el control a OTRO código
#   * usar los CSRs mtvec (¿a dónde ir?), mcause (¿qué pasó?)
#     y mepc (¿dónde pasó?)
#   * instalar tu propio manejador, dispararlo con un ecall
#     desconocido y VOLVER con mret
#   * explicar por qué un kernel ajusta mepc antes de volver
# PANELES PARA OBSERVAR: abrí «CSRs (avanzado)» (viene plegado:
#   click en su título). Vas a ver mtvec llenarse cuando lo
#   escribas, y mcause/mepc llenarse SOLOS en el instante del
#   trap. La «Consola» cuenta la historia; en «Registros», el
#   pc pega el salto más dramático del curso.
# CÓMO USARLA: «Ensamblar» y «Paso», bloque por bloque. El
#   Paso clave es el del ecall desconocido: el pc NO avanza
#   +4 — aterriza en el manejador. Mirá «CSRs (avanzado)»
#   justo en ese Paso.
# ----------------------------------------------------------

        .text
main:
# Hasta hoy, un ecall con un servicio inexistente cortaba el
# programa con error. Un sistema operativo real no se muere:
# ATIENDE el evento y decide. Hoy tu programa se desdobla en
# dos personajes: el «usuario» (main) y un mini-kernel (el
# manejador). El contrato es el CSR mtvec: una dirección que
# le dice al procesador «cuando pase algo excepcional, saltá
# ACÁ». csrw lo escribe — mirá cómo aparece en «CSRs
# (avanzado)» al dar este Paso.
        la   t0, manejador
        csrw mtvec, t0
        la   a0, listo_txt
        li   a7, 4
        ecall                  # servicio CONOCIDO: EmuRV lo
                               # atiende como siempre. Los
                               # conocidos JAMÁS van a mtvec.

# --- Disparar el trap ---------------------------------------
# El servicio 777 no existe. En este Paso el procesador:
#   1. anota en mcause un 11 («ecall desde modo máquina»)
#   2. anota en mepc la dirección de ESTE ecall
#   3. pone el pc en mtvec → tu manejador
# Compará pc y mepc en los paneles: mepc quedó apuntando acá.
        li   a7, 777
        ecall
# mret aterriza EXACTAMENTE acá (mepc + 4): la vida sigue.
        la   a0, vuelta_txt
        li   a7, 4
        ecall
        li   a7, 10
        ecall

# --- El mini-kernel -----------------------------------------
# Esto es lo que hace un sistema operativo, en miniatura:
# preguntar QUÉ pasó, atenderlo y devolver el control.
manejador:
        csrr t1, mcause        # ¿qué pasó? 11 = ecall (M-mode)
        la   a0, mcause_txt
        li   a7, 4
        ecall                  # los conocidos funcionan acá
        mv   a0, t1
        li   a7, 1
        ecall                  # imprime: 11
        li   a0, 10
        li   a7, 11
        ecall

# Atender el pedido: nuestro «servicio 777» saluda. (a7 sigue
# valiendo 777 acá adentro: un kernel real haría beq sobre a7
# para elegir qué servicio dar — ejercicio 2.)
        la   a0, srv_txt
        li   a7, 4
        ecall

# EL detalle que separa a un kernel de un bucle infinito:
# mepc apunta AL ecall culpable. Si hiciéramos mret ya mismo,
# volveríamos AL MISMO ecall → otro trap → otra vez acá…
# para siempre. Avanzamos mepc 4 bytes (una instrucción) para
# volver DESPUÉS del ecall. Mirá mepc cambiar en el panel.
        csrr t2, mepc
        addi t2, t2, 4
        csrw mepc, t2
        mret                   # pc ← mepc: de vuelta a main

        .data
listo_txt:  .asciiz "manejador instalado: ahora tengo mini-kernel\n"
mcause_txt: .asciiz "  [manejador] mcause = "
srv_txt:    .asciiz "  [manejador] servicio 777: atendido\n"
vuelta_txt: .asciiz "de vuelta en main: el programa siguió como si nada\n"

# ----------------------------------------------------------
# EJERCICIOS (modificá el código y volvé a ensamblar):
#   1. Comentá las tres líneas del ajuste de mepc (csrr/addi/
#      csrw) y ejecutá. Predicción: trap eterno — la Consola
#      repite el mensaje del manejador sin parar. Frenálo con
#      «Detener» y explicá por qué pasó. Después descomentá.
#   2. Agregá el servicio 888 «despedirse»: en el manejador,
#      compará a7 con beq (lección 04) y saltá a imprimir un
#      mensaje distinto por cada servicio. Probalo cambiando
#      el li a7, 777 de main por 888.
#   3. (desafío) Un servicio que DEVUELVE un valor: que el
#      999 deje 42 en a0 antes del mret, y que main, a la
#      vuelta, lo imprima con el servicio 1. Pista: el
#      manejador puede escribir a0 como cualquier registro…
#      pero entonces no puede usarlo para imprimir — atendé
#      el 999 en silencio. Así es como el sistema operativo
#      le CONTESTA a un programa. Esto —atender pedidos,
#      decidir, contestar— es la semilla de un sistema
#      operativo: lo que acabás de escribir.
# ==========================================================
