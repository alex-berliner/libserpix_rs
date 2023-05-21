use image::GenericImageView;
use cbor::Decoder;
use image::{imageops::crop_imm, ImageBuffer, Rgba};
use rustc_serialize::json::ToJson;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use std::time::Duration;
use tokio::task;
use tokio::sync::mpsc::Sender;
use crate::*;

static CAPTURE_MAX_W: u32 = 900;
static CAPTURE_MAX_H: u32 = 8;

struct Frame {
    size: u16,
    pixels: Vec<u8>,
    img: ImageBuffer<Rgba<u8>, Vec<u8>>,
}

impl Frame {
    #[allow(dead_code)]
    fn save(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) {
        let posix_time =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let mut file_name = posix_time.to_string();
        file_name.push_str(".bmp");
        img.save(&file_name).unwrap();
        eprintln!("Save {}", file_name);
    }

    fn new(img: ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<Frame, &'static str> {
        let payload_pixels = match Self::get_payload_pixels(&img) {
            Err(e) => { return Err(e); } ,
            Ok(v) => v
        };
        let header = color_to_integer(&payload_pixels[1]);
        let (size, checksum_rx) = decode_header(header);

        let pixels = match Self::get_payload(payload_pixels[2..payload_pixels.len()].into(), size) {
            Err(e) => { return Err(e); },
            Ok(v) => { v}
        };
        let checksum_calc = pixels.iter().fold(0, |acc, x| (acc + *x as u32)%256) as u8;
        if checksum_rx != checksum_calc {
            eprintln!("Checksum mismatch {} {}", checksum_rx, checksum_calc);
            Frame::save(&img);
            return Err("Checksum mismatch");
        }
        Ok(Frame {
            size: size,
            pixels: pixels,
            img: img
        })
    }

    fn get_payload_pixels(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<Vec<Rgba<u8>>, &'static str> {
        let loc = match find_anchor(&img, [42, 0, 69]) {
            None => {
                println!("No key start");
                Frame::save(&img);
                return Err("MAKE ERROR HERE");
            },
            Some(v) => v,
        };
        let img = crop_imm(img, loc.0, loc.1, img.width()-loc.0, img.height()-loc.1).to_image();
        // get x coords of colums that start and end with [42,0,69]
        let header_pixel = Rgba([42, 0, 69, 255]);
        let pixels_vec_tup: Vec<(u32, Vec<Rgba<u8>>)> = (0..img.width()-1)
            .filter_map(|x|
                if  img.get_pixel(x, 0) == &header_pixel &&
                    img.get_pixel(x, CAPTURE_MAX_H+1) == &header_pixel {
                    let column: Vec<_> = (1..CAPTURE_MAX_H).filter_map(|y|
                        Some(*img.get_pixel(x, y))
                    ).collect();
                    Some((x, column))
                } else {
                    None
                }
            ).collect();
        let pixels_vec: Vec<_> = (0..pixels_vec_tup.len()).filter_map(|i| {
            if (i == pixels_vec_tup.len() - 1) ||
                // sometimes wow draws a 1-pixel column as a 2-pixel column,
                // so throw that out here
                pixels_vec_tup[i].0+1 != pixels_vec_tup[i+1].0 {
                // ok to convert this to copy if it causes problems later
                Some(&pixels_vec_tup[i].1)
            } else {
                None
            }
        }).collect();
        let rx_header_pixel = match column_to_pixel(&pixels_vec[0]){
             Err(e) => { return Err("Invalid header_pixel column"); }
             Ok(v) => v,
        };
        if rx_header_pixel != header_pixel {
            Frame::save(&img);
            return Err("rx_header_pixel was not 42069!");
        }
        let pixels = (0..pixels_vec.len()).map(|x| {
            column_to_pixel(&pixels_vec[x])
        } ).collect::<Result<Vec<_>, _>>();
        pixels
    }

