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
pub(super) static CLONEABLE_CLASS: OnceLock<Arc<Class>> = OnceLock::new();

pub(super) fn init_famous_classes() {
    let bootstrap = BOOTSTRAP_CLASS_LOADER.get().unwrap();

    macro_rules! resolve_famous {
        ($cls:ident, $name:literal) => {
            $cls.set(bootstrap.resolve_class($name).expect("must succeed"))
                .expect("must not be set");
        };
    }

    resolve_famous!(OBJECT_CLASS, "java/lang/Object");
    resolve_famous!(STRING_CLASS, "java/lang/String");
    resolve_famous!(CLASS_CLASS, "java/lang/Class");
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

    resolve_famous!(CLONEABLE_CLASS, "java/lang/Cloneable");

    initialize_class(&env, STRING_CLASS.get().unwrap()).expect("must succeed");
    initialize_class(&env, CLASS_CLASS.get().unwrap()).expect("must succeed");
}
