//! Library for comic book conversion.
//!
//! You can convert locally or on a server running comiconv-server.

use cra::{ArcEntry, ArcError, ArcReader, ArcWriter};
use image::{
    codecs::{
        jpeg::JpegEncoder,
        png::{CompressionType, FilterType, PngEncoder},
        webp::WebPEncoder,
    },
    ColorType, DynamicImage, ImageError, ImageReader,
};
use indicatif::{style::TemplateError, ProgressBar, ProgressStyle};
use infer::image::is_jxl;
use jxl_oxide::integration::JxlDecoder;
use libavif_image::{is_avif, read as read_avif, save as save_avif, Error as AvifError};
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use sha2::{Digest, Sha256};
use std::{
    fs::{rename, File},
    io::{self, Cursor, Read, Write},
    net::TcpStream,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};
use thiserror::Error;
use zune_core::{bit_depth::BitDepth, colorspace::ColorSpace, options::EncoderOptions};
use zune_jpegxl::{JxlEncodeErrors, JxlSimpleEncoder};

/// This is the main error type for the library
#[derive(Error, Debug)]
#[error(transparent)]
pub enum ConvError {
    ArcError(#[from] ArcError),
    IoError(#[from] io::Error),
    TemplateError(#[from] TemplateError),
    AvifError(#[from] AvifError),
    ImageError(#[from] ImageError),
    #[error("{0:?}")]
    JxlEncodeError(JxlEncodeErrors),
    #[error("Invalid server response")]
    InvalidResponse,
    #[error("Hash mismatch")]
    HashMismatch,
}

pub type ConvResult<T> = Result<T, ConvError>;

/// Enum representing all supported target image formats
#[derive(Clone, Copy, Debug)]
pub enum Format {
    Jpeg,
    JpegXL,
    Png,
    Webp,
    Avif,
}

impl ToString for Format {
    fn to_string(&self) -> String {
        String::from(match self {
            Format::Jpeg => "jpg",
            Format::JpegXL => "jxl",
            Format::Png => "png",
            Format::Webp => "webp",
            Format::Avif => "avif",
        })
    }
}

impl FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "avif" => Ok(Format::Avif),
            "jpeg" | "jpg" => Ok(Format::Jpeg),
            "jxl" => Ok(Format::JpegXL),
            "webp" => Ok(Format::Webp),
            "png" => Ok(Format::Png),
            _ => Err(format!("Invalid format: {s}")),
        }
    }
}

/// This is the main struct for converting
/// `quality` is ignored for webp
#[derive(Clone, Copy, Debug)]
pub struct Converter {
    pub quality: u8,
    pub speed: u8,
    pub format: Format,
    pub backup: bool,
    pub quiet: bool,
}

impl Default for Converter {
    fn default() -> Self {
        Self {
            quality: 30,
            speed: 3,
            format: Format::Avif,
            backup: false,
            quiet: false,
        }
    }
}

impl Converter {
    /// Takes a path to a file and converts it
    pub fn convert_file(self, file: &str) -> ConvResult<()> {
        let buf = {
            let mut buf = Vec::new();
            File::open(file)?.read_to_end(&mut buf)?;
            buf
        };
        if !self.quiet {
            println!("Converting {}...", file);
        }
        let data = self.convert(&buf, None)?;
        if self.backup {
            rename(file, format!("{}.bak", file))?;
        }
        File::create(file)?.write_all(&data)?;
        Ok(())
    }

    /// Takes a path to a file, a tcp connection and converts the file using a server
    pub fn convert_file_online(self, file: &str, stream: &mut TcpStream) -> ConvResult<()> {
        let buf = {
            let mut buf = Vec::new();
            File::open(file)?.read_to_end(&mut buf)?;
            buf
        };
        if !self.quiet {
            println!("Converting {}...", file);
        }
        let data = self.convert_online(&buf, stream)?;
        if self.backup {
            rename(file, format!("{}.bak", file))?;
        }
        File::create(file)?.write_all(&data)?;
        Ok(())
    }

    /// Takes contents of a file as a slice of bytes and return the new converted file as bytes.
    /// Optionally takes a stream to write progress information for the client (only really used by the server).
    pub fn convert(
        mut self,
        buf: &[u8],
        mut status_stream: Option<&mut TcpStream>,
    ) -> ConvResult<Vec<u8>> {
        let format_extension = self.format.to_string();
        self.speed = self.speed.clamp(0, 10);
        self.quality = self.quality.clamp(0, 100);
        let mut archive = ArcReader::new(buf)?;
        let mut writer = ArcWriter::new(archive.format());
        let file_count = archive
            .by_ref()
            .filter(|entry| !matches!(entry, ArcEntry::Directory(_)))
            .count();
        if let Some(ref mut stream) = status_stream {
            stream.write_all(&(file_count as u32).to_be_bytes())?;
        }
        let mut bar = if self.quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(file_count as u64)
        };
        bar.set_style(
            ProgressStyle::default_bar()
                .template("Convert  [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars("=>-"),
        );
        let status_stream = status_stream.map(|stream| Arc::new(Mutex::new(stream)));
        let pb = Arc::new(Mutex::new(&mut bar));
        writer.extend(
            &archive
                .entries()
                .clone()
                .into_par_iter()
                .map(|entry| {
                    Ok(match entry {
                        ArcEntry::File(name, data) => {
                            let data = self.convert_image(&data)?;
                            if let Some(stream) = status_stream.clone() {
                                stream.lock().unwrap().write_all(b"plus")?
                            }
                            pb.clone().lock().unwrap().inc(1);
                            ArcEntry::File(
                                format!(
                                    "{}.{}",
                                    name.rsplit_once('.').unwrap_or((&name, "")).0,
                                    &format_extension
                                ),
                                data,
                            )
                        }
                        other => other,
                    })
                })
                .collect::<ConvResult<Vec<ArcEntry>>>()?,
        );
        bar.finish();
        Ok(writer.archive()?)
    }

    /// Takes file contents as a slice and a tcp connection to the server
    pub fn convert_online(mut self, buf: &[u8], stream: &mut TcpStream) -> ConvResult<Vec<u8>> {
        self.speed = self.speed.clamp(0, 10);
        self.quality = self.quality.clamp(0, 100);
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.write_all(b"comi")?;
        {
            let mut buf = [0; 4];
            stream.read_exact(&mut buf)?;
            if &buf != b"conv" {
                return Err(ConvError::InvalidResponse);
            }
        }
        let format = match self.format {
            Format::Avif => b'A',
            Format::Webp => b'W',
            Format::Png => b'P',
            Format::Jpeg => b'J',
            Format::JpegXL => todo!(),
        };
        let mut left = buf.len();
        {
            let mut buf = [0; 8];
            buf[0] = format;
            buf[1] = self.speed;
            buf[2] = self.quality;
            buf[4..].copy_from_slice(&(left as u32).to_be_bytes());
            stream.write_all(&buf)?;
        }
        let hash = {
            let mut hasher = Sha256::new();
            hasher.update(buf);
            hasher.finalize()
        };
        stream.write_all(&hash)?;
        let mut sent = 0;
        let pb = if self.quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(left as u64)
        };
        pb.set_style(
            ProgressStyle::default_bar()
                .template("Upload   [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
                .progress_chars("=>-"),
        );
        while left > 0 {
            let size = left.min(1024 * 1024);
            stream.write_all(&buf[sent..sent + size])?;
            let mut buf = [0; 2];
            stream.read_exact(&mut buf)?;
            if &buf != b"ok" {
                return Err(ConvError::InvalidResponse);
            }
            sent += size;
            left = left.saturating_sub(size);
            pb.inc(size as u64);
        }
        pb.finish();
        let mut left = {
            let mut buf = [0; 4];
            stream.read_exact(&mut buf)?;
            u32::from_be_bytes(buf)
        };
        let pb = if self.quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(left as u64)
        };
        pb.set_style(
            ProgressStyle::default_bar()
                .template("Convert  [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars("=>-"),
        );
        while left > 0 {
            let response = {
                let mut buf = [0; 4];
                stream.read_exact(&mut buf)?;
                buf
            };
            if &response != b"plus" {
                return Err(ConvError::InvalidResponse);
            }
            pb.inc(1);
            left -= 1;
        }
        pb.finish();
        let mut left = {
            let mut buf = [0; 4];
            stream.read_exact(&mut buf)?;
            u32::from_be_bytes(buf)
        };
        let hash = {
            let mut buf = [0; 32];
            stream.read_exact(&mut buf)?;
            buf
        };
        let pb = if self.quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(left as u64)
        };
        pb.set_style(
            ProgressStyle::default_bar()
                .template("Download [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
                .progress_chars("=>-"),
        );
        let mut data = Vec::new();
        while left > 0 {
            let mut buf = [0; 1024 * 1024];
            let read = stream.read(&mut buf)?;
            pb.inc(read as u64);
            data.extend_from_slice(&buf[..read]);
            left = left.saturating_sub(read as u32);
        }
        pb.finish();
        let mut hasher = Sha256::new();
        hasher.update(&data);
        if hasher.finalize() != hash.into() {
            return Err(ConvError::HashMismatch);
        }
        Ok(data)
    }

    fn convert_image(self, buf: &[u8]) -> ConvResult<Vec<u8>> {
        let image = if is_avif(buf) {
            read_avif(buf)?
        } else if is_jxl(buf) {
            DynamicImage::from_decoder(JxlDecoder::new(buf)?)?
        } else {
            ImageReader::new(Cursor::new(buf))
                .with_guessed_format()?
                .decode()?
        };
        let mut data = Vec::new();
        match self.format {
            Format::Avif => {
                data = save_avif(&image)?.to_vec();
            }
            Format::Webp => image.write_with_encoder(WebPEncoder::new_lossless(&mut data))?,
            Format::Png => image.write_with_encoder(PngEncoder::new_with_quality(
                &mut data,
                match self.speed.clamp(0, 2) {
                    0 => CompressionType::Fast,
                    1 => CompressionType::Default,
                    2 => CompressionType::Best,
                    _ => unreachable!(),
                },
                FilterType::Adaptive,
            ))?,
            Format::Jpeg => {
                image.write_with_encoder(JpegEncoder::new_with_quality(&mut data, self.quality))?
            }
            Format::JpegXL => {
                let (color, depth) = image_to_zune_colot_type(&image);
                data = JxlSimpleEncoder::new(
                    image.as_bytes(),
                    EncoderOptions::new(image.width() as _, image.height() as _, color, depth),
                )
                .encode()
                .map_err(ConvError::JxlEncodeError)?;
            }
        }
        Ok(data)
    }
}

fn image_to_zune_colot_type(image: &DynamicImage) -> (ColorSpace, BitDepth) {
    match image.color() {
        ColorType::L8 => (ColorSpace::Luma, BitDepth::Eight),
        ColorType::La16 => (ColorSpace::LumaA, BitDepth::Sixteen),
        ColorType::Rgb16 => (ColorSpace::RGB, BitDepth::Sixteen),
        ColorType::Rgba16 => (ColorSpace::RGBA, BitDepth::Sixteen),
        ColorType::Rgb32F => (ColorSpace::RGB, BitDepth::Float32),
        ColorType::Rgba32F => (ColorSpace::RGBA, BitDepth::Float32),
        ColorType::Rgba8 => (ColorSpace::RGBA, BitDepth::Eight),
        ColorType::L16 => (ColorSpace::Luma, BitDepth::Sixteen),
        ColorType::La8 => (ColorSpace::LumaA, BitDepth::Eight),
        ColorType::Rgb8 => (ColorSpace::RGB, BitDepth::Eight),
        _ => unimplemented!(),
    }
}