    fn get_payload(myvec: Vec<Rgba<u8>>, size: u16) -> Result<Vec<u8>, &'static str> {
        let mut bytevec: Vec<u8> = Vec::new();
        for p in myvec.iter() {
            bytevec.push(p[0]);
            bytevec.push(p[1]);
            bytevec.push(p[2]);
        }
        // println!("{} {}", bytevec.len(), size);
        while bytevec.len() != size as usize {
            bytevec.pop();
        }
        Ok(bytevec)
    }
}

fn color_to_integer(pixel: &Rgba<u8>) -> u32 {
    let r = pixel[0] as u32;
    let g = pixel[1] as u32;
    let b = pixel[2] as u32;
    r * 256 * 256 + g * 256 + b
}

fn decode_header(header: u32) -> (u16, u8) {
    let checksum = (header & 0xFF) as u8;
    let size = ((header >> 8) & 0xFFFF) as u16;

    (size, checksum)
}

fn get_screen(hwnd: isize) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let buf = win_screenshot::capture::capture_window(hwnd, win_screenshot::capture::Area::Full).unwrap();
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(buf.width, buf.height, buf.pixels).unwrap();
    img
}

fn cbor_parse(b: &Vec<u8>) -> Result<serde_json::Value, &'static str> {
    let mut d = Decoder::from_bytes(b.clone());
    let cbor_in = match d.items().next() {
        Some(o) => o,
        None => {
            return Err("cbor fail 1");
        }
    };
    let cbor = match cbor_in {
        Ok(o) => o,
        Err(e) => {
            eprintln!("cbor fail: {}", e);
            return Err("cbor fail 2");
        }
    };
    let c2j = cbor.to_json();
    Ok(serde_json::from_str(&c2j.to_string()).unwrap())
}

/*
process an ImageBuffer s containing a message and send it over tx as JSON
*/
pub async fn screen_proc(s: ImageBuffer<Rgba<u8>, Vec<u8>>, tx: Sender<serde_json::Value>) {
    let x = s.clone();
    let frame = match Frame::new(s) {
        Ok(v) => v,
        Err(e) => { eprintln!("frame decode error {}", e); Frame::save(&x); return; },
    };

    let mut value: serde_json::Value = match cbor_parse(&frame.pixels) {
        Ok(v) => v,
        Err(e) => {
            Frame::save(&frame.img);
            eprintln!("{}", e);
            return;
        }
    };

    if value.is_object() {
        match value.as_object_mut() {
            None => { eprintln!("no private field, very fishy"); },
            Some(v) => {v.remove_entry("p");},
        };

        tx.send(value).await.expect("json send failed");
    }
}

/*
Capture a message from a screenshot in the window handle hwnd and send it over tx as JSON
*/
pub async fn read_wow(hwnd: isize, tx: Sender<serde_json::Value>) {
    match hwnd_exists(hwnd) {
        WindowStatus::Destroyed => return,
        WindowStatus::Minimized => {
            thread::sleep(Duration::from_millis(1));
            return;
        },
        _ => {},
    }

    let s = get_screen(hwnd);
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        screen_proc(s, tx_clone).await;
    });
}

/*
Find the (x, y) coord of the anchor, which is the first column of the message
*/
fn find_anchor(buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>, color: [u8; 3]) -> Option<(u32, u32)> {
    let (width, height) = buffer.dimensions();

    for y in 0..height {
        for x in 0..width {
            let pixel = buffer.get_pixel(x, y);
            if pixel[0] == color[0] && pixel[1] == color[1] && pixel[2] == color[2] {
                return Some((x, y));
            }
        }
    }

    None
}

