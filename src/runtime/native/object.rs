use crate::runtime::{NativeEnv, NativeResult, NativeVariable, native::NATIVE_FUNCTIONS};

// public native int hashCode();
pub(super) fn native_object_hash_code(env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    let NativeVariable::Reference(rf) = env.args[0] else {
        panic!("native_object_hash_code: invalid args");
    };
    Ok(Some(NativeVariable::Int(rf as i32)))
}

// protected native Object clone() throws CloneNotSupportedException;
fn native_object_clone(env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    let NativeVariable::Reference(obj_id) = env.args[0] else {
        panic!("native_object_hash_code: invalid args");
    };
    let heap = env.heap.read().unwrap();
    let object = heap.get(obj_id);
    // TODO: check clonable
    drop(heap);

    let cloned = env.heap.write().unwrap().clone(object.as_ref());
    Ok(Some(NativeVariable::Reference(cloned)))
}

pub(super) fn register_natives() {
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/Object".to_string(),
            "hashCode".to_string(),
            vec![],
        ),
        native_object_hash_code,
    );
    NATIVE_FUNCTIONS.insert(
        ("java/lang/Object".to_string(), "clone".to_string(), vec![]),
        native_object_clone,
    );
}
