use std::fs;
use std::path::Path;
use image::{
    ImageEncoder,
    io::Reader as ImageReader,
    ColorType,
    codecs::{
        avif::AvifEncoder,
        jpeg::JpegEncoder,
        png::{
            PngEncoder,
            CompressionType,
            FilterType
        },
        webp::{
            WebPEncoder,
            WebPQuality
        }
    }
};

pub fn avif(path: &Path, speed: u8, quality: u8) {
    let img = ImageReader::open(path)
        .expect("Failed to open image")
        .decode()
        .expect("Failed to decode image")
        .to_rgb8();

    let mut buf = Vec::new();
    AvifEncoder::new_with_speed_quality(&mut buf, speed, quality)
        .write_image(
            &img,
            img.width(),
            img.height(),
            ColorType::Rgb8)
        .expect("Failed to encode image");

    let outpath = format!(
        "tmp/{}.avif",
        path.file_stem().unwrap().to_str().unwrap()
    );
    fs::write(outpath, buf).expect("Failed to write image");
}

pub fn jpeg(path: &Path, quality: u8) {
    let img = ImageReader::open(path)
        .expect("Failed to open image")
        .decode()
        .expect("Failed to decode image")
        .to_rgb8();

    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, quality)
        .encode(
            &img,
            img.width(),
            img.height(),
            ColorType::Rgb8)
        .expect("Failed to encode image");

    let outpath = format!(
        "tmp/{}.jpg",
        path.file_stem().unwrap().to_str().unwrap()
    );
    fs::write(outpath, buf).expect("Failed to write image");
}

pub fn png(path: &Path, compression: CompressionType) {
    let img = ImageReader::open(path)
        .expect("Failed to open image")
        .decode()
        .expect("Failed to decode image")
        .to_rgb8();

    let mut buf = Vec::new();
    PngEncoder::new_with_quality(&mut buf, compression, FilterType::Adaptive)
        .write_image(
            &img,
            img.width(),
            img.height(),
            ColorType::Rgb8)
        .expect("Failed to encode image");

    let outpath = format!(
        "tmp/{}.png",
        path.file_stem().unwrap().to_str().unwrap()
    );
    fs::write(outpath, buf).expect("Failed to write image");
}

pub fn webp(path: &Path, quality: WebPQuality) {
    let img = ImageReader::open(path)
        .expect("Failed to open image")
        .decode()
        .expect("Failed to decode image")
        .to_rgb8();

    let mut buf = Vec::new();
    WebPEncoder::new_with_quality(&mut buf, quality)
        .write_image(
            &img,
            img.width(),
            img.height(),
            ColorType::Rgb8)
        .expect("Failed to encode image");

    let outpath = format!(
        "tmp/{}.webp",
        path.file_stem().unwrap().to_str().unwrap()
    );
    fs::write(outpath, buf).expect("Failed to write image");
}