use std::{fs::File, io::Read};

use jvm::{frame::Frame, parser};

fn main() {
    let mut class_file = Vec::new();
    // File::open("data/Add.class")
    File::open("data/rt/java.base/java/lang/String.class")
        .unwrap()
        .read_to_end(&mut class_file)
        .unwrap();

    let (_, cls) = parser::class_file(&class_file).unwrap();
    println!("{:#?}", cls);

    // let mut add_frame = Frame::new(&cls, "add");
    // add_frame.add_local_int(10);
    // add_frame.add_local_int(20);
    // let v = add_frame.execute();
    // println!("{}", unsafe { v.int });
}
