# Lecciones de EmuRV — aprendé RISC-V desde cero

Currículo progresivo de 10 lecciones para aprender ensamblador RISC-V (RV32IM)
**sin docente al lado**: cada `leccion.s` es un programa comentado que se abre
en EmuRV y se recorre con el botón **«Paso»**, leyendo los comentarios — el
texto de la lección es tu docente. Todas corren también por terminal:

```
emurv-cli run lecciones/01-registros/leccion.s
```

## Cómo trabajar una lección

1. Abrí el archivo `leccion.s` en EmuRV (o copiá su contenido al editor).
2. Leé la cabecera: OBJETIVOS, qué PANELES mirar y CÓMO USARLA.
3. **«Ensamblar»** y después **«Paso»**, bloque por bloque. Cada comentario
   te dice qué va a pasar ANTES de que pase: tu trabajo es verificarlo.
4. Al final, hacé los 3 EJERCICIOS (el tercero siempre es un desafío).
   Modificá el código, volvé a ensamblar, comprobá tu predicción.

## Mapa de aprendizaje

```
        BASE                    CONTROL                 ESTRUCTURA
  01 registros  ──►  02 aritmética ──►  04 condicionales ──►  05 bucles
                          │                                       │
                          ▼                                       ▼
                     03 memoria   ──────────────►        06 funciones y pila
                          │                                       │
                          ▼                                       ▼
        MUNDO REAL   09 MMIO y tiempo real   ◄──  07 syscalls ──► 08 multiarchivo
                          │
                          ▼
        AVANZADA     10 traps (la semilla de un sistema operativo)
```

El orden 01→10 funciona siempre. Los atajos posibles: la 08 (multiarchivo)
se puede leer en cualquier momento después de la 06; la 09 necesita la 03
(memoria) y la 05 (bucles); la 10 es la única que pide TODO lo anterior.

## Tabla: qué aprendés dónde

| # | Lección | Qué aprendés | Prerrequisitos | Estado |
|---|---------|--------------|----------------|--------|
| 01 | Registros y movimiento de datos | li, mv, la; los 32 registros y sus nombres ABI; x0; imprimir por «Consola» | ninguno | ✓ |
| 02 | Aritmética y lógica | add/sub/mul/div/rem; complemento a 2 demostrado; and/or/xor; srli vs srai | 01 | ✓ |
| 03 | Memoria | .data, .word/.byte/.asciiz; lw/sw/lb/lbu/sb; endianness demostrado; panel «Memoria» | 01-02 | ✓ |
| 04 | Condicionales | beq/bne/blt/bge (+u); if/else con saltos; el máximo de dos números; la trampa blt vs bltu | 01-02 | ✓ |
| 05 | Bucles | while/for con saltos hacia atrás; suma 1..N; tablita anidada; **breakpoint condicional** `t0 == 5` (panel «Depuración») | 04 | ✓ |
| 06 | Funciones y pila | jal/ret, ra, a0-a7; sp y el frame; prólogo/epílogo; hoja vs no-hoja; la pila en «Memoria» → «pila (sp)» | 03-05 | ✓ |
| 07 | Syscalls y E/S | servicios 1/4/5/10/11/30/32; leer-validar-procesar; entrada por «Consola» | 04-05 | ✓ |
| 08 | Constantes, includes y macros | .eqv, .include, .macro con parámetros; macro (se copia) vs función (se salta); proyecto de 3 archivos | 06 | ✓ |
| 09 | E/S proyectada en memoria y tiempo real | MMIO; pintar píxeles en «Pantalla (320×240)»; teclado CONTROL/DATA por encuesta; concurrencia aparente | 03, 05 | ✓ |
| 10 | Traps e interrupciones (avanzada) | excepciones; mtvec/mepc/mcause; handler + mret; panel «CSRs (avanzado)» | 01-09 | llega con la fase 4 |

## Para seguir después

- Menú **«Ejemplos»** de la app: programas completos (factorial recursivo,
  Tetris gráfico, eco de teclado, manejador de excepciones) que las
  lecciones citan como material de profundización.
- `docs/ENSAMBLADOR.md`: la referencia completa del ensamblador.
- `docs/DEPURACION.md`: breakpoints condicionales, watchpoints y edición
  en vivo — el taller de la lección 05, completo.
