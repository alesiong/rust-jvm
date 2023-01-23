use std::{fs::File, io::Read, sync::Arc};

use jvm::{
    class::parser,
    descriptor,
    runtime::{self, parse_class},
};

fn main() {
    let mut class_file = Vec::new();
    // File::open("data/Add.class")
        File::open("data/rt/java.base/java/lang/Object.class")
        .unwrap()
        .read_to_end(&mut class_file)
        .unwrap();

    let (_, cls) = parser::class_file(&class_file).unwrap();
    // println!("{:#?}", cls);

    let class = Arc::new(parse_class(&cls));
    println!("{:#?}", class);

    return;
    let mut main_thread = runtime::Thread::new(1024);
    main_thread.new_frame(
        class,
        "add",
        &[descriptor::FieldType::Int, descriptor::FieldType::Int],
    );

    let frame = main_thread.top_frame().unwrap();

    frame.add_local_int(10);
    frame.add_local_int(20);
    let v = main_thread.execute();
    println!("{}", unsafe { v.int });
}
