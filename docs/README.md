# EmuRV-TFG — carpeta de trabajo

Carpeta de trabajo del prototipo técnico del TFG: entorno educativo de ensamblador RISC-V (RV32) — exploración pre-tutor.

## Contenido
- `docs/` — contrato de trabajo para Claude Code: CLAUDE.md (reglas), SPEC.md (requisitos), ARQUITECTURA.md (decisiones técnicas y riesgos), PLAN.md (fases con gates). Durante el desarrollo se sumaron los documentos técnicos ENSAMBLADOR.md (ensamblador propio RV32IM), MEMORIA-GRAFICA.md (pantalla gráfica 320×240) y TECLADO.md (teclado MMIO en tiempo real y syscalls de tiempo 30/32, fase de teclado 2026-06-12).
- El código del proyecto se generará en esta misma carpeta.
- `BITACORA.md` — aparecerá durante el trabajo: registro de decisiones con niveles de confianza. **Leéla entera al terminar: es material de tesis y es lo que te permite defender decisiones que no tomaste vos.**

Estado de las features al 2026-06-12 (rama `feature/teclado`): a lo de v0.1 y la pantalla gráfica se suman **teclado en tiempo real** (dispositivo MMIO compatible RARS/MARS, polling sin bloqueo), las **syscalls 30 (tiempo en ms) y 32 (dormir, interrumpible)** y dos ejemplos nuevos (`09-eco-teclado` y `10-tetris-tiempo-real`), para un total de **9 ejemplos**. Detalles en `TECLADO.md`.

## Cómo lanzar
```bash
cd ~/Documents/UNI/Tesis/EmuRV-TFG
claude --dangerously-skip-permissions
```
y pegar el prompt de arranque.

⚠️ Ese flag elimina TODAS las confirmaciones: Claude Code podrá ejecutar cualquier comando sin preguntar. Usalo solo dentro de esta carpeta y no lo conviertas en costumbre. La red de seguridad es git: todo queda commiteado por fase y es reversible.

## Verificación rápida al terminar
1. `git log --oneline` — debería haber un commit por fase como mínimo.
2. `cargo test` dentro de `src-tauri` — todo verde.
3. Abrir el bundle generado en `src-tauri/target/release/bundle/` y correr el ejemplo de fibonacci paso a paso.
