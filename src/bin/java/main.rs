use jvm::{
    descriptor,
    runtime::{
        genesis, {self},
    },
};

fn main() {
    genesis(
        "/opt/homebrew/Cellar/openjdk@17/17.0.15/libexec/openjdk.jdk/Contents/Home/",
        "data/test/",
    );

    // TODO: load main class
    let mut main_thread = runtime::Thread::new(1024);
    main_thread.new_main_frame(
        "D",
        "main",
        &[descriptor::FieldType::Array(Box::new(
            descriptor::FieldType::Object("java/lang/String".to_string()),
        ))],
    );

    // let frame = main_thread.top_frame().unwrap();

    // frame.add_local_int(10);
    // frame.add_local_int(20);
    // frame.add_local_reference(10);
    // frame.add_local_reference(20);
    main_thread.execute().unwrap();
    // println!("{}", unsafe { v.get_int() });
}
