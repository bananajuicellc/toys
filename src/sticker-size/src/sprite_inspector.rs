use base64::prelude::*;
use ril::{prelude::*};

pub fn to_data_uri(file_name: &str, image_bytes: &[u8]) -> String {
    let extension = if file_name.ends_with("gif") {
        "gif"
    } else {
        "png"
    };
    let data = BASE64_STANDARD.encode(image_bytes);
    let url_data = urlencoding::encode(&data);
    format!("data:image/{extension};base64,{url_data}")
}

/// Checks if a PNG byte slice contains any transparent pixels using the `ril` crate.
pub fn has_transparency(image_bytes: &[u8]) -> Result<bool, String> {
    // 1. Decode explicitly into an RGBA image matrix in-memory
    let image: Image<Rgba> = Image::from_bytes_inferred(image_bytes)
        .map_err(|e| format!("Failed to decode PNG via RIL: {e}"))?;

    // 2. Walk linearly through the typed pixel representations
    for row in image.pixels() {
        for pixel in row {
            // 'a' is a u8 component representing alpha (0 = transparent, 255 = fully opaque)
            if pixel.a < 255 {
                return Ok(true); // Found transparency!
            }
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alpha_with_ril() {
        // Standard native filesystem call
        let file_bytes = std::fs::read("test_assets/logo.png")
            .expect("Couldn't open test asset");
        
        let result = has_transparency(&file_bytes).unwrap();
        assert!(result);
    }
}
