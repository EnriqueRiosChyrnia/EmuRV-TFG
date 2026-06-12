# DEPURACIÓN-GDB — stub gdbserver de EmuRV (RP-6, 2026-06-12)

EmuRV puede actuar como **servidor GDB remoto** (protocolo gdbserver/RSP): cualquier
cliente compatible —gdb para RISC-V, lldb, el depurador nativo de VS Code— se conecta
por TCP y depura el programa ensamblado con breakpoints, paso a paso, registros y
memoria. El stub vive en el **CLI** (la app de escritorio no participa: su build y su
bundle no cambian — la dependencia está detrás de la feature `gdb`, que la app no
compila).

## Arrancar el servidor

```sh
emurv-cli debug programa.s            # espera un cliente en 127.0.0.1:3333
emurv-cli debug programa.s --port 4000
```

- El fuente se ensambla con todo el repertorio (incluye `.include` multi-archivo).
- `stdout` es **exclusivo del programa invitado** (sus syscalls de impresión);
  todos los mensajes del stub van por `stderr`.
- El servidor espera la conexión ANTES de ejecutar la primera instrucción: el
  cliente ve el programa detenido en la entrada (`0x00400000`).
- Códigos de salida: los mismos del subcomando `run` (0 programa terminado,
  1 error de runtime, 2 de ensamblado, 3 uso inválido).

## Conectar un cliente

**gdb (cliente canónico, probado con riscv64-elf-gdb 17.2 de Homebrew):**

```sh
emurv-cli asm programa.s -o programa.elf   # el ELF lleva los símbolos (etiquetas)
riscv64-elf-gdb programa.elf
(gdb) set architecture riscv:rv32
(gdb) target remote 127.0.0.1:3333
(gdb) break factorial          # ¡por nombre! el ELF exportado tiene .symtab
(gdb) continue
(gdb) info registers pc a0 ra
(gdb) stepi
(gdb) x/2xw 0x10010000
(gdb) continue
```

**VS Code** (extensión nativa C/C++, `launch.json`):

```json
{
  "name": "EmuRV remoto",
  "type": "cppdbg",
  "request": "launch",
  "program": "${workspaceFolder}/programa.elf",
  "miDebuggerPath": "/opt/homebrew/bin/riscv64-elf-gdb",
  "miDebuggerServerAddress": "127.0.0.1:3333",
  "cwd": "${workspaceFolder}",
  "MIMode": "gdb"
}
```

(Arrancar `emurv-cli debug` aparte; VS Code se conecta al puerto.)

**lldb:** conecta (`gdb-remote 127.0.0.1:3333`) y frena, pero su soporte de
riscv32 por gdb-remote no inicializa el contexto de registros (probado con
lldb-2100): usá gdb.

## Sesión real de referencia

Transcript completo (programa `ejemplos/05-pila-funciones.asm`, factorial de 5):

```
The target architecture is set to "riscv:rv32".
0x00400000 in main ()
pc             0x400000   0x400000 <main>
Breakpoint 1 at 0x40003c              ← break factorial (gdb saltea el prólogo)
Breakpoint 1, 0x0040003c in factorial ()
pc             0x40003c   <factorial+4>
a0             0x5        5           ← argumento real
ra             0x400018   <main+24>
0x00400040 in factorial ()            ← stepi
0x10010000: 0x3d202135  0x000a0020    ← ".data": "5! =" en bytes LE
factorial + 8 in section .text        ← info symbol $pc
Remote connection closed
stdout del proceso: 5! = 120
```

## Protocolo soportado (verificado por tests automatizados, sin gdb externo)

| Paquete RSP | Función | Test |
|---|---|---|
| `qSupported` | capacidades (swbreak, hwbreak, qXfer:features) | spike_gdb |
| `?` | razón del alto (T05) | spike_gdb |
| `g` | leer los 33 registros (x0–x31 + pc; respuesta con RLE) | rp6_debug |
| `m addr,len` | leer memoria (.text, .data, página MMIO sin consumir) | rp6_debug |
| `Z0/z0` | poner/sacar breakpoint por software | rp6_debug |
| `c` / `s` | continue / step instrucción | rp6_debug |

## Modelo de ejecución y convivencia

- El stub controla la ejecución: `udbserver` bloquea sirviendo al cliente y la
  emulación corre LIBRE entre paradas (NO usa los tramos por count del modo
  normal — es un modo de ejecución propio; decidido por el spike de F1,
  BITACORA RP6).
- Los hooks de EmuRV (syscalls, teclado MMIO, memoria inválida) conviven con
  los del stub: leer la página del teclado desde el cliente (`m`) NO consume
  teclas (la página tiene respaldo real); el programa sí consume al leer.
- Funciona sobre programas multi-archivo (RP-5): breakpoints en etiquetas
  definidas dentro de un `.include` (probado con `saludar` del ejemplo 11).

## Limitaciones

- Sin DWARF: no hay información de líneas de fuente — se depura por
  direcciones y SÍMBOLOS (las etiquetas del programa van en el `.symtab` del
  ELF exportado). `break etiqueta` funciona; `list`/`next` por líneas, no.
- Un cliente a la vez; sin extensiones multi-hilo del protocolo.
- Watchpoints: lo que udbserver ofrezca (`Z2..Z4`); los watchpoints nativos
  del emulador son RP-7.
- Dependencia vendoreada: `vendor/udbserver/` es udbserver 0.3.0 (MIT) con
  dos cambios mínimos documentados en BITACORA (sin `dynamic_linkage`;
  mensajes por stderr).
