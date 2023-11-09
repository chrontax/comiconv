use comiconv::*;

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        println!("No file specified. Use --help or -h for help");
        return;
    }

    let mut files = vec![];
    let mut converter = Converter::default();

    let mut skip = false;
    for i in 1..args.len() {
        if skip { skip = false; continue }
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            },
            "-v" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return;
            },
            "-s" | "--speed" => {
                converter.speed = args[i+1].parse::<u8>().unwrap();
                skip = true;
            },
            "-q" | "--quality" => {
                converter.quality = args[i+1].parse::<u8>().unwrap();
                skip = true;
            },
            "-f" | "--format" => {
                converter.format = match args[i].as_str() {
                    "avif" => Format::Avif,
                    "webp" => Format::Webp,
                    "jpeg" => Format::Jpeg,
                    "png" => Format::Png,
                    _ => {
                        println!("Invalid format. Use --help or -h for help");
                        return;
                    }
                };
                skip = true;
            },
            other => files.push(other),
        }
    }

    for file in files.iter() {
        converter.convert_file(file);
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
}
