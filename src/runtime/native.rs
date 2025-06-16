use crate::descriptor::FieldType;
use crate::runtime;
use crate::runtime::{Heap, NativeResult};
use dashmap::DashMap;
use std::sync::{Arc, LazyLock, RwLock};

pub type NativeFunction = fn(NativeEnv) -> NativeResult<Option<NativeVariable>>;

pub struct NativeEnv {
    pub args: Vec<NativeVariable>,
    pub heap: &'static RwLock<Heap>,
    pub class: Arc<runtime::Class>,
}
pub enum NativeVariable {
    Boolean(bool),
    Byte(i8),
    Char(u16),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Reference(u32),
}

// key: class_name, method_name, method_descriptor
type Key = (String, String, Vec<FieldType>);
pub(in crate::runtime) static NATIVE_FUNCTIONS: LazyLock<DashMap<Key, NativeFunction>> =
    LazyLock::new(DashMap::new);

pub fn register_natives() {
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

fn native_object_hash_code(env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    let NativeVariable::Reference(rf) = env.args[0] else {
        panic!("native_object_hash_code: invalid args");
    };
    Ok(Some(NativeVariable::Int(rf as i32)))
}

fn native_object_clone(env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    let NativeVariable::Reference(obj_id) = env.args[0] else {
        panic!("native_object_hash_code: invalid args");
    };
    let object = env.heap.read().unwrap().get(obj_id);
    // TODO: check clonable

    let cloned = env.heap.write().unwrap().clone_object(&object);
    Ok(Some(NativeVariable::Reference(cloned)))
}

fn native_nop(_: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    Ok(None)
}
