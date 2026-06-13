# Contenido de la sección «Acerca de» (About) — para diseño y mantenimiento

Sección «Acerca de» de EmuRV: identidad del proyecto, autoría y créditos de
terceros. Pensada como una sección más del modal de Ayuda (o un modal chico con
botón ⓘ). MANTENER ACTUALIZADA: cada vez que se agregue/saque una librería o
framework, actualizar la lista de tecnologías de abajo (ver «Mantenimiento»).

## El proyecto
**EmuRV** — Entorno educativo para programar en ensamblador RISC-V (RV32IMAFDC),
en español. Versión 0.1.
> La versión debería leerse del `version` de los manifiestos (Cargo/package.json),
> no escribirse a mano, para que no quede desfasada.

## Autor
**Enrique Emmanuel Ríos Chyrnia** — 25 años, Paraguay. Estudiante de Ingeniería
Informática.
> (Universidad: el autor decidió NO incluirla por ahora — 2026-06-13. Si más
> adelante se agrega, va acá junto con tutor y año, formato TFG.)
> (La edad es dato opcional en un About de software — incluida porque el autor la
> aportó; se puede quitar sin problema.)

## Asistencia de IA (honestidad metodológica)
EmuRV fue desarrollado con asistencia de modelos Claude (Anthropic) operando sobre
un contrato de especificación/arquitectura/plan, con verificación por pruebas
automatizadas. Coherente con lo registrado en BITACORA.md y ENSAMBLADOR.md.

## Tecnologías y créditos de terceros
Agrupadas por rol. Las licencias deben figurar (varias lo exigen).

### Núcleo y app de escritorio (Rust)
- **Rust** — lenguaje del núcleo.
- **Tauri 2** (MIT/Apache-2.0) — app de escritorio nativa liviana.
- **Unicorn Engine 2.1.5** (GPLv2) — motor de emulación de CPU. *(Su licencia
  GPLv2 condiciona la licencia del proyecto — ver decisión de licencia pendiente
  en Cap. VI de la memoria.)*
- **Capstone 0.14** (BSD-3-Clause) — desensamblador (panel de memoria, modo binario).
- **object 0.39** (MIT/Apache-2.0) — lectura/escritura de ejecutables ELF.
- **udbserver** (MIT, vendoreado en `vendor/udbserver`) — servidor GDB remoto
  sobre Unicorn; usa **gdbstub** (MIT).
- **serde / serde_json / thiserror** (MIT/Apache-2.0) — utilidades.

### Interfaz (TypeScript)
- **TypeScript 5.6** (Apache-2.0) + **Vite 6** (MIT) — lenguaje y empaquetado.
- **CodeMirror 6** (MIT) — editor de código (resaltado, autocompletado, hover).
- **@tauri-apps/api** + plugins dialog/opener (MIT/Apache-2.0) — puente con el
  backend.

### Tipografía e iconos
- **JetBrains Mono** (SIL Open Font License 1.1) — fuente monoespaciada
  (`src/assets/fonts/OFL.txt`).
- **Lucide** (ISC) — iconos SVG de la barra de herramientas.

## Mantenimiento (para que no se desfase)
Esta lista refleja las dependencias de `src-tauri/Cargo.toml`, `emurv-cli/
Cargo.toml` y `package.json` al 2026-06-13. Al agregar o quitar una librería o
framework en un RP futuro, ACTUALIZAR esta sección en el mismo cambio.
> Mejora opcional (patrón anti-drift del proyecto, como isa.ts/i18n): un test que
> cruce las deps DIRECTAS declaradas en los manifiestos contra esta lista y falle
> si aparece una no listada. Mantiene el About honesto sin esfuerzo manual.
> Evaluar al planificar — no todas las deps transitivas deben listarse, solo las
> tecnologías destacadas, así que el test necesitaría una lista de «ignoradas».
