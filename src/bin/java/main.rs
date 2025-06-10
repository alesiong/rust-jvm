use std::{fs::File, io::Read, sync::Arc};

use jvm::runtime::{ClassPathModule, JModModule, init_bootstrap_class_loader, register_natives};
use jvm::{
    class::parser,
    descriptor,
    runtime::{self, parse_class},
};

fn main() {

    init_bootstrap_class_loader(vec![
        Box::new(JModModule::new(
            "/opt/homebrew/Cellar/openjdk@17/17.0.15/libexec/openjdk.jdk/Contents/Home/",
            "java.base",
        )),
        Box::new(ClassPathModule::new("main", "data/")),
    ]);
    
    register_natives();

    let mut class_file = Vec::new();
    File::open("data/Add.class")
        // File::open("data/rt/java.base/java/lang/Object.class")
        .unwrap()
        .read_to_end(&mut class_file)
        .unwrap();

    let (_, cls) = parser::class_file(&class_file).unwrap();
    let class = parse_class(&cls);
    // println!("{:#?}", class);

    let mut main_thread = runtime::Thread::new(1024);
    main_thread.new_frame(
        Arc::new(class),
        "main",
        &[descriptor::FieldType::Array(Box::new(
            descriptor::FieldType::Object("java/lang/String".to_string()),
        ))],
        0,
    );

    let frame = main_thread.top_frame().unwrap();

    // frame.add_local_int(10);
    // frame.add_local_int(20);
    // frame.add_local_reference(10);
    // frame.add_local_reference(20);
    main_thread.execute();
    // println!("{}", unsafe { v.get_int() });
}
