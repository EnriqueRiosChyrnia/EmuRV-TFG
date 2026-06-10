/// Conversión de framebuffer indexado a RGBA (FB1) y estado compartido
/// con la UI (FB2).
use super::{FRAMEBUFFER_DATA_SIZE, PALETTE_DATA_SIZE};

/// Estado de video compartido entre el hilo de emulación y los comandos
/// Tauri. El hilo de emulación PUBLICA frames acá (también desde el hook
/// de syscalls, antes de bloquearse esperando entrada — si no, el tablero
/// de un juego por turnos no se vería hasta después de teclear); la UI los
/// CONSUME con `read_framebuffer` sin tocar el hilo de emulación.
#[derive(Default)]
pub struct VideoShared {
    /// Último frame RGBA publicado. Se CONSERVA (no se consume): puede
    /// haber más de un consumidor (panel embebido + ventana aparte), y un
    /// consumidor nuevo necesita el frame vigente aunque sea viejo.
    pub rgba: Option<Vec<u8>>,
    /// Número de publicación, monotónico. Cada consumidor recuerda el suyo
    /// y pide "algo más nuevo que seq": es el flag sucio de RF2 por
    /// consumidor, sin que uno le robe el frame al otro.
    pub seq: u64,
    /// El programa usó la pantalla (escribió VRAM o el registro de control).
    pub in_use: bool,
}

impl VideoShared {
    /// Estado de video para una sesión nueva (re-ensamblado) que reemplaza
    /// a `anterior`. El contador de publicaciones CONTINÚA: los
    /// consumidores (panel embebido, ventana desacoplada) recuerdan el seq
    /// de la sesión anterior y no tienen forma de enterarse del reemplazo;
    /// si el contador volviera a cero descartarían todos los frames de la
    /// sesión nueva por "viejos" (bug: al cambiar de programa la pantalla
    /// seguía mostrando el frame del anterior). Si la pantalla estaba en
    /// uso, la sesión nueva arranca publicando un frame negro: el programa
    /// nuevo no hereda la imagen del anterior.
    pub fn para_sesion_nueva(anterior: Option<&std::sync::Mutex<VideoShared>>) -> VideoShared {
        let Some(ant) = anterior else {
            return VideoShared::default();
        };
        let Ok(ant) = ant.lock() else {
            return VideoShared::default();
        };
        if ant.in_use {
            VideoShared {
                rgba: Some(frame_negro()),
                seq: ant.seq + 1,
                in_use: false,
            }
        } else {
            VideoShared {
                rgba: None,
                seq: ant.seq,
                in_use: false,
            }
        }
    }
}

/// Frame RGBA negro opaco: la "pantalla limpia".
pub fn frame_negro() -> Vec<u8> {
    let mut f = vec![0u8; (FRAMEBUFFER_DATA_SIZE * 4) as usize];
    for px in f.chunks_exact_mut(4) {
        px[3] = 255;
    }
    f
}

/// Buffer RGBA en formato compatible con canvas (RGBA8888, little-endian).
/// Dimensiones fijas: 320×240 = 76800 píxeles = 307200 bytes.
#[derive(Default)]
pub struct FramebufferRGBA {
    pub data: Vec<u8>,
}

impl FramebufferRGBA {
    pub fn new() -> Self {
        Self {
            data: vec![0u8; 320 * 240 * 4],
        }
    }

    /// Convierte framebuffer indexado + paleta a RGBA.
    /// `fb_data`: bytes de framebuffer (320×240)
    /// `pal_data`: bytes de paleta (256×3, RGB888)
    pub fn convert(&mut self, fb_data: &[u8], pal_data: &[u8]) {
        assert_eq!(fb_data.len(), FRAMEBUFFER_DATA_SIZE as usize);
        assert_eq!(pal_data.len(), PALETTE_DATA_SIZE as usize);

        for (i, &idx) in fb_data.iter().enumerate() {
            let idx = idx as usize;
            let pal_offset = idx * 3;
            let (r, g, b) = if pal_offset + 2 < pal_data.len() {
                (pal_data[pal_offset], pal_data[pal_offset + 1], pal_data[pal_offset + 2])
            } else {
                (0, 0, 0)
            };

            let rgba_offset = i * 4;
            self.data[rgba_offset] = r;
            self.data[rgba_offset + 1] = g;
            self.data[rgba_offset + 2] = b;
            self.data[rgba_offset + 3] = 255;
        }
    }
}

