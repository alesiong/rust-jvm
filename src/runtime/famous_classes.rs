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
pub(super) static SERIALIZABLE_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static SYSTEM_CLASS: OnceLock<Arc<Class>> = OnceLock::new();

// exceptions
pub(super) static THROWABLE_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static ERROR_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static EXCEPTION_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static RUNTIME_EXCEPTION_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static ARRAY_STORE_EXCEPTION_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static LINKAGE_ERROR_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static CLASS_CAST_EXCEPTION_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static CLASS_FORMAT_ERROR_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static NO_SUCH_METHOD_ERROR_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static NO_SUCH_FIELD_ERROR_CLASS: OnceLock<Arc<Class>> = OnceLock::new();

pub(super) static NULL_POINTER_EXCEPTION_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static CLONE_NOT_SUPPORTED_EXCEPTION_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static INDEX_OUT_OF_BOUND_EXCEPTION_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static ARRAY_INDEX_OUT_OF_BOUND_EXCEPTION_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static ARITHMETIC_EXCEPTION_CLASS: OnceLock<Arc<Class>> = OnceLock::new();
pub(super) static NEGATIVE_ARRAY_SIZE_EXCEPTION_CLASS: OnceLock<Arc<Class>> = OnceLock::new();

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

    resolve_famous!(SERIALIZABLE_CLASS, "java/io/Serializable");
    resolve_famous!(SYSTEM_CLASS, "java/lang/System");

    resolve_famous!(THROWABLE_CLASS, "java/lang/Throwable");
    resolve_famous!(ERROR_CLASS, "java/lang/Error");
    resolve_famous!(EXCEPTION_CLASS, "java/lang/Exception");
    resolve_famous!(RUNTIME_EXCEPTION_CLASS, "java/lang/RuntimeException");
    resolve_famous!(ARRAY_STORE_EXCEPTION_CLASS, "java/lang/ArrayStoreException");
    resolve_famous!(LINKAGE_ERROR_CLASS, "java/lang/LinkageError");
    resolve_famous!(CLASS_CAST_EXCEPTION_CLASS, "java/lang/ClassCastException");
    resolve_famous!(CLASS_FORMAT_ERROR_CLASS, "java/lang/ClassFormatError");

    resolve_famous!(NO_SUCH_METHOD_ERROR_CLASS, "java/lang/NoSuchMethodError");
    resolve_famous!(NO_SUCH_FIELD_ERROR_CLASS, "java/lang/NoSuchFieldError");

    resolve_famous!(
        NULL_POINTER_EXCEPTION_CLASS,
        "java/lang/NullPointerException"
    );
    resolve_famous!(
        CLONE_NOT_SUPPORTED_EXCEPTION_CLASS,
        "java/lang/CloneNotSupportedException"
    );
    resolve_famous!(
        INDEX_OUT_OF_BOUND_EXCEPTION_CLASS,
        "java/lang/IndexOutOfBoundsException"
    );
    resolve_famous!(
        ARRAY_INDEX_OUT_OF_BOUND_EXCEPTION_CLASS,
        "java/lang/ArrayIndexOutOfBoundsException"
    );
    resolve_famous!(ARITHMETIC_EXCEPTION_CLASS, "java/lang/ArithmeticException");
    resolve_famous!(
        NEGATIVE_ARRAY_SIZE_EXCEPTION_CLASS,
        "java/lang/NegativeArraySizeException"
    );
}
