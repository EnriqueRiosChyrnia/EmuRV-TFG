# Ejemplo 13 — Coma flotante (extensiones F/D): promedio de notas.
#
# Qué muestra: el ciclo completo de punto flotante de EmuRV —
#   * leer floats del usuario con el servicio 6 (ReadFloat → fa0)
#   * acumular con fadd.s y dividir con fdiv.s (bits IEEE 754 reales)
#   * convertir un entero a float con fcvt.s.w (para el divisor)
#   * imprimir el resultado con el servicio 2 (PrintFloat)
#
# Los registros f se ven en el panel «Registros F (flotante)» (plegable,
# al pie del panel de registros): cada valor aparece en hexadecimal de 64
# bits y como número interpretado (p. ej. «8.5 (f32)»).
#
# Tabla de servicios de punto flotante (compatibles RARS):
#   a7 = 2   imprimir float   (el valor va en fa0)
#   a7 = 3   imprimir double  (fa0)
#   a7 = 6   leer float       (devuelve en fa0)
#   a7 = 7   leer double      (devuelve en fa0)

        .data
prompt:  .asciiz "Ingresá una nota (con punto, ej. 7.5): "
result:  .asciiz "Promedio: "
nl:      .byte 10

        .text
main:
        # Acumulador en ft0 = 0.0 (lo armamos desde el entero 0).
        li   t0, 0
        fcvt.s.w ft0, t0

        # Contador de notas: 3 (entero) y su versión float para el divisor.
        li   s0, 3              # cuántas notas vamos a leer
        li   s1, 0              # cuántas leímos hasta ahora

leer:
        la   a0, prompt
        li   a7, 4
        ecall

        li   a7, 6              # leer float → fa0
        ecall
        fadd.s ft0, ft0, fa0    # acumular: ft0 += nota

        addi s1, s1, 1
        blt  s1, s0, leer       # ¿faltan notas? volver a pedir

        # Dividir la suma por la cantidad: promedio = ft0 / 3.0
        fcvt.s.w ft1, s0        # 3 (entero) → 3.0 (float)
        fdiv.s ft2, ft0, ft1

        # Imprimir "Promedio: " y el resultado con el servicio 2.
        la   a0, result
        li   a7, 4
        ecall
        fmv.s fa0, ft2          # el servicio 2 imprime fa0
        li   a7, 2
        ecall
        la   a0, nl
        li   a7, 4
        ecall

        li   a7, 10
        ecall
