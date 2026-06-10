# EmuRV-TFG — carpeta de trabajo

Carpeta de trabajo del prototipo técnico del TFG: entorno educativo de ensamblador RISC-V (RV32) — exploración pre-tutor.

## Contenido
- `docs/` — contrato de trabajo para Claude Code: CLAUDE.md (reglas), SPEC.md (requisitos), ARQUITECTURA.md (decisiones técnicas y riesgos), PLAN.md (fases con gates).
- El código del proyecto se generará en esta misma carpeta.
- `BITACORA.md` — aparecerá durante el trabajo: registro de decisiones con niveles de confianza. **Leéla entera al terminar: es material de tesis y es lo que te permite defender decisiones que no tomaste vos.**

## Cómo lanzar
```bash
cd ~/Desktop/EmuRV-TFG
claude --dangerously-skip-permissions
```
y pegar el prompt de arranque.

⚠️ Ese flag elimina TODAS las confirmaciones: Claude Code podrá ejecutar cualquier comando sin preguntar. Usalo solo dentro de esta carpeta y no lo conviertas en costumbre. La red de seguridad es git: todo queda commiteado por fase y es reversible.

## Verificación rápida al terminar
1. `git log --oneline` — debería haber un commit por fase como mínimo.
2. `cargo test` dentro de `src-tauri` — todo verde.
3. Abrir el bundle generado en `src-tauri/target/release/bundle/` y correr el ejemplo de fibonacci paso a paso.
