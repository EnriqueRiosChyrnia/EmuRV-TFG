//! Overlay de SPRITES por hardware (RP31, decisión (a) del spike F1).
//!
//! Una tabla de sprites en MMIO (`SPRITE_TABLE_BASE`) y una memoria de patrones
//! (`SPRITE_PATTERN_BASE`) se componen sobre el framebuffer INDEXADO en Rust,
//! ANTES de convertir a RGBA (ver `machine::publicar_frame`). El programa
//! «mueve» un sprite actualizando su (x, y) en la tabla, sin redibujar a mano.
//!
//! Clave para la regla dura «video 100% intacto»: la composición es ADITIVA y
//! el camino SIN sprites queda bit-idéntico (si la tabla está vacía,
//! `publicar_frame` convierte el framebuffer tal cual). Estas funciones son
//! PURAS y testeables sin Unicorn.
//!
//! Layout (ver `core/mod.rs`):
//! - Tabla: 64 entradas × 8 bytes, little-endian:
//!   x:i16, y:i16 (con signo, para recorte fuera de pantalla por izquierda/
//!   arriba), pattern:u8 (índice de patrón), flags:u8 (bit0=habilitado),
//!   2 bytes reservados.
//! - Patrones: 64 patrones de 16×16 píxeles indexados (256 bytes c/u). El
//!   índice 0 es TRANSPARENTE (color-key): no se pinta, deja ver el fondo.
//! - Orden de pintado: sprite 0 al fondo, 63 arriba.

use super::{
    FRAMEBUFFER_DATA_SIZE, SPRITE_COUNT, SPRITE_ENTRY_SIZE, SPRITE_PATTERN_BYTES, SPRITE_PATTERN_DIM,
};

/// Ancho/alto de la pantalla indexada (320×240). Privado: el resto del módulo
/// los toma de aquí para no repetir literales.
const ANCHO: i32 = 320;
const ALTO: i32 = 240;

/// ¿Hay al menos un sprite habilitado en la tabla? Señal de «sprites en uso»
/// (espejo de la detección de uso de la VRAM): barata, recorre solo el byte de
/// flags de cada entrada. Tolera una tabla más corta de lo esperado.
pub fn hay_sprites(tabla: &[u8]) -> bool {
    (0..SPRITE_COUNT).any(|i| tabla.get(i * SPRITE_ENTRY_SIZE + 5).is_some_and(|f| f & 1 != 0))
}

