use comiconv::*;
use std::fs;

#[test]
fn convert() {
    let converter = Converter::default();
    converter.convert("tests/test.cbz").unwrap();
    fs::remove_file("tests/test.cbz").unwrap();
    fs::rename("tests/test.cbz.bak", "tests/test.cbz").unwrap();
}