/*
Determine the color value of a column of pixels
Each pixel in a message is drawn as a column instead of a 1x1 pixel to account
for WoW UI wonkiness so this collapses the column into a single value
*/
fn column_to_pixel(pixels: &Vec<Rgba<u8>>) -> Result<Rgba<u8>, &'static str> {
    if pixels.len() < 1 {
        return Err("pixels array empty");
    }
    // println!("{:?}", pixels);
    let pixel_counts =
        pixels.iter().fold(std::collections::HashMap::new(), |mut acc, &x| {
            *acc.entry(x).or_insert(0) += 1;
            acc
        });

    let (most_common_pixel, most_common_count) = pixel_counts.iter()
        .max_by_key(|&(_, count)| count)
        .map(|(pixel, count)| (*pixel, *count))
        .ok_or("no max?")?;

    if most_common_count >= 3 {
        Ok(most_common_pixel)
    } else {
        Err("Not at least 3 pixels are the same")
    }
}


mod tests {
    use super::*;
    use image::GenericImageView;

    #[tokio::test]
    async fn find_key_start_test() {
        let img = image::open("assets/windowed_valid_header.bmp").unwrap().into_rgba8();
        assert_eq!(Some((9,38)), find_anchor(&img, [42,0,69]));
    }

    #[tokio::test]
    async fn test_frame_from_imgbuf2_success() {
        let img = image::open("assets/windowed_valid_header.bmp").unwrap().into_rgba8();
        let loc = match find_anchor(&img, [42, 0, 69]) {
            None => { println!("No key start"); return; },
            Some(v) => v,
        };
        let img = img.view(loc.0, loc.1, img.width()-loc.0, img.height()-loc.1).to_image();
        // get x coords of colums that start and end with [42,0,69]
        let header = Rgba([42, 0, 69, 255]);
        let pixels_vec: Vec<_> = (0..img.width())
            .filter_map(|x|
                if  img.get_pixel(x, 0) == &header &&
                    img.get_pixel(x, CAPTURE_MAX_H+1) == &header {
                    // println!("{:?}", img.get_pixel(x, 1));
                    let column: Vec<_> = (1..CAPTURE_MAX_H).map(|y| *img.get_pixel(x, y)).collect();
                    Some(column)
                } else {
                    None
                }
            ).collect();

        let pixels = (0..pixels_vec.len()).map(|x|
            column_to_pixel(&pixels_vec[x])
        ).collect::<Result<Vec<_>, _>>();

        let pixels = match pixels {
            Ok(v) => {
                println!("{:?}", v);
                v
            }
            Err(e) => {
                println!("Error: {}", e);
                return;
            }
        };
        assert_eq!(7, pixels.len());
        println!("{} {:?}", pixels.len(), pixels);
    }

    #[tokio::test]
    async fn test_frame_from_imgbuf2_failure() {
        let img = image::open("assets/windowed_invalid_header.bmp").unwrap().into_rgba8();
        let loc = match find_anchor(&img, [42, 0, 69]) {
            None => { println!("No key start"); return; },
            Some(v) => v,
        };
        let img = img.view(loc.0, loc.1, img.width()-loc.0, img.height()-loc.1).to_image();
        // get x coords of colums that start and end with [42,0,69]
        let header = Rgba([42, 0, 69, 255]);
        let pixels_vec: Vec<_> = (0..img.width())
            .filter_map(|x|
                if  img.get_pixel(x, 0) == &header &&
                    img.get_pixel(x, CAPTURE_MAX_H+1) == &header {
                    // println!("{:?}", img.get_pixel(x, 1));
                    let column: Vec<_> = (1..CAPTURE_MAX_H).map(|y| *img.get_pixel(x, y)).collect();
                    Some(column)
                } else {
                    None
                }
            ).collect();

        let pixels = (0..pixels_vec.len()).map(|x|
            column_to_pixel(&pixels_vec[x])
        ).collect::<Result<Vec<_>, _>>();

        let pixels = match pixels {
            Ok(v) => {
                println!("{:?}", v);
                v
            }
            Err(e) => {
                println!("Error: {}", e);
                return;
            }
        };
        assert_eq!(7, pixels.len());
        println!("{} {:?}", pixels.len(), pixels);
    }

    #[tokio::test]
    async fn test_create_frame_success() {
        let img = image::open("assets/windowed_valid_header.bmp").unwrap().into_rgba8();
        let f = Frame::new(img).unwrap();
    }
}

