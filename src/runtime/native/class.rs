use crate::{
    class::JavaStr,
    descriptor::FieldType,
    runtime::{
        NativeEnv, NativeResult, NativeVariable,
        NativeVariable::{Boolean, Reference},
        SpecialStringObject,
        class_loader::{get_class_object, intern_string},
        famous_classes::INT_TYPE_CLASS,
        heap::reflection::SpecialClassObject,
        native::NATIVE_FUNCTIONS,
    },
};
use std::{
    any::Any,
    sync::{Arc, atomic::Ordering::Relaxed},
};
use crate::runtime::famous_classes::{BOOLEAN_TYPE_CLASS, BYTE_TYPE_CLASS, CHAR_TYPE_CLASS, SHORT_TYPE_CLASS, FLOAT_TYPE_CLASS, DOUBLE_TYPE_CLASS, LONG_TYPE_CLASS, VOID_TYPE_CLASS};

// private native String initClassName();
fn init_class_name(env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    let this = env.args[0].get_ref();
    let this_obj = env.heap.read().unwrap().get(this);
    let class_obj = (&this_obj as &dyn Any)
        .downcast_ref::<SpecialClassObject>()
        .expect("must be class object");
    let class_binary_name = class_obj.class.class_name.replace("/", ".");
    let name_str = intern_string(&JavaStr::from_str(&class_binary_name).into());

    class_obj.name_str.store(name_str, Relaxed);

    Ok(Some(Reference(name_str)))
}

// private static native boolean desiredAssertionStatus0(Class<?> clazz);
fn desired_assertion_status0(_env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    // TODO: from config
    Ok(Some(Boolean(true)))
}

// static native Class<?> getPrimitiveClass(String name);
fn get_primitive_class(env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    let name_ref = env.args[0].get_ref();
    let object_name = env.heap.read().unwrap().get(name_ref);
    let string_name = (&object_name as &dyn Any)
        .downcast_ref::<SpecialStringObject>()
        .expect("must be string object");

    // TODO: exception
    let class_name = str::from_utf8(string_name.get_bytes()).expect("error");

    let class = match class_name {
        "boolean" => BOOLEAN_TYPE_CLASS.get().expect("must have init"),
        "byte" => BYTE_TYPE_CLASS.get().expect("must have init"),
        "char" => CHAR_TYPE_CLASS.get().expect("must have init"),
        "short" => SHORT_TYPE_CLASS.get().expect("must have init"),
        "float" => FLOAT_TYPE_CLASS.get().expect("must have init"),
        "double" => DOUBLE_TYPE_CLASS.get().expect("must have init"),
        "int" => INT_TYPE_CLASS.get().expect("must have init"),
        "long" => LONG_TYPE_CLASS.get().expect("must have init"),
        "void" => VOID_TYPE_CLASS.get().expect("must have init"),

        // TODO: exception
        _ => panic!("not primitive class {}", class_name),
    };
    Ok(Some(Reference(get_class_object(Arc::clone(class))?)))
}

fn native_class_register_natives(_env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/Class".to_string(),
            "initClassName".to_string(),
            vec![],
        ),
        init_class_name,
    );
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/Class".to_string(),
            "desiredAssertionStatus0".to_string(),
            vec![FieldType::Object("java/lang/Class".to_string())],
        ),
        desired_assertion_status0,
    );

    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/Class".to_string(),
            "getPrimitiveClass".to_string(),
            vec![FieldType::Object("java/lang/String".to_string())],
        ),
        desired_assertion_status0,
    );

    Ok(None)
}

pub(super) fn register_natives() {
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/Class".to_string(),
            "registerNatives".to_string(),
            vec![],
        ),
        native_class_register_natives,
    );
}
