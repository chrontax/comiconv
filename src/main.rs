use comiconv::*;

fn main() {
    let argss = std::env::args().collect::<Vec<String>>();
    if argss.len() < 2 {
        println!("No file specified. Use --help or -h for help");
        return;
    }

    let mut files = vec![];
    let mut converter = Converter::default();

    let mut skip = false;
    for i in 1..argss.len() {
        if skip { skip = false; continue }
        match argss[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            },
            "-v" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return;
            },
            "-s" | "--speed" => {
                converter.speed = argss[i+1].parse::<u8>().unwrap();
                skip = true;
            },
            "-q" | "--quality" => {
                converter.quality = argss[i+1].parse::<u8>().unwrap();
                skip = true;
            },
            "-f" | "--format" => {
                converter.format = Format::from_str(&argss[i+1]).unwrap();
                skip = true;
            },
            "-a" | "--archive" => {
                converter.archive = Archive::from_str(&argss[i+1]).unwrap();
                skip = true;
            },
            "-t" | "--threads" => {
                converter.threads = argss[i+1].parse::<u8>().unwrap();
                skip = true;
            },
            other => files.push(other),
        }
    }

    for file in files.iter() {
        converter.convert(file).unwrap();
    }
}

fn print_help() {
    println!("Usage: comiconv <files> [options]");
    println!();
    println!("Options:");
    println!();
    println!("  -h, --help\t\tPrint this help message");
    println!("  -v, --version\t\tPrint version");
    println!("  -s, --speed\t\tSet speed 0 (Slowest) - 10 (Fastest) (0-2 for png) default: 3");
    println!("  -q, --quality\t\tSet quality 0 (Worst) - 100 (Best) (101 for lossless webp) default: 30");
    println!("  -f, --format\t\tSet format (avif, webp, jpeg, png) default: avif");
    println!("  -a, --archive\t\tSet archive type (cbz, cbr, cb7, cbt) default: detects from file extension");
    println!("  -t, --threads\t\tSet number of threads default: number of cpus");
}
