# Ejemplo 14 — Atómicas (A) e instrucciones comprimidas (C).
#
# Qué muestra:
#   * la extensión A: amoadd.w hace «leer-modificar-escribir» en UNA
#     operación indivisible sobre una palabra de memoria;
#   * lr.w / sc.w: el patrón «load-reserved / store-conditional» con el
#     que se construyen los candados (en EmuRV sc SIEMPRE tiene éxito);
#   * la extensión C: instrucciones de 2 bytes (c.li, c.addi, c.mv) que
#     hacen lo mismo que las de 4 ocupando la mitad — mirá el panel de
#     Memoria en «.text»: vas a ver instrucciones de 2 y de 4 bytes
#     mezcladas.
#
# NOTA sobre concurrencia: EmuRV emula UN SOLO procesador (mono-hart). Las
# atómicas son "atómicas" trivialmente porque nadie más toca la memoria;
# acá se usan para APRENDER el repertorio y el patrón lr/sc, no para
# paralelismo real (que necesitaría varios procesadores).

        .data
contador: .word 0

        .text
main:
        la   t2, contador

        # --- amoadd.w: sumar 10, después 5, al contador atómicamente ---
        # amoadd suma rs2 a la palabra en (rs1) y deja el VIEJO valor en rd.
        c.li t1, 10            # (comprimida) t1 = 10
        amoadd.w t0, t1, (t2)  # contador += 10  (t0 = valor viejo = 0)
        c.li t1, 5             # (comprimida) t1 = 5
        amoadd.w t0, t1, (t2)  # contador += 5   (t0 = valor viejo = 10)

        # --- lr.w / sc.w: poner el contador en 100 con store-conditional ---
        lr.w t0, (t2)          # reservar la celda
        li   t1, 100           # 100 no cabe en c.li (6 bits): va la de 4
        sc.w t0, t1, (t2)      # contador = 100; t0 = 0 (éxito, mono-hart)

        # --- imprimir el contador final ---
        lw   a0, 0(t2)
        c.li a7, 1             # (comprimida) servicio 1: imprimir entero
        ecall
        c.li a0, 10            # (comprimida) salto de línea
        c.li a7, 11
        ecall

        c.li a7, 10            # (comprimida) terminar
        ecall
