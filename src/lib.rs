mod cbz;
mod enc;
use image::codecs::{png::CompressionType, webp::WebPQuality};
use std::{
    fs,
    path::PathBuf,
    sync::mpsc::{channel, Sender, Receiver},
    thread::JoinHandle,
};
use num_cpus;
use tar;
use sevenz_rust;
use rar;
use indicatif::ProgressBar;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy)]
pub struct Converter {
    pub format: Format,
    pub quality: u8,
    pub speed: u8,
    pub threads: u8,
    pub archive: Archive,
}

#[derive(Debug, Clone, Copy)]
pub enum Format {
    Jpeg,
    Png,
    Webp,
    Avif,
}

#[derive(Debug, Clone, Copy)]
pub enum Archive {
    CBZ,
    CBT,
    CB7,
    CBR,
    Unset,
}

impl Converter {
    pub fn new(format: Format, quality: u8, speed: u8, threads: u8) -> Self {
        Self {
            format,
            quality,
            speed,
            threads,
            archive: Archive::Unset,
        }
    }

    pub fn new_with_archive(
        format: Format,
        quality: u8,
        speed: u8,
        threads: u8,
        archive: Archive,
    ) -> Self {
        Self {
            format,
            quality,
            speed,
            threads,
            archive,
        }
    }

