use crate::Result;

/// Icon tile geometry. The tile is a rounded square; the monogram is drawn on top.
pub const SIZE: u32 = 128;
const TILE_INSET: f32 = 3.0;
const TILE_RADIUS: f32 = 24.0;
const TEXT_HEIGHT: u32 = 56;
const TEXT_MAX_WIDTH: u32 = 92;
const OUTLINE_RADIUS: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
    pub fn hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
    /// WCAG relative luminance. Used to pick a readable text and outline color.
    pub fn luminance(&self) -> f32 {
        fn channel(v: u8) -> f32 {
            let v = v as f32 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
    pub fn contrast(&self, other: &Rgb) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }
}

/// Fixed palette used for automatically generated icons.
///
/// Hues are spread around the circle at a constant OKLCH lightness and chroma
/// so no entry looks louder than the others. Every entry is dark enough that the
/// white monogram wins the contrast check, which keeps the icon family uniform.
pub const PALETTE: [Rgb; 12] = [
    Rgb::new(0x9F, 0x4D, 0x48), // 赤
    Rgb::new(0x9C, 0x52, 0x2E), // 錆
    Rgb::new(0x93, 0x5A, 0x11), // 琥珀
    Rgb::new(0x31, 0x7A, 0x45), // 緑
    Rgb::new(0x00, 0x7D, 0x5F), // エメラルド
    Rgb::new(0x00, 0x7C, 0x77), // 青緑
    Rgb::new(0x00, 0x7A, 0x87), // ティール
    Rgb::new(0x00, 0x75, 0x98), // ペトロール
    Rgb::new(0x31, 0x6C, 0xA5), // 青
    Rgb::new(0x57, 0x62, 0xA8), // インディゴ
    Rgb::new(0x73, 0x59, 0x9E), // 紫
    Rgb::new(0x8C, 0x50, 0x87), // マゼンタ
];

/// Accepts only `#RRGGBB`.
pub fn parse_color(text: &str) -> Result<Rgb> {
    let hex = text
        .strip_prefix('#')
        .ok_or("色は #RRGGBB 形式で指定してください")?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("色は #RRGGBB 形式で指定してください".into());
    }
    let value = u32::from_str_radix(hex, 16).map_err(|_| "色は #RRGGBB 形式で指定してください")?;
    Ok(Rgb::new(
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ))
}

/// Stable color for an application id.
///
/// The hash is FNV-1a, so the color only depends on the id: the same id always
/// maps to the same palette entry, on every machine and every run.
pub fn color_for_id(id: &str) -> Rgb {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    PALETTE[(hash % PALETTE.len() as u64) as usize]
}

/// Text and outline colors for a tile background.
///
/// The text color is the one with the better contrast against the background.
/// The outline uses the opposite tone, so the glyph keeps a visible contour on
/// light and on dark taskbars.
pub fn text_colors(background: &Rgb) -> (Rgb, Rgb) {
    const DARK: Rgb = Rgb::new(0x00, 0x00, 0x00);
    const LIGHT: Rgb = Rgb::new(0xFF, 0xFF, 0xFF);
    if DARK.contrast(background) > LIGHT.contrast(background) {
        (DARK, LIGHT)
    } else {
        (LIGHT, DARK)
    }
}

/// One or two characters taken from a display name.
///
/// The initials of the first two words are used. A single word contributes its
/// first character, plus the second one when it is an ASCII letter or digit.
pub fn monogram(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let mut letters: Vec<char> = Vec::new();
    for word in &words {
        if let Some(c) = word.chars().find(|c| c.is_alphanumeric()) {
            letters.push(c);
        }
        if letters.len() == 2 {
            break;
        }
    }
    if letters.is_empty()
        && let Some(c) = name.trim().chars().next()
    {
        letters.push(c);
    }
    if letters.len() == 1 {
        // A single word may contribute a second ASCII character.
        if let Some(word) = words.first()
            && let Some(second) = word.chars().filter(|c| c.is_ascii_alphanumeric()).nth(1)
        {
            letters.push(second);
        }
    }
    letters
        .into_iter()
        .map(|c| {
            if c.is_ascii() {
                c.to_ascii_uppercase()
            } else {
                c
            }
        })
        .collect()
}