/// Paleta VGA 13h estándar: primeros 16 colores (CGA) + rampa de grises.
pub fn palette_vga13h_default() -> Vec<u8> {
    let mut pal = vec![0u8; PALETTE_DATA_SIZE as usize];

    // Primeros 16 colores: CGA estándar (versiónificada a partir de la especificación VGA)
    let cga16 = [
        (0x00, 0x00, 0x00), // 0: negro
        (0x00, 0x00, 0xAA), // 1: azul oscuro
        (0x00, 0xAA, 0x00), // 2: verde oscuro
        (0x00, 0xAA, 0xAA), // 3: cian oscuro
        (0xAA, 0x00, 0x00), // 4: rojo oscuro
        (0xAA, 0x00, 0xAA), // 5: magenta oscuro
        (0xAA, 0x55, 0x00), // 6: marrón
        (0xAA, 0xAA, 0xAA), // 7: gris claro
        (0x55, 0x55, 0x55), // 8: gris oscuro
        (0x55, 0x55, 0xFF), // 9: azul claro
        (0x55, 0xFF, 0x55), // 10: verde claro
        (0x55, 0xFF, 0xFF), // 11: cian claro
        (0xFF, 0x55, 0x55), // 12: rojo claro
        (0xFF, 0x55, 0xFF), // 13: magenta claro
        (0xFF, 0xFF, 0x55), // 14: amarillo
        (0xFF, 0xFF, 0xFF), // 15: blanco
    ];

    for (i, &(r, g, b)) in cga16.iter().enumerate() {
        pal[i * 3] = r;
        pal[i * 3 + 1] = g;
        pal[i * 3 + 2] = b;
    }

    // Entradas 16-255: rampa de grises (0x00 a 0xFF)
    for i in 16..256 {
        let gray = ((i - 16) as u16 * 255 / 239) as u8;
        pal[i * 3] = gray;
        pal[i * 3 + 1] = gray;
        pal[i * 3 + 2] = gray;
    }

    pal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_indice_a_rgba() {
        let mut fb_rgba = FramebufferRGBA::new();

        // Paleta con colores conocidos
        let mut pal = vec![0u8; PALETTE_DATA_SIZE as usize];
        // Entrada 0: rojo puro
        pal[0] = 0xFF;
        pal[1] = 0x00;
        pal[2] = 0x00;
        // Entrada 1: verde puro
        pal[3] = 0x00;
        pal[4] = 0xFF;
        pal[5] = 0x00;

        // Framebuffer: dos píxeles, uno rojo (índice 0) uno verde (índice 1)
        let mut fb = vec![0u8; FRAMEBUFFER_DATA_SIZE as usize];
        fb[0] = 0;
        fb[1] = 1;

        fb_rgba.convert(&fb, &pal);

        // Verificar píxel 0 (rojo)
        assert_eq!(fb_rgba.data[0], 0xFF);
        assert_eq!(fb_rgba.data[1], 0x00);
        assert_eq!(fb_rgba.data[2], 0x00);
        assert_eq!(fb_rgba.data[3], 0xFF);

        // Verificar píxel 1 (verde)
        assert_eq!(fb_rgba.data[4], 0x00);
        assert_eq!(fb_rgba.data[5], 0xFF);
        assert_eq!(fb_rgba.data[6], 0x00);
        assert_eq!(fb_rgba.data[7], 0xFF);
    }

    /// Regresión del bug "al cambiar de programa la pantalla no se
    /// refresca": la sesión nueva debe continuar el contador de
    /// publicaciones de la anterior, no arrancar de cero.
    #[test]
    fn sesion_nueva_continua_el_seq_y_limpia_la_pantalla() {
        use std::sync::Mutex;
        // Un consumidor (panel o ventana desacoplada) ya pintó hasta seq 5.
        let since_consumidor = 5u64;
        let vieja = Mutex::new(VideoShared {
            rgba: Some(vec![1, 2, 3]),
            seq: since_consumidor,
            in_use: true,
        });

        // Reproducción del bug: una sesión que arranca de cero publica su
        // primer frame con seq 1, y el consumidor lo descarta (1 <= 5):
        // exactamente la condición de filtrado de read_framebuffer.
        let desincronizada = VideoShared::default();
        let primer_frame = desincronizada.seq + 1;
        assert!(
            primer_frame <= since_consumidor,
            "con seq desde cero el primer frame quedaría filtrado"
        );

        // Con el fix: el contador continúa y la pantalla limpia llega.
        let nueva = VideoShared::para_sesion_nueva(Some(&vieja));
        assert!(
            nueva.seq > since_consumidor,
            "el frame inicial de la sesión nueva debe superar el seq del consumidor"
        );
        assert!(!nueva.in_use, "el uso de video se detecta por programa (RF4)");
        let negro = nueva.rgba.expect("pantalla limpia publicada");
        assert_eq!(negro.len(), 320 * 240 * 4);
        assert_eq!(&negro[0..4], &[0, 0, 0, 255], "frame negro opaco");
    }

    #[test]
    fn sesion_nueva_sin_anterior_arranca_de_cero() {
        let v = VideoShared::para_sesion_nueva(None);
        assert_eq!(v.seq, 0);
        assert!(v.rgba.is_none());
        assert!(!v.in_use);
    }

    #[test]
    fn sesion_nueva_tras_sesion_sin_video_no_publica() {
        use std::sync::Mutex;
        let vieja = Mutex::new(VideoShared {
            rgba: None,
            seq: 0,
            in_use: false,
        });
        let nueva = VideoShared::para_sesion_nueva(Some(&vieja));
        assert!(nueva.rgba.is_none(), "sin uso previo no hay nada que limpiar");
        assert_eq!(nueva.seq, 0);
    }

    #[test]
    fn paleta_vga13h_primeros_16_colores() {
        let pal = palette_vga13h_default();

        // Verificar entrada 0 (negro)
        assert_eq!(pal[0], 0x00);
        assert_eq!(pal[1], 0x00);
        assert_eq!(pal[2], 0x00);

        // Verificar entrada 15 (blanco)
        assert_eq!(pal[45], 0xFF);
        assert_eq!(pal[46], 0xFF);
        assert_eq!(pal[47], 0xFF);
    }
}
