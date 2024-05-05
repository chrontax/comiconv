use clap::{arg, command, value_parser, ArgAction};
use std::{
    io::Write,
    net::{Shutdown, TcpStream},
};

use comiconv::*;

fn main() {
    let matches = command!()
        .arg(
            arg!(-s --speed <VALUE> "Set speed: 0 (Slowest) - 10 (Fastest) (0-2 for png)")
                .required(false)
                .value_parser(value_parser!(u8)),
        )
        .arg(
            arg!(-q --quality <VALUE> "Set quality 0 (Worst) - 100 (Best) (ignored for webp, it's always lossless)")
                .required(false)
                .value_parser(value_parser!(u8)),
        )
        .arg(
            arg!(-f --format <VALUE>"Set format (avif, webp, jpeg, png)")
                .required(false)
                .value_parser(value_parser!(String)),
        )
        .arg(arg!(--quiet "Suppress progress messages").required(false))
        .arg(arg!(--backup "Keep backup of original file").required(false))
        .arg(
            arg!(--server <ADDRESS> "Server for online conversion")
                .required(false)
                .value_parser(value_parser!(String)),
        )
        .arg(
            arg!(<FILES> "Files to convert")
                .action(ArgAction::Append)
                .required(true),
        )
        .get_matches();
    let mut converter = Converter {
        quiet: matches.get_flag("quiet"),
        backup: matches.get_flag("backup"),
        ..Default::default()
    };
    if let Some(q) = matches.get_one::<u8>("quality") {
        converter.quality = *q
    }
    if let Some(f) = matches.get_one::<String>("format") {
        converter.format = f.parse().unwrap()
    }
    if let Some(s) = matches.get_one::<u8>("speed") {
        converter.speed = *s
    }
    let files = matches.get_many::<String>("FILES").unwrap().enumerate();
    let len = files.len();
    if let Some(addr) = matches.get_one::<String>("server") {
        let mut conn = TcpStream::connect(addr).expect("Failed to connect to server");
        for (i, file) in files {
            if !converter.quiet {
                print!("[{}/{}] ", i, len);
            }
            let mut done = false;
            while !done {
                match converter.convert_file_online(file, &mut conn) {
                    Ok(()) => done = true,
                    other => {
                        println!("Error: {:?}", other);
                        print!("Do you want to retry? [Y/n]: ");
                        std::io::stdout().flush().unwrap();
                        let mut str = String::new();
                        std::io::stdin().read_line(&mut str).unwrap();
                        match str.as_str() {
                            "y\n" | "Y\n" | "\n" => {
                                if let Err(ConvError::IoError(_)) = other {
                                    conn = TcpStream::connect(addr)
                                        .expect("Failed to connect to server");
                                }
                            }
                            _ => done = true,
                        }
                    }
                }
            }
        }
        conn.shutdown(Shutdown::Both).unwrap();
    } else {
        for (i, file) in files {
            if !converter.quiet {
                print!("[{}/{}] ", i, len);
            }
            let mut done = false;
            while !done {
                match converter.convert_file(file) {
                    Ok(()) => done = true,
                    other => {
                        println!("Error: {:?}", other);
                        print!("Do you want to retry? [Y/n]: ");
                        std::io::stdout().flush().unwrap();
                        let mut str = String::new();
                        std::io::stdin().read_line(&mut str).unwrap();
                        match str.as_str() {
                            "y\n" | "Y\n" | "\n" => (),
                            _ => done = true,
                        }
                    }
                }
            }
        }
    }
    if !converter.quiet {
        println!("Done!");
    }
}
