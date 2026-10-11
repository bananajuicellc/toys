use base64::prelude::*;
use ril::prelude::*;

struct Point {
    x: i32,
    y: i32,
}

struct Line {
    left: Point,
    right: Point,
}

#[derive(Debug)]
struct Edge {
    left: i32,
    right: i32,
}

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

fn is_transparent(pixel: &Rgba) -> bool {
    pixel.a < 255
}

fn find_edges(image: &Image<Rgba>) -> Vec<Option<Edge>> {
    let mut out: Vec<Option<Edge>> =
        Vec::with_capacity(image.height().try_into().expect("image too tall"));
    for row in image.pixels() {
        let mut left = 0;
        let mut right: i32 = row.len().try_into().expect("image too wide");
        right -= 1;
        for pixel in row.iter() {
            if is_transparent(pixel) {
                left += 1;
            } else {
                break;
            }
        }

        for pixel in row.iter().rev() {
            if right < left {
                // Whole row is transparent.
                break;
            } else if is_transparent(pixel) {
                right -= 1;
            } else {
                break;
            }
        }

        out.push(if left <= right {
            Some(Edge { left, right })
        } else {
            None
        });
    }
    out
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
    use std::assert_matches;

use super::*;

    #[test]
    fn test_find_edges_with_all_filled() {
       let image = Image::new(256, 256, Rgba::new(255, 0, 0, 255)); 
       let got = find_edges(&image);
       for edge in got {
        let edge = edge.expect("got unexpected fully tranparent row");
        assert_eq!(edge.left, 0);
        assert_eq!(edge.right, 255);
       }
    }

    #[test]
    fn test_find_edges_with_empty_rows() {
       let mut image = Image::new(256, 5, Rgba::new(0, 0, 0, 0)); 
       ril::draw::Line::new((1, 1), (254, 1), Rgba::new(255, 0, 0, 255)).draw(&mut image);
       ril::draw::Line::new((128, 2), (129, 2), Rgba::new(255, 0, 0, 255)).draw(&mut image);
       ril::draw::Line::new((12, 3), (246, 3), Rgba::new(255, 0, 0, 255)).draw(&mut image);
       
       let mut got = find_edges(&image).into_iter();

       let first = got.next();
       assert_matches!(first, Some(None));
       let second = got.next();
       if let Some(Some(edge)) = second {
        assert_eq!(edge.left, 1);
        assert_eq!(edge.right, 253);
       } else {
        panic!("missing 2nd row edges");
       }
       let third = got.next();
       if let Some(Some(edge)) = third {
        assert_eq!(edge.left, 128);
        assert_eq!(edge.right, 128);
       } else {
        panic!("missing 3rd row edges");
       }
       let fourth = got.next();
       if let Some(Some(edge)) = fourth {
        assert_eq!(edge.left, 12);
        assert_eq!(edge.right, 245);
       } else {
        panic!("missing 4th row edges");
       }
       let last = got.next();
       assert_matches!(last, Some(None));
       let end = got.next();
       assert_matches!(end, None);
       
    }
}
