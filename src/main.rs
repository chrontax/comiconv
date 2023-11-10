use comiconv::*;

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    let mut files = vec![];
    let mut converter = Converter::default();
    let mut server = String::new();
    let mut skip = false;
    for i in 1..args.len() {
        if skip {
            skip = false;
            continue;
        }
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            }
            "-v" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "-s" | "--speed" => {
                converter.speed = args[i + 1].parse::<u8>().unwrap();
                skip = true;
            }
            "-q" | "--quality" => {
                converter.quality = args[i + 1].parse::<u8>().unwrap();
                skip = true;
            }
            "-f" | "--format" => {
                converter.format = match args[i + 1].as_str() {
                    "a" | "avif" => Format::Avif,
                    "w" | "webp" => Format::Webp,
                    "j" | "jpeg" => Format::Jpeg,
                    "p" | "png" => Format::Png,
                    _ => {
                        println!("Invalid format. Use --help or -h for help");
                        return;
                    }
                };
                skip = true;
            }
            "--quiet" => converter.quiet = true,
            "--backup" => converter.backup = true,
            "--server" => {
                server = args[i + 1].clone();
                skip = true;
            }
            other => files.push(other),
        }
    }
    if files.is_empty() {
        println!("No files specified. Use --help or -h for help");
        return;
    }
    let mut i = 1;
    let len = files.len();
    for file in files {
        print!("[{}/{}] ", i, len);
        if server.is_empty() {
            converter.convert_file(file);
        } else {
            converter.convert_file_online(file, &server);
        }
        i += 1;
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