/// Renders the icon tile as RGBA bytes.
pub fn render(text: &str, background: Rgb) -> Vec<u8> {
    let (ink, outline) = text_colors(&background);
    let mut canvas = vec![0u8; (SIZE * SIZE * 4) as usize];
    paint_tile(&mut canvas, &background);
    let mask = super::font::render_monogram(text, TEXT_HEIGHT, TEXT_MAX_WIDTH);
    let (mask, mask_w, mask_h) = (mask.pixels, mask.width, mask.height);
    let outline_mask = dilate(&mask, mask_w, mask_h, OUTLINE_RADIUS);
    let x0 = (SIZE.saturating_sub(mask_w)) / 2;
    let y0 = (SIZE.saturating_sub(mask_h)) / 2;
    for y in 0..mask_h {
        for x in 0..mask_w {
            let o = outline_mask[(y * mask_w + x) as usize];
            if o > 0 {
                blend(&mut canvas, x0 + x, y0 + y, &outline, o);
            }
        }
    }
    for y in 0..mask_h {
        for x in 0..mask_w {
            let a = mask[(y * mask_w + x) as usize];
            if a > 0 {
                blend(&mut canvas, x0 + x, y0 + y, &ink, a);
            }
        }
    }
    canvas
}

fn blend(canvas: &mut [u8], x: u32, y: u32, color: &Rgb, alpha: u8) {
    if x >= SIZE || y >= SIZE {
        return;
    }
    let a = f32::from(alpha) / 255.0;
    let offset = ((y * SIZE + x) * 4) as usize;
    for (index, channel) in [color.r, color.g, color.b].into_iter().enumerate() {
        let dst = canvas[offset + index] as f32;
        let value = f32::from(channel) * a + dst * (1.0 - a);
        canvas[offset + index] = value.round().clamp(0.0, 255.0) as u8;
    }
    let dst = canvas[offset + 3] as f32;
    canvas[offset + 3] = (255.0 * a + dst * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
}

fn paint_tile(canvas: &mut [u8], color: &Rgb) {
    for y in 0..SIZE {
        for x in 0..SIZE {
            let coverage = tile_coverage(x as f32 + 0.5, y as f32 + 0.5);
            if coverage > 0.0 {
                blend(canvas, x, y, color, (coverage * 255.0).round() as u8);
            }
        }
    }
}

/// Anti-aliased coverage of the rounded square tile.
fn tile_coverage(x: f32, y: f32) -> f32 {
    let half = SIZE as f32 / 2.0 - TILE_INSET;
    let center = SIZE as f32 / 2.0;
    let qx = (x - center).abs() - (half - TILE_RADIUS);
    let qy = (y - center).abs() - (half - TILE_RADIUS);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    let inside = qx.max(qy).min(0.0);
    let distance = outside + inside - TILE_RADIUS;
    (0.5 - distance).clamp(0.0, 1.0)
}

/// Grows a coverage mask so a contrasting outline can be drawn around it.
fn dilate(mask: &[u8], w: u32, h: u32, radius: u32) -> Vec<u8> {
    if radius == 0 || w == 0 || h == 0 {
        return mask.to_vec();
    }
    let mut horizontal = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let mut best = 0u8;
            let start = x.saturating_sub(radius);
            let end = (x + radius).min(w - 1);
            for sx in start..=end {
                best = best.max(mask[(y * w + sx) as usize]);
            }
            horizontal[(y * w + x) as usize] = best;
        }
    }
    let mut out = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let mut best = 0u8;
            let start = y.saturating_sub(radius);
            let end = (y + radius).min(h - 1);
            for sy in start..=end {
                best = best.max(horizontal[(sy * w + x) as usize]);
            }
            out[(y * w + x) as usize] = best;
        }
    }
    out
}
