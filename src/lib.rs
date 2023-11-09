use image::{
    codecs::{
        avif::AvifEncoder,
        jpeg::JpegEncoder,
        png::{CompressionType, FilterType, PngEncoder},
        webp::{WebPEncoder, WebPQuality},
    },
    io::Reader as ImageReader,
};
use indicatif::{ProgressBar, ProgressStyle};
use infer::get;
use libavif_image::{is_avif, read as read_avif};
use rayon::spawn;
use sevenz_rust::{Password, SevenZReader, SevenZWriter};
use std::{
    fs::{rename, File},
    io::{Cursor, Read, Write},
    sync::mpsc::{channel, Sender},
};
use tar::{Archive as TarArchive, Builder};
use zip::{ZipArchive, ZipWriter};

#[derive(Clone, Copy)]
pub enum Format {
    Jpeg,
    Png,
    Webp,
    Avif,
}

#[derive(Clone, Copy)]
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
    pub fn convert_file(self, file: &str) {
        let buf = {
            let mut buf = Vec::new();
            File::open(file).unwrap().read_to_end(&mut buf).unwrap();
            buf
        };
        if !self.quiet {
            println!("Converting {}...", file);
        }
        let data = self.convert(&buf);
        if self.backup {
            rename(file, format!("{}.bak", file)).unwrap();
        }
        File::create(file).unwrap().write_all(&data).unwrap();
    }

    pub fn convert(mut self, buf: &[u8]) -> Vec<u8> {
        self.speed = self.speed.clamp(0, 10);
        self.quality = self.quality.clamp(0, 100);
        match get(&buf).unwrap().extension() {
            "zip" => self.convert_zip(buf),
            "7z" => self.convert_7z(buf),
            "tar" => self.convert_tar(buf),
            _ => panic!("Unsupported archive format"),
        }
    }

    fn convert_zip(self, buf: &[u8]) -> Vec<u8> {
        let mut archive = ZipArchive::new(Cursor::new(buf)).unwrap();
        let mut files = Vec::new();
        for i in 0..archive.len() {
            files.push(archive.by_index(i).unwrap().name().to_owned());
        }
        let file_count = files.len();
        let mut files_data = vec![Vec::new(); file_count];
        let (tx, rx) = channel();
        for i in 0..file_count {
            let file = &files[i];
            if file.ends_with('/') {
                continue;
            }
            let mut file = archive.by_name(&file).unwrap();
            let mut file_data = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut file_data).unwrap();
            let tx = tx.clone();
            spawn(move || self.convert_image(&file_data, tx, i));
        }
        let pb = if self.quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(file_count as u64)
        };
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );
        let mut finished = 0;
        while finished < file_count {
            let (id, data) = rx.recv().unwrap();
            files_data[id] = data;
            finished += 1;
            pb.inc(1);
        }
        pb.finish();
        let mut data = Vec::new();
        let mut archive = ZipWriter::new(Cursor::new(&mut data));
        for i in 0..file_count {
            let file = &files[i];
            if file.ends_with('/') {
                archive.add_directory(file, Default::default()).unwrap();
                continue;
            }
            archive.start_file(file, Default::default()).unwrap();
            archive.write_all(&files_data[i]).unwrap();
        }
        archive.finish().unwrap();
        drop(archive);
        data
    }

    fn convert_7z(self, buf: &[u8]) -> Vec<u8> {
        let mut i = 0;
        let (tx, rx) = channel();
        let mut files = Vec::new();
        let mut archive =
            SevenZReader::new(Cursor::new(buf), buf.len() as u64, Password::empty()).unwrap();
        archive
            .for_each_entries(|entry, reader| {
                files.push(entry.clone());
                if entry.is_directory() {
                    return Ok(true);
                }
                let mut file_data = Vec::with_capacity(entry.size() as usize);
                reader.read_to_end(&mut file_data).unwrap();
                let tx = tx.clone();
                spawn(move || self.convert_image(&file_data, tx, i));
                i += 1;

                Ok(true)
            })
            .unwrap();
        let mut files_data = vec![Vec::new(); i];
        let pb = if self.quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(i as u64)
        };
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );
        let mut finished = 0;
        while finished < i {
            let (id, data) = rx.recv().unwrap();
            files_data[id] = data;
            finished += 1;
            pb.inc(1);
        }
        pb.finish();

        let mut data = Vec::new();
        let mut archive = SevenZWriter::new(Cursor::new(&mut data)).unwrap();
        for i in 0..files.len() {
            let file = &files[i];
            if file.name().ends_with('/') {
                archive
                    .push_archive_entry::<&[u8]>(file.clone(), None)
                    .unwrap();
            } else {
                archive
                    .push_archive_entry::<&[u8]>(file.clone(), Some(&files_data[i]))
                    .unwrap();
            }
        }
        archive.finish().unwrap();
        data
    }

    fn convert_tar(self, buf: &[u8]) -> Vec<u8> {
        let mut archive = TarArchive::new(buf);
        let entries = archive.entries().unwrap();
        let (tx, rx) = channel();
        let mut headers = Vec::new();
        let mut i = 0;
        for entry in entries {
            let mut entry = entry.unwrap();
            let header = entry.header().clone();
            headers.push(header);
            if entry.header().entry_type().is_dir() {
                i += 1;
                continue;
            }
            let mut file_data = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut file_data).unwrap();
            let tx = tx.clone();
            spawn(move || self.convert_image(&file_data, tx, i));
            i += 1;
        }
        let pb = if self.quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(i as u64)
        };
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );
        let mut finished = 0;
        let mut files_data = vec![Vec::new(); i];
        while finished < i {
            let (id, data) = rx.recv().unwrap();
            files_data[id] = data;
            finished += 1;
            pb.inc(1);
        }
        pb.finish();
        let mut data = Vec::new();
        let mut archive = Builder::new(&mut data);
        for i in 0..i {
            let header = &headers[i];
            if header.entry_type().is_dir() {
                archive.append(header, Cursor::new(Vec::new())).unwrap();
            } else {
                archive.append(header, &*files_data[i]).unwrap_or_else(|_| {
                    panic!(
                        "Failed to append file {}",
                        header.path().unwrap().to_str().unwrap()
                    )
                });
            }
        }

        vec![]
    }

    fn convert_image(self, buf: &[u8], tx: Sender<(usize, Vec<u8>)>, id: usize) {
        let image = if is_avif(buf) {
            read_avif(buf).unwrap()
        } else {
            ImageReader::new(Cursor::new(buf))
                .with_guessed_format()
                .unwrap()
                .decode()
                .unwrap()
        };
        let mut data = Vec::new();
        match self.format {
            Format::Avif => image
                .write_with_encoder(AvifEncoder::new_with_speed_quality(
                    &mut data,
                    self.speed,
                    self.quality,
                ))
                .unwrap(),
            Format::Webp => image
                .write_with_encoder(WebPEncoder::new_with_quality(
                    &mut data,
                    WebPQuality::lossy(self.quality),
                ))
                .unwrap(),
            Format::Png => image
                .write_with_encoder(PngEncoder::new_with_quality(
                    &mut data,
                    match self.speed.clamp(0, 2) {
                        0 => CompressionType::Fast,
                        1 => CompressionType::Default,
                        2 => CompressionType::Best,
                        _ => unreachable!(),
                    },
                    FilterType::Adaptive,
                ))
                .unwrap(),
            Format::Jpeg => image
                .write_with_encoder(JpegEncoder::new_with_quality(&mut data, self.quality))
                .unwrap(),
        }
        tx.send((id, data)).unwrap();
    }
}
