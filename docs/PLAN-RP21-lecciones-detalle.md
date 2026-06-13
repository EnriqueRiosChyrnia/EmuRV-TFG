# PLAN-RP21 — Recuperar vista de objetivos en el modal de Lecciones

Punto RP-21 de `docs/ROADMAP-PRO.md`. Fix de una REGRESIÓN introducida por RP-20.
Plan por fases con compuertas: PROHIBIDO avanzar con la compuerta en rojo.

## Qué pasó (la regresión)

RP-15 dio un navegador de lecciones que mostraba, ANTES de abrir cada lección:
sus OBJETIVOS («qué vas a aprender»), QUÉ PANELES mirar, y navegación
siguiente/anterior. RP-20 le dio la cara de «modal lista de un click» del mock y
en el camino se perdió esa vista previa: ahora un click abre la lección directo y
solo el 1er objetivo quedó como descripción. La lógica y el progreso de RP-15
(src/lecciones/) están INTACTOS — falta recuperar la PRESENTACIÓN del detalle.

Decisión del autor (2026-06-13): recuperar la vista de objetivos manteniendo la
estética del rediseño. (Acá la fidelidad al mock cede: el mock simplificó sin
conocer la feature de RP-15; el valor pedagógico para el principiante gana.)

## Contexto (anclas)

- `src/lecciones/parser.ts`: `MetaLeccion` ya tiene `objetivos: string[]` y
  `paneles: string` (+ título, número, multiarchivo). LA DATA YA EXISTE — solo
  hay que mostrarla.
- `src/lecciones/` (RP-15): progreso en localStorage, `vecina(n, dir, total)`
  para sig/ant ya existe (progreso.test.ts la prueba). NO reescribir.
- Modal de lecciones actual (RP-20): `src/paneles/lecciones.ts` + shell
  `src/paneles/modal.ts`, con `.modal-progreso` (barra real) + `.modal-item`
  (check/num). Estética Cálido (RP-19): `--radio-panel`, `--fuente-ui`, etc.
- Referencia de estilo: el modal de Ayuda de 2 columnas (RP-20, ay-body grid
  200px 1fr) es el patrón visual a seguir para coherencia.

## Diseño acordado (no re-litigar)

**Modal de Lecciones a DOS columnas** (mismo lenguaje que la Ayuda, así es
coherente y recupera la feature):
- Izquierda: la lista actual (`.modal-item` con check/num + barra de progreso
  arriba) — se conserva tal cual.
- Derecha: panel de DETALLE de la lección seleccionada — al hacer click en un
  item de la lista, en vez de abrir, se MUESTRA acá: título, OBJETIVOS (lista),
  QUÉ PANELES mirar, botón «Abrir lección» (carga al editor y cierra el modal),
  y navegación «‹ anterior / siguiente ›» (usa `vecina`). Marcar la lección
  seleccionada como activa en la lista (como el `ay-nav` de la ayuda).
- Primera apertura: detalle muestra la lección «en curso» (o la 1ª) por defecto.
- Reusar tokens/clases del rediseño (modal-card, ay-body-like grid, ay-cont
  styles). NADA hardcodeado fuera de tokens; contraste AA en los 5 temas.
- El caso multiarchivo (lección 8, guardar a carpeta) se dispara recién en
  «Abrir lección», igual que hoy — sin cambios.

## Fase 1 — Modal de Lecciones 2 columnas con detalle

Reestructurar `src/paneles/lecciones.ts` al layout 2-columnas: lista (izq,
conservada) + detalle (der, nuevo) leyendo `objetivos`/`paneles` del parser;
«Abrir lección» y sig/ant cableados a la lógica existente (vecina, carga,
progreso). i18n es/en de los rótulos nuevos («Objetivos», «Paneles a observar»,
«Abrir lección», «anterior/siguiente»). Tests: lo testeable (selección actualiza
detalle; sig/ant respeta bordes — reusa/extiende progreso.test). 
**Compuerta F1:** detalle muestra objetivos+paneles de la lección elegida; abrir
y sig/ant funcionan; progreso real intacto; contraste AA 5 temas; anti-drift
i18n; tsc/build; suite Rust intacta. Commit `rp21-fase-1: modal de lecciones a dos columnas con objetivos`.

## Fase 2 — Docs y cierre

README/Ayuda si el flujo de lecciones cambió de cara; BITACORA (recuperación de
la vista de RP-15, decisión de 2 columnas). ROADMAP-PRO y Cap. VI: NO tocar.
**Compuerta F2 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤30 MB + verificación visual (detalle en los
5 temas). Commit `rp21-fase-2: docs vista de objetivos de lecciones`.

## Criterios de aceptación

- Al elegir una lección se ven sus OBJETIVOS y QUÉ PANELES mirar ANTES de abrir
  (feature de RP-15 recuperada). ✓ F1.
- Navegación siguiente/anterior funcionando (vecina). ✓ F1.
- Lógica y progreso de RP-15 intactos (no reescritos); barra real. ✓ F1.
- Estética coherente con el rediseño (2 columnas como la Ayuda); contraste AA. ✓ F1.
- i18n es/en de los rótulos nuevos (anti-drift). ✓ F1.
