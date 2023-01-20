use std::{fs::File, io::Read};

use jvm::parser;

fn main() {
    let mut class_file = Vec::new();
    File::open("data/Add.class")
        .unwrap()
        .read_to_end(&mut class_file)
        .unwrap();

    let (_, cls) = parser::class_file(&class_file).unwrap();
    println!("{:?}", cls);
}
