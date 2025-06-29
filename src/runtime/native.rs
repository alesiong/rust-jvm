mod class;
mod object;
mod string;
mod system;

use crate::{
    descriptor::FieldType,
    runtime,
    runtime::{NativeResult, heap::Heap},
};
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

impl NativeVariable {
    pub fn get_boolean(&self) -> bool {
        match self {
            NativeVariable::Boolean(b) => *b,
            _ => panic!("get_boolean: invalid type"),
        }
    }
    pub fn get_byte(&self) -> i8 {
        match self {
            NativeVariable::Byte(b) => *b,
            _ => panic!("get_byte: invalid type"),
        }
    }

    pub fn get_char(&self) -> u16 {
        match self {
            NativeVariable::Char(c) => *c,
            _ => panic!("get_char: invalid type"),
        }
    }

    pub fn get_int(&self) -> i32 {
        match self {
            NativeVariable::Int(i) => *i,
            _ => panic!("get_int: invalid type"),
        }
    }

    pub fn get_ref(&self) -> u32 {
        match self {
            NativeVariable::Reference(r) => *r,
            _ => panic!("get_ref: invalid type"),
        }
    }
}

// key: class_name, method_name, method_descriptor
type Key = (String, String, Vec<FieldType>);
pub(in crate::runtime) static NATIVE_FUNCTIONS: LazyLock<DashMap<Key, NativeFunction>> =
    LazyLock::new(DashMap::new);

pub(in crate::runtime) fn register_natives() {
    object::register_natives();
    system::register_natives();
    string::register_natives();
    class::register_natives();
}

fn native_nop(_: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    Ok(None)
}
