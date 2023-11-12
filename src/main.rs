use std::net::{Shutdown, TcpStream};

use comiconv::*;

fn main() {
    let mut args = std::env::args();
    args.next();
    let mut files = vec![];
    let mut converter = Converter::default();
    let mut server = String::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            }
            "-v" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "-s" | "--speed" => converter.speed = args.next().unwrap().parse::<u8>().unwrap(),
            "-q" | "--quality" => converter.quality = args.next().unwrap().parse::<u8>().unwrap(),
            "-f" | "--format" => {
                converter.format = match args.next().unwrap().as_str() {
                    "a" | "avif" => Format::Avif,
                    "w" | "webp" => Format::Webp,
                    "j" | "jpeg" => Format::Jpeg,
                    "p" | "png" => Format::Png,
                    _ => {
                        println!("Invalid format. Use --help or -h for help");
                        return;
                    }
                }
            }
            "--quiet" => converter.quiet = true,
            "--backup" => converter.backup = true,
            "--server" => server = args.next().unwrap(),
            _ => files.push(arg),
        }
    }
    if files.is_empty() {
        println!("No files specified. Use --help or -h for help");
        return;
    }
    let mut i = 1;
    let len = files.len();
    if server.is_empty() {
        for file in files {
            print!("[{}/{}] ", i, len);
            converter.convert_file(&file);
            i += 1;
        }
    } else {
        let mut conn = TcpStream::connect(server).unwrap();
        for file in files {
            print!("[{}/{}] ", i, len);
            converter.convert_file_online(&file, &mut conn);
            i += 1;
        }
        conn.shutdown(Shutdown::Both).unwrap();
    }
    println!("Done!");
}

fn print_help() {
    println!("Usage: comiconv <files> [options]");
    println!();
    println!("Options:");
    println!();
    println!("  -h, --help\t\tPrint this help message");
    println!("  -v, --version\t\tPrint version");
    println!("  -s, --speed\t\tSet speed 0 (Slowest) - 10 (Fastest) (0-2 for png) default: 3");
    println!("  -q, --quality\t\tSet quality 0 (Worst) - 100 (Best) default: 30");
    println!("  -f, --format\t\tSet format (avif, webp, jpeg, png) default: avif");
    println!("      --quiet\t\tDon't print progress");
    println!("      --backup\t\tCreate backup of original file");
    println!("      --server\t\tSet server to use for online conversion");
}
