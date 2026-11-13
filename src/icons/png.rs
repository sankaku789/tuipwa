use crate::Result;
use std::path::Path;

/// Encodes RGBA pixels as a PNG file.
pub fn write_png(path: &Path, pixels: &[u8], width: u32, height: u32) -> Result<()> {
    if pixels.len() != (width * height * 4) as usize {
        return Err("PNGピクセル数が不正です".into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    Ok(())
}

/// Reads a PNG file back as RGBA pixels. Used by the tests.
#[cfg(test)]
pub fn read_png(path: &Path) -> Result<(Vec<u8>, u32, u32)> {
    let file = std::fs::File::open(path)?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("PNGのバッファサイズ不明です")?
    ];
    let info = reader.next_frame(&mut buffer)?;
    let color = info.color_type;
    let depth = info.bit_depth;
    if color != png::ColorType::Rgba || depth != png::BitDepth::Eight {
        return Err(format!("想定外のPNG形式です: {color:?}/{depth:?}").into());
    }
    Ok((buffer, info.width, info.height))
}
