use std::path::Path;

pub struct TextureData {
    pub width: u32,
    pub height: u32,
    /// RGBA8 пиксели (width * height * 4 байт).
    pub pixels: Vec<u8>,
}

/// Загружает изображение как RGBA8. Поддерживает PNG, JPEG, HDR.
pub fn load(path: &Path) -> TextureData {
    let img = image::open(path)
        .unwrap_or_else(|e| panic!("Failed to load image '{}': {}", path.display(), e));

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    log::debug!("Loaded image '{}': {}x{}", path.display(), width, height);

    TextureData {
        width,
        height,
        pixels: rgba.into_raw(),
    }
}
