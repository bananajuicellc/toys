use base64::prelude::*;
use ril::prelude::*;

#[derive(Debug, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Debug, PartialEq)]
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

fn calculate_square_length(left: (i32, i32), right: (i32, i32)) -> f32 {
    let dx: f32 = (right.0 - left.0).abs() as f32;
    let dy: f32 = (right.1 - left.1).abs() as f32;
    dx * dx + dy * dy
}

fn find_longest_dimension(edges: &Vec<Option<Edge>>) -> Option<Line> {
    let mut max_distance_square = 0.0;
    let mut max_dimension: Option<Line> = None;

    for (left_y, left) in edges.iter().enumerate() {
        match left {
            Some(left) => {
                let left_x = left.left;
                for (right_y, right) in edges.iter().enumerate() {
                    match right {
                        Some(right) => {
                            let right_x = right.right;
                            let distance_square = calculate_square_length(
                                (left_x, left_y.try_into().expect("image too tall")),
                                (right_x, right_y.try_into().expect("image too tall")),
                            );
                            if distance_square > max_distance_square {
                                max_distance_square = distance_square;
                                max_dimension = Some(Line {
                                    left: Point {
                                        x: left_x,
                                        y: left_y.try_into().expect("image too tall"),
                                    },
                                    right: Point {
                                        x: right_x,
                                        y: right_y.try_into().expect("image too tall"),
                                    },
                                });
                            }
                        }
                        None => continue,
                    }
                }
            }
            None => continue,
        }
    }
    max_dimension
}

pub fn draw_largest_dimension(image_bytes: &[u8]) -> Result<(Vec<u8>, Option<f32>), String> {
    let mut image: Image<Rgba> = Image::from_bytes_inferred(image_bytes)
        .map_err(|e| format!("Failed to decode image via RIL: {e}"))?;
    let edges = find_edges(&image);
    let dimension = find_longest_dimension(&edges);
    let measure = match dimension {
        Some(line) => {
            ril::draw::Line::new(
                (
                    line.left.x.try_into().expect("unexpected negative bounds"),
                    line.left.y.try_into().expect("unexpected negative bounds"),
                ),
                (
                    line.right.x.try_into().expect("unexpected negative bounds"),
                    line.right.y.try_into().expect("unexpected negative bounds"),
                ),
                Rgba::new(255, 0, 0, 255),
            )
            .draw(&mut image);
            let measure = calculate_square_length((line.left.x, line.left.y), (line.right.x, line.right.y))
                    .sqrt()
                     // Add 1.0 to account for going to the edge of the pixel from the center of each endpoint.
                    + 1.0;
            Some(measure)
        }
        None => None,
    };

    let mut out = Vec::<u8>::new();
    image
        .encode(ImageFormat::Png, &mut out)
        .map_err(|e| format!("Failed to encode PNG via RIL: {e}"))?;
    Ok((out, measure))
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;

    #[test]
    fn test_calculate_square_length() {
        assert_eq!(0.0, calculate_square_length((0, 0), (0, 0)));
    }

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

    #[test]
    fn test_find_longest_dimension_with_wide_image() {
        let edges = vec![
            Some(Edge { left: 2, right: 2 }),
            Some(Edge { left: 0, right: 4 }),
            Some(Edge { left: 2, right: 2 }),
        ];
        let got = find_longest_dimension(&edges);
        assert_eq!(
            got,
            Some(Line {
                left: Point { x: 0, y: 1 },
                right: Point { x: 4, y: 1 }
            })
        );
    }
}
