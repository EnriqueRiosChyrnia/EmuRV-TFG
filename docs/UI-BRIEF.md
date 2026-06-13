# UI-BRIEF — Mapa de la interfaz de EmuRV (para trabajo de diseño)

Documento de referencia para diseñar/ajustar la interfaz de EmuRV con Claude
(claude.ai / Desktop / Artifacts). Describe la estructura REAL para que cualquier
propuesta de diseño sea PORTABLE al código y no un HTML desde cero. Léelo junto
con las capturas de pantalla que aporta el autor.

## Qué es EmuRV

Emulador educativo de RISC-V (RV32IMAFDC) de escritorio, en español, para
aprender ensamblador. Público primario: **principiante absoluto**. Stack: Tauri
(ventana nativa) + TypeScript vanilla + CodeMirror 6 (editor) + CSS puro. NO hay
framework de UI (React/Vue/etc.) y CLAUDE.md lo prohíbe.

## Restricciones DURAS (toda propuesta debe respetarlas)

1. **Sin frameworks de UI.** CSS puro + TS vanilla. Nada de React/Vue/Tailwind-
   runtime/Bootstrap. Componentes = elementos DOM creados en TS.
2. **Estética por design tokens.** Toda la estética temable vive en
   `src/temas/tokens.css` como variables CSS. `styles.css` solo usa `var(--…)`,
   sin literales de color. Un tema = un bloque `[data-tema="x"]` que sobrescribe
   tokens. NO introducir colores/medidas hardcodeados.
3. **Legibilidad innegociable (WCAG AA, 4.5:1)** en los pares texto/fondo del
   editor, registros, memoria, consola y mensajes de error — en TODOS los temas.
   Hay un test que lo verifica (`src/temas/contraste.test.ts`).
4. **Peso ≤ 30 MB** (RNF-04, requisito de tesis). Nada de assets pesados; fuentes
   self-hosted livianas, iconos SVG inline.
5. **Idioma español** primario (UI internacionalizada es/en vía `src/i18n/`).
6. **Los `id` del DOM son contratos con el JavaScript.** `main.ts` y los paneles
   hacen `getElementById` de: `app, toolbar, editor, lateral, panel-video,
   video-canvas, video-desacoplar, panel-registros, registros, panel-memoria,
   memoria, panel-depuracion, depuracion, panel-consola, consola, estado`.
   Se puede cambiar CSS, grid, orden visual, estilos — pero **NO renombrar esos
   id ni eliminar los elementos que el JS espera**, o la app deja de funcionar.

## Layout actual (`index.html` + `src/styles.css`)

Grid principal `#app` (alto = 100vh):
```
grid-template-columns: minmax(420px, 1.4fr)  minmax(340px, 1fr)
grid-template-rows:    auto  1fr  230px  auto
grid-template-areas:
  "toolbar  toolbar"      ← barra de herramientas (ancho completo)
  "editor   lateral"      ← editor de código (izq) | panel lateral (der)
  "consola  lateral"      ← consola (izq, alto fijo 230px) | lateral sigue
  "estado   estado"       ← barra de estado (ancho completo)
```

Componentes:
- **#toolbar** (header): botones Abrir, Guardar, Exportar ELF, Ensamblar
  (`.principal`, color de acento), Ejecutar, Paso, Detener, Reiniciar, Ayuda +
  selectores de Ejemplos, Lecciones, Idioma (es/en) y Tema (Educativo/Retro-CRT).
  Botones con icono SVG (`.ico`, Lucide) + texto. `.separador` y `.espacio`
  (flex:1) organizan grupos.
- **#editor** (main): CodeMirror 6, fuente `--mono` (JetBrains Mono). Canalón de
  breakpoints (`.cm-breakpoint-gutter`), línea en ejecución resaltada
  (`.cm-linea-actual`). En modo binario (ELF importado) pasa a solo-lectura con
  listado desensamblado.
- **#lateral** (aside): sub-grid de 2 filas con 4 paneles `.panel` (cada uno
  `<h2>` título + `.panel-cuerpo` scrolleable):
  - **#panel-video** (oculto hasta que un programa use el framebuffer): canvas
    320×240, botón ⧉ para desacoplar a ventana aparte (`pantalla.html`).
  - **#panel-registros**: 32 registros enteros + (plegable) CSRs y registros F.
    Columnas hex/decimal, resaltado del que cambió, edición inline (doble-click).
  - **#panel-memoria**: volcado hex navegable + columna de instrucción
    desensamblada; edición inline.
  - **#panel-depuracion**: breakpoints (con condición editable) y watchpoints.
