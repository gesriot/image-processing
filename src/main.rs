extern crate image;
extern crate rayon;

use image::{GenericImageView, ImageBuffer, ImageError, Rgba};
use rayon::prelude::*;
use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

#[allow(non_snake_case)]
#[derive(Copy, Clone)]
struct Point {
    X: u32,
    Y: u32,
}

#[allow(non_snake_case)]
#[derive(Copy, Clone)]
struct RGB {
    R: u8,
    G: u8,
    B: u8,
}

fn get_average_color_between_points(
    img: &image::DynamicImage,
    p1: Point,
    p2: Point,
) -> Result<RGB, ImageError> {
    let mut total_r = 0u32;
    let mut total_g = 0u32;
    let mut total_b = 0u32;
    let mut count = 0u32;

    for x in p1.X..=p2.X {
        let pixel = img.get_pixel(x, p1.Y);
        total_r += pixel[0] as u32;
        total_g += pixel[1] as u32;
        total_b += pixel[2] as u32;
        count += 1;
    }

    Ok(RGB {
        R: (total_r / count) as u8,
        G: (total_g / count) as u8,
        B: (total_b / count) as u8,
    })
}

fn interpolate(y1: f64, y2: f64, x1: u32, x2: u32, x: u32) -> f64 {
    y1 + (y2 - y1) * (x - x1) as f64 / (x2 - x1) as f64
}

fn get_alfa(tap: f64) -> f64 {
    let tap = if tap < 0.0 { 0.0 } else { tap };
    if tap < 50.0 {
        255.0 * tap / 50.0
    } else {
        255.0
    }
}

fn get_interpolated_value_for_point(
    img: &image::DynamicImage,
    point: Point,
    color_map: &HashMap<u32, RGB>,
    interpolation_map: &HashMap<u32, f64>,
) -> Option<f64> {
    let point_color = img.get_pixel(point.X, point.Y);
    let mut min_distance = None;
    let mut min_value = None;

    for (y, color) in color_map {
        let distance = ((color.R as i32 - point_color[0] as i32).pow(2)
            + (color.G as i32 - point_color[1] as i32).pow(2)
            + (color.B as i32 - point_color[2] as i32).pow(2)) as f64;

        if min_distance.is_none() || distance < min_distance.unwrap() {
            min_distance = Some(distance);
            min_value = Some(*y);
        }
    }

    min_value.and_then(|y| interpolation_map.get(&y).cloned())
}

fn create_image_with_alfa_channel(
    path_to_image: &str,
    color_map: &HashMap<u32, RGB>,
    interpolation_map: &HashMap<u32, f64>,
) -> Result<(), ImageError> {
    let img = image::open(&Path::new(path_to_image))?;
    let (width, height) = img.dimensions();

    let pixels: Vec<(u32, u32, Rgba<u8>)> = (0..height)
        .into_par_iter()
        .map_init(
            || image::open(&Path::new(path_to_image)).ok(),
            |img_opt, y| match img_opt {
                Some(img) => (0..width)
                    .into_par_iter()
                    .filter_map(move |x| {
                        match get_interpolated_value_for_point(
                            &img,
                            Point { X: x, Y: y },
                            &color_map,
                            &interpolation_map,
                        ) {
                            Some(value) => {
                                let alfa = get_alfa(value);
                                Some((x, y, Rgba([0u8, 0u8, 255u8, alfa as u8])))
                            }
                            None => {
                                eprintln!(
                                    "Не удалось найти интерполированное значение для точки ({},{}).",
                                    x, y
                                );
                                None
                            }
                        }
                    })
                    .collect::<Vec<(u32, u32, Rgba<u8>)>>(),
                None => vec![],
            },
        )
        .flatten()
        .collect();

    let mut output_img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);

    for (x, y, pixel) in pixels {
        output_img.put_pixel(x, y, pixel);
    }

    let output_path = Path::new(path_to_image).with_file_name(format!(
        "{}_alfa.png",
        Path::new(path_to_image)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
    ));

    output_img.save(output_path)?;

    Ok(())
}

fn main() -> Result<(), ImageError> {
    if !Path::new("image.png").exists() {
        eprintln!("Файла image.png не существует (нужен для калибровки). Пожалуйста, положите его в директории с исполняемым файлом.");
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut String::new()).unwrap();
        return Ok(());
    }

    let img = image::open(&Path::new("image.png"))?;
    let mut color_map = HashMap::with_capacity(472 - 7 + 1);
    let mut interpolation_map = HashMap::with_capacity(472 - 7 + 1);

    let y_values = [7, 41, 79, 120, 161, 200, 240, 280, 322, 361, 401, 440, 472];
    //let values = [90.0, 83.4, 76.7, 70.9, 65.4, 60.8, 56.6, 52.9, 50.0, 46.8, 44.3, 42.2, 40.0];
    let values = [50.0, 43.4, 36.7, 30.9, 25.4, 20.8, 16.6, 12.9, 10.0, 6.8, 4.3, 2.2, 0.0];

    for y in 7..=472 {
        let p1 = Point { X: 650, Y: y };
        let p2 = Point { X: 658, Y: y };
        match get_average_color_between_points(&img, p1, p2) {
            Ok(color) => {
                color_map.insert(y, color);
            }
            Err(e) => eprintln!("Ошибка при получении среднего цвета между точками: {}", e),
        }

        for i in 0..(y_values.len() - 1) {
            if y >= y_values[i] && y <= y_values[i + 1] {
                let value = interpolate(values[i], values[i + 1], y_values[i], y_values[i + 1], y);
                interpolation_map.insert(y, value);
                break;
            }
        }
    }
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Пожалуйста, укажите путь к изображению в качестве аргумента командной строки.");
        return Ok(());
    }

    args[1..].par_iter().for_each(|path_to_image| {
        let start = Instant::now();
        match create_image_with_alfa_channel(path_to_image, &color_map, &interpolation_map) {
            Ok(_) => println!(
                "Обработка изображения {} завершена за {} мс",
                Path::new(path_to_image)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap(),
                start.elapsed().as_millis()
            ),
            Err(err) => eprintln!(
                "Ошибка при обработке изображения {}: {:?}",
                path_to_image, err
            ),
        }
    });

    println!("Нажмите Enter для продолжения...");
    let mut buffer = String::new();
    let _ = io::stdin().read_line(&mut buffer);

    Ok(())
}
