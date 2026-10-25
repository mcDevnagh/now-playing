use std::io::{BufRead, BufReader, Cursor, Write};
use std::net::TcpListener;
use std::thread;

use image::{DynamicImage, ImageFormat, RgbImage};

use super::*;

/// An image encoded as `format`: red on the left half, blue on the right.
fn encoded(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
    let image = RgbImage::from_fn(width, height, |x, _| {
        if x < width / 2 {
            image::Rgb([255, 0, 0])
        } else {
            image::Rgb([0, 0, 255])
        }
    });
    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(image)
        .write_to(&mut Cursor::new(&mut bytes), format)
        .unwrap();
    bytes
}

fn pixel(image: &Image, x: u32, y: u32) -> [u8; 4] {
    let i = (y * image.width + x) as usize * 4;
    image.rgba[i..i + 4].try_into().unwrap()
}

#[test]
fn decodes_jpeg_and_png() {
    for format in [ImageFormat::Jpeg, ImageFormat::Png] {
        let cover = decode("u", &encoded(100, 100, format), 1024).unwrap();
        assert_eq!(
            (cover.image.width, cover.image.height),
            (100, 100),
            "{format:?}"
        );
        assert_eq!(
            (cover.blur.width, cover.blur.height),
            (64, 64),
            "{format:?}"
        );
        assert_eq!(cover.url, "u");
    }
}

#[test]
fn crops_to_a_centred_square() {
    // 200×100: the middle 100×100 is half red, half blue.
    let cover = decode("u", &encoded(200, 100, ImageFormat::Png), 1024).unwrap();
    assert_eq!((cover.image.width, cover.image.height), (100, 100));
    assert_eq!(pixel(&cover.image, 25, 50), [255, 0, 0, 255]);
    assert_eq!(pixel(&cover.image, 75, 50), [0, 0, 255, 255]);
}

#[test]
fn scales_large_covers_down() {
    let cover = decode("u", &encoded(3000, 3000, ImageFormat::Jpeg), 1339).unwrap();
    assert_eq!((cover.image.width, cover.image.height), (1339, 1339));
}

#[test]
fn never_scales_covers_up() {
    let cover = decode("u", &encoded(620, 620, ImageFormat::Png), 1339).unwrap();
    assert_eq!((cover.image.width, cover.image.height), (620, 620));
}

#[test]
fn rounds_the_corners() {
    let cover = decode("u", &encoded(620, 620, ImageFormat::Png), 1024).unwrap();
    // Radius 15 px on a 620 px side.
    assert_eq!(pixel(&cover.image, 0, 0)[3], 0);
    assert_eq!(pixel(&cover.image, 619, 619)[3], 0);
    assert_eq!(pixel(&cover.image, 15, 0)[3], 255);
    assert_eq!(pixel(&cover.image, 310, 310)[3], 255);
}

#[test]
fn rejects_what_it_cannot_decode() {
    assert!(decode("u", b"not an image", 1024).is_err());
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#;
    assert!(decode("u", svg, 1024).is_err());
}

#[test]
fn replaces_the_host() {
    let cases = [
        (
            "http://server:1780/__image_cache?name=a.jpg",
            "http://192.0.2.1:1780/__image_cache?name=a.jpg",
        ),
        ("https://art.example/a.jpg", "https://192.0.2.1/a.jpg"),
        ("http://art.example", "http://192.0.2.1"),
        ("http://[::1]/a.jpg", "http://192.0.2.1/a.jpg"),
        ("http://[::1]:8080/a.jpg", "http://192.0.2.1:8080/a.jpg"),
    ];
    for (url, expected) in cases {
        assert_eq!(with_host(url, "192.0.2.1").as_deref(), Some(expected));
    }
    assert_eq!(with_host("not a url", "192.0.2.1"), None);
}

/// Serves `body` once on a local port, returning the port.
fn serve_once(body: Vec<u8>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap() > 2 {
            line.clear();
        }
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
    });
    port
}

#[test]
fn fetches_and_falls_back_to_the_server_host() {
    let png = encoded(32, 32, ImageFormat::Png);
    let fetcher = Fetcher::new("127.0.0.1");

    let port = serve_once(png.clone());
    let direct = fetcher
        .fetch(&format!("http://127.0.0.1:{port}/a.png"))
        .unwrap();
    assert_eq!(direct, png);

    // `.invalid` never resolves, so the server's host is used instead.
    let port = serve_once(png.clone());
    let fallback = fetcher
        .fetch(&format!("http://cover.invalid:{port}/a.png"))
        .unwrap();
    assert_eq!(fallback, png);
}
