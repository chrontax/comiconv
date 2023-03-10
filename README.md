# Comiconv
Comic book converter written in rust.

## Features
* reads RAR/CBR, 7Z/CB7, TAR/CBT and ZIP/CBZ
* saves in the same archive format as input (except RAR for which output is ZIP)
* can convert images to JPEG, PNG, WEBP and AVIF

## Usage
```bash
Usage: comiconv <files> [options]

Options:

  -h, --help		Print this help message
  -v, --version		Print version
  -s, --speed		Set speed 0 (Slowest) - 10 (Fastest) (0-2 for png) default: 3
  -q, --quality		Set quality 0 (Worst) - 100 (Best) (101 for lossless webp) default: 30
  -f, --format		Set format (avif, webp, jpeg, png) default: avif
  -a, --archive		Set archive type (cbz, cbr, cb7, cbt) default: detects from file extension
  -t, --threads		Set number of threads default: number of cpus
```

## Examples
Convert using default settings
```bash
comiconv path/to/file
```

Convert to jpeg with quality 80 and cbz archive
```bash
comiconv paht/to/file -f jpeg -q 80 -a cbz
```