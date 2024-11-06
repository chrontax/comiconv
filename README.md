# Comiconv

Comic book converter written in rust.

## Features

- reads 7Z/CB7, TAR/CBT and ZIP/CBZ
- saves in the same archive format as input
- can convert images to JPEG, JPEGXL, PNG, WEBP and AVIF
- can convert locally or on a server running [comiconv-server](https://github.com/chrontax/comiconv-server)

## Installation

You can install comiconv through cargo:

```sh
cargo install comiconv
```

## Usage

```none
Usage: comiconv [OPTIONS] <FILES>...

Arguments:
  <FILES>...  Files to convert

Options:
  -s, --speed <VALUE>     Set speed: 0 (Slowest) - 10 (Fastest) (0-2 for png)
  -q, --quality <VALUE>   Set quality 0 (Worst) - 100 (Best) (ignored for webp, it's always lossless)
  -f, --format <VALUE>    Set format (avif, webp, jpeg, jxl, png)
      --quiet             Suppress progress messages
      --backup            Keep backup of original file
      --server <ADDRESS>  Server for online conversion
  -h, --help              Print help
  -V, --version           Print version
```

## Examples

Convert using default settings:

```sh
comiconv path/to/file
```

Convert to jpeg with quality 80:

```sh
comiconv -f jpeg -q 80 path/to/file
```
