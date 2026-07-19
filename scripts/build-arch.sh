#!/usr/bin/env bash
# Compila el paquete pacman de EmuRV para Arch Linux x86_64 vía Docker.
# Uso:  ./scripts/build-arch.sh
set -euo pipefail
cd "$(dirname "$0")/.."

mkdir -p arch-dist
docker build --platform linux/amd64 -f Dockerfile.arch \
  --target export --output type=local,dest=arch-dist .

echo
echo "Paquete listo:"
ls -lh arch-dist/*.pkg.tar.zst
echo
echo "Instalar en Arch:  sudo pacman -U <archivo>.pkg.tar.zst"
