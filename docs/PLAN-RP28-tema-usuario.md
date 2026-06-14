# PLAN-RP28 — Tema de usuario (crear/editar/cargar)

Punto RP-28 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas: PROHIBIDO
avanzar con la compuerta en rojo. Se ejecuta en su WORKTREE (rama
`rp28/tema-usuario`), área `src/temas/` + un panel/modal nuevo — disjunta del lote.

## Contexto

- Sistema de temas (RP-17): `src/temas/tokens.css` con 5 temas como bloques
  `[data-tema="x"]` de variables; mecanismo `data-tema` + selector + localStorage
  `emurv.tema.v1`; cambio en vivo sin recargar; `src/temas/contraste.test.ts`
  verifica WCAG AA. La nota «cómo agregar un tema» está en tokens.css.
- Un tema = un conjunto de valores de tokens. El editor de tokens (RP-17 Parte A)
  ya dejó la base; RP-28 es la «Parte B» pendiente: que el USUARIO arme el suyo.

## Alcance

- SÍ: un tema «Personalizado» editable por el usuario — un editor (panel/modal)
  con controles de color para los tokens principales, preview en vivo, persistido
  en localStorage; aplicar/seleccionar ese tema; advertencia de contraste AA.
- NO: temas múltiples del usuario (uno «Personalizado» alcanza para v1; registrar
  como futuro); editar la sintaxis del editor token por token si resulta mucho
  (cubrir al menos fondo/superficies/texto/acento/estados).

## Diseño

- Tema `[data-tema="usuario"]`: sus valores NO van hardcodeados en tokens.css,
  se aplican por JS como variables inline en `:root`/el contenedor a partir de un
  objeto guardado en localStorage (`emurv.tema-usuario.v1`). Default: copia del
  Educativo, que el usuario modifica.
- Editor: panel/modal «Editar tema» con un control por token principal
  (`--fondo, --fondo-panel, --fondo-claro, --borde, --texto, --texto-suave,
  --acento, --texto-sobre-acento, --ok, --error, --cambio`, y los `--sx-*` de
  sintaxis si entra). `<input type="color">` + entrada hex. Preview EN VIVO
  (aplica al instante mientras edita). Botón restaurar (vuelve al Educativo).
- Selección: el selector de tema (RP-17) suma «Personalizado»; al elegirlo se
  aplican los valores guardados.
- Contraste: al guardar/editar, calcular el ratio de los pares críticos (reusar
  la lógica de `contraste.test.ts`) y ADVERTIR en la UI si algún par < 4.5:1 —
  no bloquear (es su tema), pero avisar («este color puede ser difícil de leer»).
- Cargar/exportar JSON del tema: opcional v1 (si es barato, un botón
  exportar/importar el objeto de tokens; si no, futuro — registrar).

## Fase 1 — Tema usuario (editor + persistencia + aplicar + aviso AA)

`data-tema="usuario"` aplicado desde localStorage; editor de tokens con preview;
selección; aviso de contraste. Tests: la función de aplicar tokens y la de
ratio de contraste (pura) testeables; el editor visual = manual (BITACORA).
**Compuerta F1:** crear/editar un tema, persiste al reabrir, se selecciona,
avisa si un par no cumple AA; los 5 temas built-in intactos; tsc + suite +
clippy.
Commit `rp28-fase-1: tema de usuario (editor de tokens + persistencia)`.

## Fase 2 — Docs y cierre

README/Ayuda (cómo crear tu tema); tokens.css nota actualizada; BITACORA
(decisión inline-vars vs CSS, alcance de tokens editables, export JSON sí/no).
NO tocar ROADMAP-PRO ni Cap. VI. Claves i18n AL FINAL de es.json y en.json.
**Compuerta F2 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤30 MB.
Commit `rp28-fase-2: docs tema de usuario`.

## Criterios de aceptación

- El usuario crea/edita un tema, con preview en vivo, persistido. ✓ F1.
- Aviso de contraste AA (no bloqueante). ✓ F1.
- Los 5 temas built-in y su contraste intactos. ✓ F1.
- Diferenciador: ninguna herramienta del nicho (Tabla 2.1) lo tiene. ✓.