- **#panel-consola**: salida del programa (`.consola-salida`), entrada
  (`.consola-entrada`), botón Limpiar (`.consola-limpiar`); eco y errores con
  color (`.consola-eco`, `.consola-error`).
- **#estado** (footer): barra de estado (mensajes tipo «Pausado», «Listo»).

Ventana secundaria: `pantalla.html` (framebuffer desacoplado) — tiene su PROPIO
`<style>` embebido; si se rediseña el video, contemplarla aparte.

## Tokens disponibles (lo que un tema/ajuste puede tocar)

De `src/temas/tokens.css` (default «Educativo»): `--fondo, --fondo-panel,
--fondo-claro, --borde, --texto, --texto-suave, --texto-sobre-acento, --acento,
--ok, --error, --cambio, --resaltado-linea, --resaltado-cambio, --fila-par,
--overlay-fondo, --sombra-modal, --fondo-pantalla, --mono`. Si una propuesta
necesita un token nuevo (p. ej. un radio o un espaciado parametrizable), proponerlo
explícitamente como token, no como literal suelto.

## Cómo entregar las propuestas (para que sean portables)

Para CADA cambio propuesto, entregar:
1. **Mockup visual** (artifact HTML/CSS standalone) para VER el resultado — usando
   los nombres de token reales (`var(--acento)`, etc.) y, si se puede, la misma
   estructura de grid/clases de arriba.
2. **Lista accionable** de los cambios concretos, en términos del código real:
   - cambios de layout → en `grid-template-*` de `#app` o `#lateral`, o en clases
     existentes (NO renombrar ids);
   - cambios de tema → valores de tokens (un bloque `[data-tema="x"]`);
   - elementos nuevos → qué id/clase, dónde en la jerarquía, y qué JS haría falta.
3. Señalar cualquier cambio que toque un `id` contratado con el JS (para evaluar
   el costo en `main.ts`).

Lo que vuelve a la terminal de implementación es esa lista accionable + los
valores de tokens. El mockup es para decidir; el código real se edita en el repo.

## Problemas de layout conocidos / objetivos del autor (2026-06-13)

### BUG — toolbar se rompe en ventana angosta
En ventana reducida (no maximizada), los controles de la derecha del #toolbar
(Idioma, Ayuda, Tema) se tapan / quedan ocultos / rompen el diseño; maximizada se
ve bien. Es un bug de responsividad del flex de #toolbar (desborde sin manejo).
ARREGLAR: `flex-wrap`, scroll horizontal, o colapsar grupos a un menú «⋯» cuando
no hay ancho. La toolbar debe ser usable a cualquier ancho de ventana.

### Densidad de paneles — progressive disclosure (público principiante)
Decisión de diseño del autor + criterio pedagógico:
- **#panel-registros**: MÁXIMO valor didáctico, SIEMPRE visible (default). Es lo
  primero que las lecciones piden mirar.
- **#panel-memoria**: valor contextual (solo con .data/lw/sw/pila). Visible, pero
  secundario; plegable si hace falta espacio.
- **#panel-depuracion**: avanzado. Para un principiante es RUIDO al inicio.
  Recomendación: PLEGADO/oculto por defecto, se revela al poner un breakpoint o
  al desplegarlo. Dentro, los breakpoints son más intuitivos que los watchpoints;
  los watchpoints van en sub-sección plegada con una mini-explicación «¿qué es
  esto?». Aplicar progressive disclosure: lo esencial a la vista, lo avanzado
  bajo demanda.

### Orden de registros — CORRECTO, NO REORDENAR
El panel muestra los registros por número `x0…x31` (pc primero, luego x0..x31 con
su nombre ABI). El orden ABI NO es contiguo por tipo: t0–t2 = x5–x7, s0–s1 =
x8–x9, a0–a7 = x10–x17, s2–s11 = x18–x27, t3–t6 = x28–x31. Por eso los temporales
y los «saved» aparecen «partidos» en la lista. **Esto es la convención estándar
de RISC-V — RARS, gdb y toda la documentación lo muestran así.** NO reordenar a
«agrupado por tipo»: rompería la correspondencia con las herramientas y manuales
reales que el estudiante va a usar. Mejora permitida: separadores visuales sutiles
entre grupos o tooltip con el rol de cada registro — pero el ORDEN se mantiene.