/// Compone los sprites habilitados sobre una copia del framebuffer indexado y
/// la devuelve. `fb` debe ser de `FRAMEBUFFER_DATA_SIZE` bytes (320×240
/// indexado); `tabla` la tabla de sprites; `patrones` la memoria de patrones.
/// Píxel de índice 0 = transparente; lo que cae fuera de la pantalla se
/// recorta. Si los buffers vienen cortos (lectura fallida), se saltan las
/// entradas que no se pueden resolver, sin pánico.
pub fn componer(fb: &[u8], tabla: &[u8], patrones: &[u8]) -> Vec<u8> {
    let mut out = fb.to_vec();
    // Defensa: si el framebuffer no tiene el tamaño esperado, no se compone
    // nada (devolver la copia tal cual es lo más seguro).
    if out.len() != FRAMEBUFFER_DATA_SIZE as usize {
        return out;
    }
    for i in 0..SPRITE_COUNT {
        let base = i * SPRITE_ENTRY_SIZE;
        // La entrada completa debe caber en la tabla.
        let Some(entrada) = tabla.get(base..base + SPRITE_ENTRY_SIZE) else {
            continue;
        };
        if entrada[5] & 1 == 0 {
            continue; // sprite deshabilitado
        }
        let x = i16::from_le_bytes([entrada[0], entrada[1]]) as i32;
        let y = i16::from_le_bytes([entrada[2], entrada[3]]) as i32;
        let patron = entrada[4] as usize;
        let pbase = patron * SPRITE_PATTERN_BYTES;
        let Some(pixeles) = patrones.get(pbase..pbase + SPRITE_PATTERN_BYTES) else {
            continue; // patrón fuera de la memoria de patrones
        };
        for fila in 0..SPRITE_PATTERN_DIM {
            let sy = y + fila as i32;
            if !(0..ALTO).contains(&sy) {
                continue; // fila fuera de pantalla (arriba/abajo)
            }
            for col in 0..SPRITE_PATTERN_DIM {
                let sx = x + col as i32;
                if !(0..ANCHO).contains(&sx) {
                    continue; // columna fuera de pantalla (izquierda/derecha)
                }
                let px = pixeles[fila * SPRITE_PATTERN_DIM + col];
                if px == 0 {
                    continue; // índice 0 = transparente (color-key)
                }
                out[(sy * ANCHO + sx) as usize] = px;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Framebuffer indexado en negro (todo índice 0).
    fn fb_negro() -> Vec<u8> {
        vec![0u8; FRAMEBUFFER_DATA_SIZE as usize]
    }

    /// Tabla de sprites vacía (64 entradas en cero).
    fn tabla_vacia() -> Vec<u8> {
        vec![0u8; SPRITE_COUNT * SPRITE_ENTRY_SIZE]
    }

    /// Memoria de patrones vacía.
    fn patrones_vacios() -> Vec<u8> {
        vec![0u8; SPRITE_COUNT * SPRITE_PATTERN_BYTES]
    }

    /// Escribe una entrada de sprite en la tabla.
    fn set_entrada(tabla: &mut [u8], i: usize, x: i16, y: i16, patron: u8, enable: bool) {
        let base = i * SPRITE_ENTRY_SIZE;
        tabla[base..base + 2].copy_from_slice(&x.to_le_bytes());
        tabla[base + 2..base + 4].copy_from_slice(&y.to_le_bytes());
        tabla[base + 4] = patron;
        tabla[base + 5] = if enable { 1 } else { 0 };
    }

    /// Llena el patrón `p` con un color sólido (mismo índice en los 256 píxeles).
    fn patron_solido(patrones: &mut [u8], p: usize, color: u8) {
        let base = p * SPRITE_PATTERN_BYTES;
        for b in &mut patrones[base..base + SPRITE_PATTERN_BYTES] {
            *b = color;
        }
    }

    fn px(fb: &[u8], x: i32, y: i32) -> u8 {
        fb[(y * 320 + x) as usize]
    }

    #[test]
    fn hay_sprites_detecta_habilitados() {
        let mut t = tabla_vacia();
        assert!(!hay_sprites(&t), "tabla vacía: sin sprites");
        set_entrada(&mut t, 3, 0, 0, 0, true);
        assert!(hay_sprites(&t), "una entrada habilitada basta");
        set_entrada(&mut t, 3, 0, 0, 0, false);
        assert!(!hay_sprites(&t), "deshabilitarla la apaga");
    }

    #[test]
    fn sprite_se_dibuja_en_su_posicion() {
        let mut t = tabla_vacia();
        let mut p = patrones_vacios();
        patron_solido(&mut p, 1, 5); // patrón 1 = color 5 sólido
        set_entrada(&mut t, 0, 10, 20, 1, true);
        let out = componer(&fb_negro(), &t, &p);
        // Esquina superior izquierda del sprite y esquina inferior derecha.
        assert_eq!(px(&out, 10, 20), 5, "esquina (10,20) pintada");
        assert_eq!(px(&out, 25, 35), 5, "esquina (25,35) = (10+15, 20+15)");
        // Justo afuera del sprite sigue en el fondo.
        assert_eq!(px(&out, 9, 20), 0, "una columna a la izquierda: fondo");
        assert_eq!(px(&out, 26, 20), 0, "una columna a la derecha: fondo");
    }

    #[test]
    fn indice_0_es_transparente() {
        let mut t = tabla_vacia();
        let mut p = patrones_vacios();
        // Patrón 2: solo el píxel (0,0) es color 7; el resto índice 0.
        p[2 * SPRITE_PATTERN_BYTES] = 7;
        set_entrada(&mut t, 0, 0, 0, 2, true);
        // Fondo no-cero para distinguir transparencia de «pintó 0».
        let mut fb = fb_negro();
        for b in fb.iter_mut() {
            *b = 3;
        }
        let out = componer(&fb, &t, &p);
        assert_eq!(px(&out, 0, 0), 7, "el píxel opaco pisa el fondo");
        assert_eq!(px(&out, 1, 0), 3, "el píxel índice 0 deja ver el fondo");
        assert_eq!(px(&out, 15, 15), 3, "todo el resto del sprite es transparente");
    }

    #[test]
    fn sprite_parcialmente_fuera_se_recorta_sin_panico() {
        let mut t = tabla_vacia();
        let mut p = patrones_vacios();
        patron_solido(&mut p, 0, 9);
        // x=315: solo las columnas 315..320 (5) caben; el resto se recorta.
        // y=-3: las filas 0..3 del patrón quedan arriba de la pantalla.
        set_entrada(&mut t, 0, 315, -3, 0, true);
        let out = componer(&fb_negro(), &t, &p);
        assert_eq!(px(&out, 319, 0), 9, "columna visible más a la derecha pintada");
        assert_eq!(px(&out, 315, 0), 9, "primera columna visible pintada");
        // No hay forma de comprobar «no escribió fuera» salvo que no entró en
        // pánico y el tamaño no cambió:
        assert_eq!(out.len(), FRAMEBUFFER_DATA_SIZE as usize);
    }

    #[test]
    fn sprite_totalmente_fuera_no_cambia_nada() {
        let mut t = tabla_vacia();
        let mut p = patrones_vacios();
        patron_solido(&mut p, 0, 9);
        set_entrada(&mut t, 0, 400, 400, 0, true); // fuera de 320×240
        let fb = fb_negro();
        let out = componer(&fb, &t, &p);
        assert_eq!(out, fb, "un sprite totalmente fuera no toca el framebuffer");
    }

    #[test]
    fn sprite_deshabilitado_no_se_dibuja() {
        let mut t = tabla_vacia();
        let mut p = patrones_vacios();
        patron_solido(&mut p, 0, 9);
        set_entrada(&mut t, 0, 10, 10, 0, false); // enable=false
        let fb = fb_negro();
        let out = componer(&fb, &t, &p);
        assert_eq!(out, fb, "sin habilitar, el sprite no se compone");
    }

    #[test]
    fn orden_de_pintado_sprite_mayor_arriba() {
        let mut t = tabla_vacia();
        let mut p = patrones_vacios();
        patron_solido(&mut p, 1, 4);
        patron_solido(&mut p, 2, 6);
        // Dos sprites solapados en (0,0): el de índice MAYOR (1) pinta encima.
        set_entrada(&mut t, 0, 0, 0, 2, true); // fondo del overlay (color 6)
        set_entrada(&mut t, 1, 0, 0, 1, true); // encima (color 4)
        let out = componer(&fb_negro(), &t, &p);
        assert_eq!(px(&out, 0, 0), 4, "el sprite de índice mayor queda arriba");
    }

    /// El camino SIN sprites es bit-idéntico al framebuffer (video intacto).
    #[test]
    fn sin_sprites_devuelve_el_framebuffer_tal_cual() {
        let mut fb = fb_negro();
        fb[100] = 12;
        fb[76799] = 7;
        let out = componer(&fb, &tabla_vacia(), &patrones_vacios());
        assert_eq!(out, fb, "tabla vacía: composición = framebuffer original");
    }
}
