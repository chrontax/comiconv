use image::{
    codecs::{
        jpeg::JpegEncoder,
        png::{CompressionType, FilterType, PngEncoder},
        webp::{WebPEncoder, WebPQuality},
    },
    io::Reader as ImageReader,
};
use indicatif::{ProgressBar, ProgressStyle};
use infer::get;
use libavif_image::{is_avif, read as read_avif, save as save_avif};
use rayon::spawn;
use sevenz_rust::{Password, SevenZReader, SevenZWriter};
use sha2::{Digest, Sha256};
use std::{
    fs::{rename, File},
    io::{Cursor, Read, Write},
    net::TcpStream,
    sync::mpsc::{channel, Sender},
    time::Duration,
};
use tar::{Archive as TarArchive, Builder};
use zip::{ZipArchive, ZipWriter};

#[derive(Clone, Copy, Debug)]
pub enum Format {
    Jpeg,
    Png,
    Webp,
    Avif,
}

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
    pub fn convert_file(self, file: &str) {
        let buf = {
            let mut buf = Vec::new();
            File::open(file).unwrap().read_to_end(&mut buf).unwrap();
            buf
        };
        if !self.quiet {
            println!("Converting {}...", file);
        }
        let data = self.convert(&buf, None);
        if self.backup {
            rename(file, format!("{}.bak", file)).unwrap();
        }
        File::create(file).unwrap().write_all(&data).unwrap();
    }

    pub fn convert_file_online(self, file: &str, stream: &mut TcpStream) {
        let buf = {
            let mut buf = Vec::new();
            File::open(file).unwrap().read_to_end(&mut buf).unwrap();
            buf
        };
        if !self.quiet {
            println!("Converting {}...", file);
        }
        let data = self.convert_online(&buf, stream);
        if self.backup {
            rename(file, format!("{}.bak", file)).unwrap();
        }
        File::create(file).unwrap().write_all(&data).unwrap();
    }

    pub fn convert(mut self, buf: &[u8], status_stream: Option<&mut TcpStream>) -> Vec<u8> {
        self.speed = self.speed.clamp(0, 10);
        self.quality = self.quality.clamp(0, 100);
        match get(&buf).unwrap().extension() {
            "zip" => self.convert_zip(buf, status_stream),
            "7z" => self.convert_7z(buf, status_stream),
            "tar" => self.convert_tar(buf, status_stream),
            _ => panic!("Unsupported archive format"),
        }
    }

    pub fn convert_online(mut self, buf: &[u8], stream: &mut TcpStream) -> Vec<u8> {
        self.speed = self.speed.clamp(0, 10);
        self.quality = self.quality.clamp(0, 100);
        stream.set_nodelay(true).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        stream.write_all(b"comi").unwrap();
        {
            let mut buf = [0; 4];
            stream.read_exact(&mut buf).unwrap();
            if &buf != b"conv" {
                panic!("Invalid server response");
            }
        }
        let format = match self.format {
            Format::Avif => b'A',
            Format::Webp => b'W',
            Format::Png => b'P',
            Format::Jpeg => b'J',
        };
        let mut left = buf.len();
        {
            let mut buf = [0; 8];
            buf[0] = format;
            buf[1] = self.speed;
            buf[2] = self.quality;
            buf[4..].copy_from_slice(&(left as u32).to_be_bytes());
            stream.write_all(&buf).unwrap();
        }
        let hash = {
            let mut hasher = Sha256::new();
            hasher.update(buf);
            hasher.finalize()
        };
        stream.write_all(&hash).unwrap();
        let mut sent = 0;
        let pb = if self.quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(left as u64)
        };
        pb.set_style(
            ProgressStyle::default_bar()
                .template("Upload   [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );
        while left > 0 {
            let size = left.min(1024 * 1024);
            stream.write_all(&buf[sent..sent + size]).unwrap();
            let mut buf = [0; 2];
            stream.read_exact(&mut buf).unwrap();
            if &buf != b"ok" {
                panic!("Invalid server response");
            }
            sent += size;
            left = left.saturating_sub(size);
            pb.inc(size as u64);
        }
        pb.finish();
        let mut left = {
            let mut buf = [0; 4];
            stream.read_exact(&mut buf).unwrap();
            u32::from_be_bytes(buf)
        };
        let pb = if self.quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(left as u64)
        };
        pb.set_style(
            ProgressStyle::default_bar()
                .template("Convert  [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );
        while left > 0 {
            let response = {
                let mut buf = [0; 4];
                stream.read_exact(&mut buf).unwrap();
                buf
            };
            if &response != b"plus" {
                panic!("Invalid server response");
            }
            pb.inc(1);
            left -= 1;
        }
        pb.finish();
        let mut left = {
            let mut buf = [0; 4];
            stream.read_exact(&mut buf).unwrap();
            u32::from_be_bytes(buf)
        };
        let hash = {
            let mut buf = [0; 32];
            stream.read_exact(&mut buf).unwrap();
            buf
        };
        let pb = if self.quiet {
            ProgressBar::hidden()
        } else {
            ProgressBar::new(left as u64)
        };
        pb.set_style(
            ProgressStyle::default_bar()
                .template("Download [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );
        let mut data = Vec::new();
        while left > 0 {
            let mut buf = [0; 1024 * 1024];
            let read = stream.read(&mut buf).unwrap();
            pb.inc(read as u64);
            data.extend_from_slice(&buf[..read]);
            left = left.saturating_sub(read as u32);
        }
        pb.finish();
        let mut hasher = Sha256::new();
        hasher.update(&data);
        if hasher.finalize() != hash.into() {
            panic!("Invalid hash");
        }
        data
    }

    fn convert_zip(self, buf: &[u8], mut status_stream: Option<&mut TcpStream>) -> Vec<u8> {
        let mut archive = ZipArchive::new(Cursor::new(buf)).unwrap();
        let mut files = Vec::new();
        for i in 0..archive.len() {
            files.push(archive.by_index(i).unwrap().name().to_owned());
        }
        let total_count = files.len();
        let file_count = files.iter().filter(|f| !f.ends_with('/')).count();
        let mut files_data = vec![Vec::new(); total_count];
        let (tx, rx) = channel();
        for i in 0..total_count {
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
        if let Some(ref mut stream) = status_stream {
            stream
                .write_all((file_count as u32).to_be_bytes().as_ref())
                .unwrap();
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
            if let Some(ref mut stream) = status_stream {
                stream.write_all(b"plus").unwrap();
            }
        }
        pb.finish();
        let mut data = Vec::new();
        let mut archive = ZipWriter::new(Cursor::new(&mut data));
        for i in 0..total_count {
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

    fn convert_7z(self, buf: &[u8], mut status_stream: Option<&mut TcpStream>) -> Vec<u8> {
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
        if let Some(ref mut stream) = status_stream {
            stream.write_all((i as u32).to_be_bytes().as_ref()).unwrap();
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
        while finished < i {
            let (id, data) = rx.recv().unwrap();
            files_data[id] = data;
            finished += 1;
            pb.inc(1);
            if let Some(ref mut stream) = status_stream {
                stream.write_all(b"plus").unwrap();
            }
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

    fn convert_tar(self, buf: &[u8], mut status_stream: Option<&mut TcpStream>) -> Vec<u8> {
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
                continue;
            }
            let mut file_data = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut file_data).unwrap();
            let tx = tx.clone();
            spawn(move || self.convert_image(&file_data, tx, i));
            i += 1;
        }
        if let Some(ref mut stream) = status_stream {
            stream.write_all((i as u32).to_be_bytes().as_ref()).unwrap();
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
            if let Some(ref mut stream) = status_stream {
                stream.write_all(b"plus").unwrap();
            }
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
            Format::Avif => {
                data = save_avif(&image).unwrap().to_vec();
            },
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
