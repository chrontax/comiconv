use clap::{arg, command, value_parser, ArgAction};
use std::net::{Shutdown, TcpStream};

use comiconv::*;

fn main() {
    let matches = command!()
        .arg(
            arg!(-s --speed <VALUE> "Set speed: 0 (Slowest) - 10 (Fastest) (0-2 for png)")
                .required(false)
                .value_parser(value_parser!(u8)),
        )
        .arg(
            arg!(-q --quality <VALUE> "Set quality 0 (Worst) - 100 (Best)")
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
            converter.convert_file_online(file, &mut conn);
        }
        conn.shutdown(Shutdown::Both).unwrap();
    } else {
        for (i, file) in files {
            if !converter.quiet {
                print!("[{}/{}] ", i, len);
            }
            converter.convert_file(file);
        }
    }
    if !converter.quiet {
        println!("Done!");
    }
}
