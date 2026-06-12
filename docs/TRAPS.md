# TRAPS — CSRs y manejo de excepciones (RP-8, 2026-06-12)

EmuRV soporta el subconjunto didáctico de **Zicsr** y un manejo de
excepciones **delegable al programa**: con `mtvec` configurado, ciertas
excepciones le pasan el control a un manejador escrito por el estudiante —
el mismo mecanismo con el que un sistema operativo real atiende syscalls y
fallos. Ejemplo completo: `ejemplos/12-trap-handler.asm`.

## CSRs soportados

| Nombre | Núm. | Qué guarda |
|---|---|---|
| mstatus | 0x300 | estado global (bit 3 = MIE, lo usa el protocolo de traps) |
| mie / mip | 0x304 / 0x344 | máscaras de interrupciones (sin efecto: no hay interrupciones asíncronas) |
| mtvec | 0x305 | dirección del manejador de excepciones (0 = no hay) |
| mscratch | 0x340 | registro de uso libre del manejador |
| mepc | 0x341 | dónde ocurrió la excepción (a dónde vuelve `mret`) |
| mcause | 0x342 | qué pasó (tabla de abajo) |
| mtval | 0x343 | detalle (dirección o instrucción culpable) |

Instrucciones: `csrrw/csrrs/csrrc` (+ variantes inmediatas `csrr*i`) y los
pseudo `csrr rd, csr` / `csrw|csrs|csrc csr, rs`, más `mret`. Otros CSR
pueden nombrarse por número crudo `0xNNN`, pero solo los 8 de la tabla
están garantizados por el motor (Unicorn). Encodings verificados contra la
espec y ejecutados nativos (spike RP8-F1).

## Política de delegación (didáctica, no negociable)

| Situación | mtvec == 0 (inicial) | mtvec configurado |
|---|---|---|
| `ecall` con servicio CONOCIDO (1,4,5,10,11,30,32) | syscall de EmuRV | **syscall de EmuRV — JAMÁS se delega** |
| `ecall` con servicio desconocido | error didáctico | delegado: mcause=11, mepc=dir. del ecall |
| instrucción ilegal | error didáctico | delegado: mcause=2, mtval=la palabra |
| acceso a memoria inválido | error didáctico | delegado: mcause=5 (carga) / 7 (almacenamiento), mtval=la dirección |
| `ebreak` | pausa didáctica (RP-1) | **pausa didáctica — JAMÁS se delega** |

- `mepc` apunta **a la instrucción que trapeó** (fiel a la espec): el
  manejador debe avanzarlo (`+4`) antes de `mret` para no repetir el trap —
  exactamente lo que hace un kernel real (el ejemplo 12 lo enseña).
- **Doble falta**: una excepción DENTRO del manejador corta con el error
  «excepción dentro del manejador de excepciones (doble falta)» — sin
  loops. El mecanismo es el de la espec: la entrada al trap apaga
  `mstatus.MIE` y `mret` la restaura solo.
- Al reiniciar o re-ensamblar, los CSRs vuelven a 0 (con `MIE=1`): no hay
  manejadores fantasma entre sesiones.

## Límites

- **Sin interrupciones asíncronas** (timer/software/externas): `mie`/`mip`
  existen pero nada las dispara — propuesta futura (exige inyectar entre
  tramos de ejecución).
- M-mode plano: sin niveles de privilegio S/U reales.
- El panel de registros muestra los CSRs en la sección «CSRs (avanzado)»
  (plegada por defecto), editables con la máquina pausada.
