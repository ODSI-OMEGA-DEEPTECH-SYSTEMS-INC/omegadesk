//! Conversão de cor BGRA → I420 (YUV 4:2:0), BT.601 faixa limitada, em inteiros.

/// Converte BGRA para I420. `width` e `height` precisam ser pares.
///
/// Devolve os planos `(y, u, v)`: Y com `width * height` bytes e U/V com um
/// quarto disso cada.
#[must_use]
pub fn bgra_to_i420(bgra: &[u8], width: usize, height: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    debug_assert!(width.is_multiple_of(2) && height.is_multiple_of(2));
    let stride = width * 4;
    let mut y_plane = vec![0u8; width * height];
    let mut u_plane = vec![0u8; width * height / 4];
    let mut v_plane = vec![0u8; width * height / 4];

    for row in (0..height).step_by(2) {
        let top = &bgra[row * stride..(row + 1) * stride];
        let bottom = &bgra[(row + 1) * stride..(row + 2) * stride];
        let (y_top, y_bottom) = y_plane[row * width..(row + 2) * width].split_at_mut(width);
        let uv_row = row / 2 * (width / 2);

        for col in (0..width).step_by(2) {
            let mut sum_r = 0i32;
            let mut sum_g = 0i32;
            let mut sum_b = 0i32;
            for (line, y_line) in [(top, &mut *y_top), (bottom, &mut *y_bottom)] {
                for dx in 0..2 {
                    let px = (col + dx) * 4;
                    let b = i32::from(line[px]);
                    let g = i32::from(line[px + 1]);
                    let r = i32::from(line[px + 2]);
                    y_line[col + dx] = luma(r, g, b);
                    sum_r += r;
                    sum_g += g;
                    sum_b += b;
                }
            }
            let (r, g, b) = (sum_r / 4, sum_g / 4, sum_b / 4);
            u_plane[uv_row + col / 2] = clamp_u8(((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128);
            v_plane[uv_row + col / 2] = clamp_u8(((112 * r - 94 * g - 18 * b + 128) >> 8) + 128);
        }
    }
    (y_plane, u_plane, v_plane)
}

fn luma(r: i32, g: i32, b: i32) -> u8 {
    clamp_u8(((66 * r + 129 * g + 25 * b + 128) >> 8) + 16)
}

fn clamp_u8(value: i32) -> u8 {
    u8::try_from(value.clamp(0, 255)).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::bgra_to_i420;

    #[test]
    fn black_and_white() {
        let white = [255u8; 2 * 2 * 4];
        let (y, u, v) = bgra_to_i420(&white, 2, 2);
        assert!(y.iter().all(|&l| l == 235));
        assert_eq!((u[0], v[0]), (128, 128));

        let black = [0u8, 0, 0, 255].repeat(4);
        let (y, u, v) = bgra_to_i420(&black, 2, 2);
        assert!(y.iter().all(|&l| l == 16));
        assert_eq!((u[0], v[0]), (128, 128));
    }

    #[test]
    fn plane_sizes() {
        let frame = vec![0u8; 8 * 4 * 4];
        let (y, u, v) = bgra_to_i420(&frame, 8, 4);
        assert_eq!((y.len(), u.len(), v.len()), (32, 8, 8));
    }
}
