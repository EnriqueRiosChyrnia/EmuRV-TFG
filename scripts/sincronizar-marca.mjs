import { copyFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const files = ['assets.js', 'audio.js', 'player.js', 'scene.js', 'scene-base.js',
  'audio/ensamble-industrial.wav', 'audio/trazo.mp3', 'audio/cursor.mp3'];
for (const name of files) {
  const target = resolve(root, 'public/marca/emurv/inicio', name);
  await mkdir(dirname(target), { recursive: true });
  await copyFile(resolve(root, 'marca/emurv/inicio', name), target);
}
console.log('EmuRV: recursos de inicio sincronizados desde marca/emurv.');
