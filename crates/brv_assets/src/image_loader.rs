use crate::TextureData;

pub fn load(full_path: &str) -> TextureData {
    let img = image::open(full_path)
        .unwrap_or_else(|e| panic!("Failed to load image {}: {}", full_path, e))
        .into_rgba8();

    let width = img.width();
    let height = img.height();
    let pixels = img.into_raw();

    TextureData { pixels, width, height }
}
