use std::fs;
use std::io;
use zip::{ZipArchive, ZipWriter};
use walkdir::WalkDir;

pub fn extract(dir: &str, file: &str) {
    let mut archive = ZipArchive::new(fs::File::open(file).expect("Failed to open file"))
        .expect("Failed to read archive");

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).expect("Failed to read file");
        let outpath = file.enclosed_name().unwrap();
        if let Some(p) = outpath.parent() {
            if !p.exists() {
                fs::create_dir_all(dir.to_owned() + p.to_str().unwrap())
                    .expect("Failed to create directory");
            }
        }
        let mut outfile = fs::File::create(dir.to_owned() + outpath.to_str().unwrap())
            .expect("Failed to create file");
        io::copy(&mut file, &mut outfile).expect("Failed to copy file");
    }
}

pub fn pack(dir: &str, name: &str) {
    let mut archive = ZipWriter::new(fs::File::create(name).expect("Failed to create file"));
    for entry in WalkDir::new(dir).into_iter().skip(1) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            archive.add_directory(path.to_str().unwrap(), zip::write::FileOptions::default())
                .expect("Failed to add directory");
        } else {
            let name = path.file_name().unwrap().to_str().unwrap();
            archive.start_file(name, zip::write::FileOptions::default())
                .expect("Failed to start file");
            io::copy(
                &mut fs::File::open(path).expect("Failed to open file"),
                &mut archive
            ).expect("Failed to copy file");
        }
    }
}