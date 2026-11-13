use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// A rendered glyph run as an 8 bit coverage mask.
pub struct Mask {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl Mask {
    fn empty() -> Self {
        Self {
            pixels: Vec::new(),
            width: 0,
            height: 0,
        }
    }
}

/// Font discovery and monogram rasterization.
///
/// Fonts are taken from the running system so that real display names,
/// including CJK names, render correctly. When no usable font exists the
/// monogram degrades to a deterministic geometric mark that still differs per
/// character, so icons stay distinguishable and reproducible.
pub fn render_monogram(text: &str, target_height: u32, max_width: u32) -> Mask {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Mask::empty();
    }
    if let Some(font) = system_font(&chars)
        && let Some(mask) = rasterize(&font, &chars, target_height, max_width)
    {
        return mask;
    }
    geometric(&chars, target_height, max_width)
}

fn system_font(chars: &[char]) -> Option<Font> {
    static CACHE: OnceLock<Vec<Font>> = OnceLock::new();
    let fonts = CACHE.get_or_init(load_fonts);
    fonts
        .iter()
        .find(|font| chars.iter().all(|c| font.has_glyph(*c)))
        .cloned()
}

fn load_fonts() -> Vec<Font> {
    let mut fonts = Vec::new();
    for path in font_files() {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        // Font collections (.ttc) hold several faces; try each index.
        let indices: &[u32] = if path.extension() == Some(OsStr::new("ttc")) {
            &[0, 1, 2, 3]
        } else {
            &[0]
        };
        for index in indices {
            let settings = FontSettings {
                collection_index: *index,
                ..FontSettings::default()
            };
            if let Ok(font) = Font::from_bytes(data.clone(), settings) {
                fonts.push(font);
                break;
            }
        }
        if fonts.len() >= 8 {
            break;
        }
    }
    fonts
}

/// Collects font files from the usual system locations.
fn font_files() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").filter(|v| Path::new(v).is_absolute()) {
        roots.push(PathBuf::from(data).join("fonts"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.push(home.join(".fonts"));
        roots.push(home.join(".local/share/fonts"));
    }
    roots.push(PathBuf::from("/usr/share/fonts"));
    roots.push(PathBuf::from("/usr/local/share/fonts"));
    let mut files = Vec::new();
    for root in roots {
        collect(&root, 0, &mut files);
    }
    files.sort();
    files
}

fn collect(dir: &Path, depth: u32, files: &mut Vec<PathBuf>) {
    if depth > 6 || files.len() > 512 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut names: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    names.sort();
    for path in names {
        if path.is_dir() {
            collect(&path, depth + 1, files);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ttf" | "otf" | "ttc")
        ) {
            files.push(path);
        }
    }
}

/// Rasterizes the characters with a font and scales the result to fit.
fn rasterize(font: &Font, chars: &[char], target_height: u32, max_width: u32) -> Option<Mask> {
    // Supersample so downscaling keeps the glyph edges smooth.
    let px = (target_height as f32) * 2.0;
    let text: String = chars.iter().collect();
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings::default());
    layout.append(&[font], &TextStyle::new(&text, px, 0));
    let glyphs = layout.glyphs();
    if glyphs.is_empty() {
        return None;
    }
    let mut left = u32::MAX;
    let mut top = u32::MAX;
    let mut right = 0u32;
    let mut bottom = 0u32;
    let mut rasters = Vec::new();
    for glyph in glyphs {
        let (metrics, pixels) = font.rasterize_indexed(glyph.key.glyph_index, px);
        if metrics.width == 0 || metrics.height == 0 {
            continue;
        }
        let x = glyph.x.max(0.0) as u32;
        let y = glyph.y.max(0.0) as u32;
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + metrics.width as u32);
        bottom = bottom.max(y + metrics.height as u32);
        rasters.push((x, y, metrics.width as u32, metrics.height as u32, pixels));
    }
    if rasters.is_empty() {
        return None;
    }
    let width = right - left;
    let height = bottom - top;
    let mut canvas = vec![0u8; (width * height) as usize];
    for (x, y, w, h, pixels) in rasters {
        for row in 0..h {
            for column in 0..w {
                let value = pixels[(row * w + column) as usize];
                if value == 0 {
                    continue;
                }
                let tx = x - left + column;
                let ty = y - top + row;
                if tx >= width || ty >= height {
                    continue;
                }
                canvas[(ty * width + tx) as usize] = value;
            }
        }
    }
    Some(scale(canvas, width, height, target_height, max_width))
}

/// Scales a coverage mask so it fits the requested height and width.
fn scale(pixels: Vec<u8>, width: u32, height: u32, target_height: u32, max_width: u32) -> Mask {
    let factor = (target_height as f32 / height as f32)
        .min(max_width as f32 / width as f32)
        .min(1.0);
    let target_w = ((width as f32 * factor).round() as u32).max(1);
    let target_h = ((height as f32 * factor).round() as u32).max(1);
    if target_w == width && target_h == height {
        return Mask {
            pixels,
            width,
            height,
        };
    }
    let mut out = vec![0u8; (target_w * target_h) as usize];
    for y in 0..target_h {
        for x in 0..target_w {
            let x0 = (x * width) / target_w;
            let x1 = (((x + 1) * width) / target_w).max(x0 + 1);
            let y0 = (y * height) / target_h;
            let y1 = (((y + 1) * height) / target_h).max(y0 + 1);
            let mut total = 0u32;
            let mut count = 0u32;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    total += u32::from(pixels[(sy * width + sx) as usize]);
                    count += 1;
                }
            }
            out[(y * target_w + x) as usize] = (total / count.max(1)) as u8;
        }
    }
    Mask {
        pixels: out,
        width: target_w,
        height: target_h,
    }
}

/// Fallback monogram: a deterministic bar pattern derived from the characters.
fn geometric(chars: &[char], target_height: u32, max_width: u32) -> Mask {
    let height = target_height.max(8);
    let bar_width = (height / 8).max(1);
    let gap = bar_width.max(1);
    let mut width = 0u32;
    for ch in chars {
        let bars = 2 + (*ch as u32 % 3);
        width += bars * bar_width + (bars - 1) * gap + gap;
    }
    let width = width.min(max_width.max(1));
    let mut pixels = vec![0u8; (width * height) as usize];
    let mut x = 0u32;
    for ch in chars {
        let code = *ch as u32;
        let bars = 2 + (code % 3);
        for bar in 0..bars {
            let fill = 40 + ((code / (bar + 1)) % 5) * 15;
            let bar_x = x + bar * (bar_width + gap);
            for py in 0..height {
                for px in 0..bar_width {
                    let tx = bar_x + px;
                    if tx >= width {
                        continue;
                    }
                    let value = if (code / (bar + 1)).is_multiple_of(2) {
                        fill
                    } else {
                        255 - fill
                    };
                    pixels[(py * width + tx) as usize] = value as u8;
                }
            }
        }
        x += bars * bar_width + (bars - 1) * gap + gap;
    }
    Mask {
        pixels,
        width,
        height,
    }
}
