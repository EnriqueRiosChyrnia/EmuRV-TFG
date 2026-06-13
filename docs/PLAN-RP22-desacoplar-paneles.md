# PLAN-RP22 — Desacoplar paneles (Registros / Memoria / Depuración) a ventanas

Punto RP-22 de `docs/ROADMAP-PRO.md`. Feature nueva. Plan por fases con
compuertas: PROHIBIDO avanzar con la compuerta en rojo.

**Decisiones del autor (2026-06-13):** los TRES paneles (Registros, Memoria,
Depuración); modo MOVER (no duplicar) — al desacoplar, el panel se va del lateral
y aparece en una ventana; al cerrar la ventana, vuelve al lateral. Mismo patrón
que Pantalla.

## Contexto (anclas verificadas)

- Patrón existente: `src/paneles/video.ts` desacopla Pantalla con `WebviewWindow`
  (@tauri-apps/api/webviewWindow), label "pantalla", documento `pantalla.html`/
  `pantalla.ts`, flag `desacoplada` para convivencia. ESTUDIARLO: es la plantilla.
- Comandos Tauri ya existen para alimentar ventanas 2das: `get_state`
  (registros), `read_mem`, `write_reg`, `write_mem`, `write_reg_f`,
  `set_breakpoints`, `set_watchpoints`.
- Eventos ya emitidos: `emu://registros`, `emu://detenido` (con estado),
  `emu://consola`. Una ventana 2da se suscribe con `listen()` y se refresca sola.
- Paneles como clases TS reutilizables: `Registros`, `Memoria`, el de
  `Depuracion` — se renderizan en un contenedor; la idea es renderizar la MISMA
  clase en el lateral o en la ventana (sin duplicar lógica).
- Estética Cálido + 5 temas + i18n es/en: cada ventana 2da debe cargar
  tokens.css + tema activo + idioma (ver cómo lo hace pantalla.html/pantalla.ts).
- Depuración: los breakpoints se crean en el gutter del editor (ventana
  principal). Desacoplada, la LISTA vive aparte pero los breakpoints se siguen
  poniendo en el editor principal → sincronizar por eventos. Es el panel más
  delicado; va último.

## Alcance

- SÍ: mecanismo genérico de panel desacoplable (botón ⧉ en cada panel, ventana
  host que renderiza el panel pedido reusando su clase TS); modo MOVER
  (oculta del lateral / restaura al cerrar); sincronización por eventos;
  edición desde la ventana (write_reg/write_mem ya existen); propagación de
  tema+idioma a la ventana; los 3 paneles.
- NO: duplicar (decisión: mover); desacoplar la consola/editor; persistir la
  posición/tamaño de las ventanas entre sesiones (propuesta futura si se quiere);
  multi-instancia del mismo panel.

## Diseño acordado (no re-litigar; el spike afina el CÓMO)

- **Mecanismo genérico** en vez de 3 ad-hoc: un módulo `src/paneles/desacople.ts`
  (o similar) que, dado un panel (id + clase + título), abre una `WebviewWindow`
  con un documento host (`panel.html`/`panel.ts`) parametrizado por query string
  (?panel=registros|memoria|depuracion). El host instancia la MISMA clase del
  panel, se suscribe a los eventos relevantes, y aplica tema+idioma. Pantalla
  queda como está (caso especial canvas) o se alinea si es barato — NO romperla.
- **Modo MOVER:** al desacoplar, el panel se oculta del `#lateral` (flag, como
  video.ts) y el `#lateral` flex se recompone (RP-18 ya maneja paneles ocultos).
  Botón ⧉ en el `<h2>` de cada panel (igual que `video-desacoplar`). Al cerrar la
  ventana (evento close), el panel REAPARECE en el lateral.
- **Sincronización:** la ventana 2da escucha `emu://registros`/`emu://detenido` y
  re-renderiza; al editar (doble-click → write_reg/write_mem) invoca el comando y
  el estado se re-emite a TODAS las ventanas (coherencia). Cuidar no duplicar
  refrescos ni perder el resaltado de cambios.
- **Tema/idioma:** la ventana host lee el tema (localStorage emurv.tema.v1) e
  idioma (emurv.lang.v1) al abrir y aplica data-tema + traducciones; si el
  usuario cambia tema/idioma en la principal, propagarlo (evento o al reabrir —
  decidir en el spike; mínimo: correcto al abrir).

## Fase 1 — Spike + mecanismo genérico con Registros (mover)

`pantalla.ts`/`video.ts` como plantilla. PoC: desacoplar Registros a una ventana
que (1) renderiza la clase Registros, (2) se refresca por `emu://registros`/
`emu://detenido`, (3) aplica tema+idioma, (4) al cerrar reaparece en el lateral,
(5) editar un registro desde la ventana funciona (write_reg) y se refleja en la
principal. Responder en BITACORA: ¿la clase Registros se reusa tal cual en otro
documento? ¿los eventos llegan a la ventana 2da? ¿cómo se propaga el tema en vivo?
**Compuerta F1:** Registros desacopla/recompone/edita/sincroniza; tema+idioma ok;
contraste AA; tsc/build; suite Rust intacta. Commit `rp22-fase-1: mecanismo de desacople genérico + Registros`.

## Fase 2 — Memoria

Aplicar el mecanismo a Memoria (read_mem/write_mem + navegación + columna disasm).
Tests/verificación: desacopla, navega, edita, sincroniza, reaparece.
**Compuerta F2:** Memoria desacoplable igual que Registros; todo verde.
Commit `rp22-fase-2: desacople de Memoria`.

## Fase 3 — Depuración (el delicado)

Depuración a ventana: la lista de breakpoints (con condición) y watchpoints vive
en la ventana; los breakpoints se siguen creando en el gutter del editor
principal → la ventana se sincroniza por eventos/estado. Editar condición y
agregar watchpoint desde la ventana (set_breakpoints/set_watchpoints). Verificar
que poner un breakpoint en el editor aparece en la ventana desacoplada y
viceversa. **Compuerta F3:** Depuración desacoplable y sincronizada con el editor;
todo verde. Commit `rp22-fase-3: desacople de Depuración`.

## Fase 4 — Docs y cierre

README/Ayuda (cómo desacoplar paneles, botón ⧉); BITACORA (mecanismo genérico,
sincronización, decisiones del spike, lo dejado futuro: persistir posición,
duplicar). ROADMAP-PRO y Cap. VI: NO tocar.
**Compuerta F4 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤30 MB + verificación visual (los 3 paneles
desacoplados en varios temas). Commit `rp22-fase-4: docs desacople de paneles`.

## Criterios de aceptación

- Registros, Memoria y Depuración desacoplables a ventana, modo MOVER (se van del
  lateral, vuelven al cerrar). ✓ F1-F3.
- Ventanas 2das sincronizadas por eventos; edición desde la ventana funciona. ✓.
- Tema + idioma aplicados en las ventanas. ✓ F1.
- Mecanismo genérico (no 3 implementaciones ad-hoc); Pantalla no se rompe. ✓ F1.
- 5 temas AA, ids/registros/Rust intactos, bundle ≤30 MB. ✓ todas.
