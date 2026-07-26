# Licencias

EmuRV se distribuye bajo la **GNU General Public License v2** (ver `LICENSE`).

Copyright (C) 2026 Enrique Rios Chyrnia

La razón es el enlazado: EmuRV usa `unicorn-engine`, que es GPL v2, así que el
binario resultante hereda esa licencia.

## Componentes de terceros

| Componente | Licencia | Dónde |
|---|---|---|
| unicorn-engine | GPL v2 | `vendor/unicorn-engine-sys/COPYING` |
| unicorn-engine (partes) | LGPL v2 | `vendor/unicorn-engine-sys/COPYING.LGPL2` |
| glib (partes) | ver archivo | `vendor/unicorn-engine-sys/COPYING_GLIB` |
| udbserver | MIT — Copyright (c) 2022 Bet4 | `vendor/udbserver/LICENSE` |

La copia de `unicorn-engine-sys` está vendorizada con un parche mínimo; el
detalle está comentado en `src-tauri/Cargo.toml` y en
`vendor/unicorn-engine-sys/CMakeLists.txt`.
