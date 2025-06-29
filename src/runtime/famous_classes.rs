use crate::{
    descriptor::FieldType,
    runtime,
    runtime::{
        Class, VmEnv,
        class_loader::initialize_class,
        global::{BOOTSTRAP_CLASS_LOADER, HEAP},
        register_natives,
    },
};
use std::sync::{Arc, OnceLock};

pub(super) static OBJECT_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static STRING_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static CLASS_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static BYTE_ARRAY_CLASS: OnceLock<Arc<Class>> = OnceLock::new();

pub(super) fn init_famous_classes() {
    let bootstrap = BOOTSTRAP_CLASS_LOADER.get().unwrap();
    OBJECT_CLASS
        .set(
            bootstrap
                .resolve_class("java/lang/Object")
                .expect("must succeed"),
        )
        .expect("must not be set");
    STRING_CLASS
        .set(
            bootstrap
                .resolve_class("java/lang/String")
                .expect("must succeed"),
        )
        .expect("must not be set");
    CLASS_CLASS
        .set(
            bootstrap
                .resolve_class("java/lang/Class")
                .expect("must succeed"),
        )
        .expect("must not be set");
    BYTE_ARRAY_CLASS
        .set(
            bootstrap
                .resolve_primitive_array_class(&FieldType::Byte)
                .expect("must succeed"),
        )
        .expect("must not be set");

    let bootstrap_thread = runtime::Thread::new(1024);
    let env = VmEnv::new(&bootstrap_thread, &HEAP);

    register_natives();

    initialize_class(&env, STRING_CLASS.get().unwrap()).expect("must succeed");
    initialize_class(&env, CLASS_CLASS.get().unwrap()).expect("must succeed");
}