    pub fn convert(&self, file: &str) -> Result<(), &str> {
        fs::create_dir("tmp").expect("Failed to create tmp directory");
        let mut archive_type = self.archive;

        match self.archive {
            Archive::Unset => match &file[file.len() - 3..] {
                "cbz" | "zip" => {
                    archive_type = Archive::CBZ;
                    cbz::extract("tmp/", file)
                },
                "cbt" | "tar" => {
                    archive_type = Archive::CBT;
                    let mut archive = tar::Archive::new(fs::File::open(file).expect("Failed to open file"));
                    archive.unpack("tmp/").expect("Failed to unpack archive");
                },
                "cb7" | ".7z" => {
                    archive_type = Archive::CB7;
                    sevenz_rust::decompress_file(file, "tmp").unwrap();
                },
                "cbr" | "rar" => {
                    archive_type = Archive::CBR;
                    rar::Archive::extract_all(file, "tmp", "").unwrap();
                    ()
                },
                _ => {
                    return Err("Fiel not recognized")
                }
            },
            Archive::CBZ => cbz::extract("tmp/", file),
            Archive::CBT => {
                let mut archive = tar::Archive::new(fs::File::open(file).expect("Failed to open file"));
                archive.unpack("tmp/").expect("Failed to unpack archive");
            },
            Archive::CB7 => sevenz_rust::decompress_file(file, "tmp").unwrap(),
            Archive::CBR => {
                rar::Archive::extract_all(file, "tmp", "").unwrap();
                ()
            },
        }

        let mut threads: Vec<(JoinHandle<()>, Sender<PathBuf>, Sender<Converter>, Receiver<()>)> = vec![];

        for _ in 0..self.threads {
            let (tx, rx) = channel::<PathBuf>();
            let (tx2, rx2) = channel::<Converter>();
            let (tx3, rx3) = channel::<()>();
            threads.push((
                std::thread::spawn(move || {
                    let args = rx2.recv().unwrap();
                    loop {
                        match rx.recv() {
                            Ok(path) => {
                                if path == PathBuf::new() {
                                    break;
                                } else {
                                    match args.format {
                                        Format::Avif => {
                                            enc::avif(&path, args.speed.clamp(0, 10), args.quality.clamp(0, 100));
                                            tx3.send(()).unwrap();
                                        }
                                        Format::Jpeg => {
                                            enc::jpeg(&path, args.quality.clamp(0, 100));
                                            tx3.send(()).unwrap();
                                        }
                                        Format::Png => {
                                            enc::png(
                                                &path,
                                                match args.speed {
                                                    0 => CompressionType::Fast,
                                                    1 => CompressionType::Default,
                                                    _ => CompressionType::Best,
                                                },
                                            );
                                            tx3.send(()).unwrap();
                                        }
                                        Format::Webp => {
                                            enc::webp(
                                                &path,
                                                match args.quality {
                                                    0..=100 => WebPQuality::lossy(args.quality),
                                                    _ => WebPQuality::lossless(),
                                                },
                                            );
                                            tx3.send(()).unwrap();
                                        }
                                    }
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }),
                tx,
                tx2,
                rx3
            ));
        }

        for (_, _, tx, _) in &mut threads {
            tx.send(*self).unwrap();
        }

        let pb = ProgressBar::new(WalkDir::new("tmp").into_iter().filter(|e| e.as_ref().unwrap().path().is_file()).count() as u64);
        let mut i = 0;
        for entry in WalkDir::new("tmp") {
            let entry = entry.expect("Failed to read entry");
            let path = entry.path();

            if path.is_file() {
                let (_, tx, _, _) = &mut threads[i];
                tx.send(path.to_path_buf()).unwrap();
                i = (i + 1) % threads.len();
            }
        }

        for (_, tx, _, _) in &mut threads {
            tx.send(PathBuf::new()).unwrap()
        }

        loop {
            for (_, _, _, rx) in &mut threads {
                if rx.recv().is_ok() {
                    pb.inc(1);
                }
            }
            if pb.position() == pb.length().unwrap() {
                pb.finish();
                println!("Converted \"{}\"", file);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        loop {
            for i in (0..threads.len()).rev() {
                threads.remove(i).0.join().unwrap();
            }
            if threads.len() == 0 {
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        for entry in WalkDir::new("tmp") {
            let entry = entry.expect("Failed to read entry");
            let path = entry.path();
            if path.is_file() {
                if !path.to_str().unwrap().ends_with(self.format.to_str()) {
                    fs::remove_file(path).expect("Failed to remove file");
                }
            }
        }

        fs::rename(file, &format!("{}.bak", file)).expect("Failed to rename file");

        match archive_type {
            Archive::CBZ => cbz::pack("tmp", file),
            Archive::CB7 => sevenz_rust::compress_to_path("tmp", file).unwrap(),
            Archive::CBR => {
                let mut f: String;
                if file.ends_with(".cbr") { f = file.replace(".cbr", ".cbz") }
                else if file.ends_with(".rar") { f = file.replace(".rar", ".cbz") }
                else { f = file.to_string() }
                cbz::pack("tmp", &f)
            },
            Archive::CBT => {
                let mut archive = tar::Builder::new(fs::File::create(file).unwrap());
                archive.append_dir_all(".", "tmp").unwrap();
            },
            _ => (),
        };
        fs::remove_dir_all("tmp").expect("Failed to remove tmp directory");

        Ok(())
    }
}

impl Default for Converter {
    fn default() -> Self {
        Self {
            format: Format::Avif,
            quality: 30,
            speed: 3,
            threads: num_cpus::get() as u8,
            archive: Archive::Unset,
        }
    }
}

impl Format {
    pub fn from_str(s: &str) -> Result<Self, &str> {
        match s {
            "avif" => Ok(Self::Avif),
            "jpeg" => Ok(Self::Jpeg),
            "png" => Ok(Self::Png),
            "webp" => Ok(Self::Webp),
            _ => Err("Invalid format"),
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            Self::Avif => "avif",
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::Webp => "webp",
        }
    }
}

impl Archive {
    pub fn from_str(s: &str) -> Result<Self, &str> {
        match s {
            "cbz" | "CBZ" => Ok(Self::CBZ),
            "cbt" | "CBT" => Ok(Self::CBT),
            "cb7" | "CB7" => Ok(Self::CB7),
            "cbr" | "CBR" => Ok(Self::CBR),
            _ => Err("Invalid archive"),
        }
    }
}