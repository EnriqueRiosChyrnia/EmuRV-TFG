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
