use crate::{
    class::JavaStr,
    descriptor::FieldType,
    runtime::{
        NativeEnv, NativeResult, NativeVariable,
        NativeVariable::{Boolean, Reference},
        class_loader::intern_string,
        heap::reflection::SpecialClassObject,
        native::NATIVE_FUNCTIONS,
    },
};
use std::{any::Any, sync::atomic::Ordering::Relaxed};

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
    // TODO: always true
    Ok(Some(Boolean(true)))
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
