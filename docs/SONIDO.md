# SONIDO — Dispositivo de tono MMIO (RP31)

Dispositivo de **sonido** que da a los programas una voz de tono simple,
espejando el modelo del framebuffer y del teclado: una región MMIO nueva, un
estado compartido y un evento hacia la UI, que reproduce el tono con **WebAudio**
(nativo del runtime, cero dependencias). **Apagado por defecto** (RF4): un
programa que no usa el dispositivo no produce ningún sonido y ni siquiera crea
el contexto de audio.

## Región mapeada

| Región | Base | Tamaño físico | Tamaño mapeado | Propósito | Acceso |
|--------|------|---|---|-----------|--------|
| **Sonido** | **`0x9004_0000`** | **16 B** | **4 KB** | Registros de tono (RP31) | R/W |

Disjunta del framebuffer/paleta/control (`0x9000`–`0x9003`), de los sprites
(`0x9005`–`0x9006`) y del teclado (`0xFFFF_0000`) — verificado en
`tests/spike_sonido.rs`. Constantes en `src-tauri/src/core/mod.rs`.

## Registros (`0x9004_0000`)

Cuatro registros de 32 bits (escribir con `sw`):

| Offset | Registro | Significado |
|--------|----------|-------------|
| +0x00 | `SOUND_FREQ` | frecuencia del tono en Hz |
| +0x04 | `SOUND_DUR` | duración del tono en ms |
| +0x08 | `SOUND_WAVE` | forma de onda: 0=cuadrada, 1=senoidal, 2=triángulo, 3=sierra |
| +0x0C | `SOUND_TRIGGER` | **disparo**: escribir cualquier valor emite un tono con la frecuencia, duración y forma de onda vigentes |

`FREQ`, `DUR` y `WAVE` son memoria común con respaldo real (se pueden leer y
reescribir sin efectos colaterales, igual que el teclado). Solo `TRIGGER`
dispara: cada escritura encola **un** tono.

## Semántica

- **Mecanismo (como el teclado, no como el framebuffer).** El framebuffer se
  publica de forma pasiva (se relee cada tramo); el sonido necesita eventos
  **discretos** (cada disparo = un tono), así que usa un **hook MEM_WRITE** sobre
  `SOUND_TRIGGER` (validado en `tests/spike_sonido.rs`). Al disparar, el hook lee
  `freq/dur/wave` de memoria y encola un evento en `SoundShared` (espejo de
  `VideoShared`), marcando el dispositivo en uso.
- **Emisión.** El run loop drena la cola entre tramos (y el hilo de consola al
  pedir entrada, por si el programa disparó un tono y se bloqueó esperando una
  línea) y emite un evento Tauri `emu://sonido { freq, dur_ms, wave }` por cada
  tono.
- **Reproducción (WebAudio).** El frontend (`src/paneles/audio.ts`) crea un
  `AudioContext` perezosamente en el primer tono (para entonces el usuario ya
  tocó «Ejecutar», así que la política de autoplay permite reanudarlo), y por
  cada evento crea un `OscillatorNode` (tipo según `wave`) con un `GainNode` de
  envolvente corta (ataque/liberación de 5 ms) para que el tono no «chasquee» al
  empezar/cortar. Se autodetiene a `dur_ms`.
- **Silencio tolerante.** Sin soporte de WebAudio, o ante un tono vacío
  (frecuencia o duración ≤ 0), no suena nada.
- **Reiniciar** vacía la cola y pone los registros en 0: sin tonos fantasma de la
  corrida anterior.

## Uso típico

```asm
li t0, 0x9004_0000     # base del dispositivo de sonido
li t1, 1
sw t1, 8(t0)           # forma de onda = senoidal
li t1, 300
sw t1, 4(t0)           # duración = 300 ms
li t1, 440
sw t1, 0(t0)           # frecuencia = 440 Hz (la)
li t1, 1
sw t1, 12(t0)          # disparo: suena el tono
```

Disparar varios tonos seguidos sin pausa entre ellos los reproduce **juntos**
(un acorde), porque los eventos se emiten al ceder el control el tramo. Para una
melodía, separá los disparos con la syscall 32 (dormir N ms). Ver el ejemplo
`ejemplos/16-sonido.asm` (un acorde de Do mayor).

## Fuera de alcance (declarado)

Un canal de tono basta para lo educativo. NO se implementan: música/secuenciador,
mezcla multicanal compleja, ni formatos de audio (WAV/MP3). Si hiciera falta más
de un canal, se registraría como extensión.
