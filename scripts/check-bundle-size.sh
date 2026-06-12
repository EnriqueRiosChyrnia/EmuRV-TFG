#!/usr/bin/env bash
# Gate de RNF-04: el bundle macOS (.app) debe pesar ≤ 30 MB.
# Compartido por el job bundle-macos del CI y por scripts/ci-local.sh
# (principio espejo: un solo chequeo, no dos copias que diverjan).
# Este gate habría cazado automáticamente la regresión de RP-3 (la .app
# saltó a 34 MB cuando el bundler empaquetó el binario del CLI).
set -euo pipefail

APP="src-tauri/target/release/bundle/macos/EmuRV.app"
LIMITE_MB=30

if [ ! -d "$APP" ]; then
  echo "ERROR: no existe $APP (¿corrió 'npx tauri build'?)" >&2
  exit 1
fi

PESO_MB=$(du -sm "$APP" | cut -f1)
echo "Bundle: $APP → ${PESO_MB} MB (límite RNF-04: ${LIMITE_MB} MB)"
if [ "$PESO_MB" -gt "$LIMITE_MB" ]; then
  echo "ERROR: el bundle pesa ${PESO_MB} MB > ${LIMITE_MB} MB (RNF-04)" >&2
  exit 1
fi
echo "OK: dentro del límite."
