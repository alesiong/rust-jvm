use jvm::runtime::{ClassPathModule, JModModule, init_bootstrap_class_loader, register_natives};
use jvm::{
    descriptor,
    runtime::{self},
};

fn main() {
    init_bootstrap_class_loader(vec![
        Box::new(JModModule::new(
            "/opt/homebrew/Cellar/openjdk@17/17.0.15/libexec/openjdk.jdk/Contents/Home/",
            "java.base",
        )),
        Box::new(ClassPathModule::new("main", "data")),
    ]);

    register_natives();

    // TODO: load main class
    let mut main_thread = runtime::Thread::new(1024);
    main_thread.new_main_frame(
        "Add",
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
