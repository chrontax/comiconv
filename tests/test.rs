use comiconv::*;

#[test]
fn convert() {
    let converter = Converter::default();
    converter.convert_file("tests/test.cbz").unwrap();
}
