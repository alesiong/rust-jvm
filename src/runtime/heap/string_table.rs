use crate::{
    class::JavaStr,
    runtime::{
        Class, Object, Variable,
        famous_classes::{BYTE_ARRAY_CLASS, STRING_CLASS},
        heap::SpecialObject,
        structs::ObjectMonitor,
    },
};
use std::{collections::HashMap, sync::Arc};

pub struct StringTable {
    pub(in crate::runtime) map: HashMap<Arc<[u8]>, StringTableEntry>,
}

impl StringTable {
    pub(in crate::runtime) fn new() -> Self {
        Self {
            map: Default::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct StringTableEntry {
    pub(in crate::runtime) string_id: u32,
    pub(in crate::runtime) bytes_id: u32,
    pub(in crate::runtime) hash: i32,
    pub(in crate::runtime) has_multi_bytes: bool,
}

#[derive(Debug, Clone)]
pub enum SpecialStringObject {
    Bytes {
        monitor: ObjectMonitor,
        bytes: Arc<[u8]>,
    },
    String {
        monitor: ObjectMonitor,
        bytes_id: u32,
        bytes: Arc<[u8]>,
        hash: i32,
        has_multi_bytes: bool,
    },
}

impl Object for SpecialStringObject {
    fn get_class(&self) -> &Arc<Class> {
        match self {
            SpecialStringObject::Bytes { .. } => BYTE_ARRAY_CLASS
                .get()
                .expect("byte array class must be loaded"),
            SpecialStringObject::String { .. } => {
                STRING_CLASS.get().expect("string class must be loaded")
            }
        }
    }

    unsafe fn put_field(&self, _index: usize, _v: Variable) {
        panic!("cannot modify interned string");
    }

    unsafe fn get_field(&self, index: usize) -> Variable {
        let SpecialStringObject::String {
            bytes_id,
            hash,
            has_multi_bytes,
            ..
        } = self
        else {
            panic!("not an object");
        };

        let field = self
            .get_class()
            .instance_fields_info
            .iter()
            .find(|f| f.index == index as _)
            .expect("invalid field");

        if field.name.as_ref() == JavaStr::from_str("value").as_ref() {
            Variable {
                reference: *bytes_id,
            }
        } else if field.name.as_ref() == JavaStr::from_str("coder").as_ref() {
            Variable {
                int: if *has_multi_bytes { 1 } else { 0 },
            }
        } else if field.name.as_ref() == JavaStr::from_str("hash").as_ref() {
            Variable { int: *hash }
        } else if field.name.as_ref() == JavaStr::from_str("hashIsZero").as_ref() {
            Variable {
                int: if *hash == 0 { 1 } else { 0 },
            }
        } else {
            panic!("invalid field");
        }
    }

    unsafe fn put_array_index_raw(&self, _index: usize, _v: &[u8], _element_size: usize) {
        panic!("cannot modify interned string");
    }

    unsafe fn get_array_index_raw(&self, index: usize, element_size: usize) -> &[u8] {
        let SpecialStringObject::Bytes { bytes, .. } = self else {
            panic!("not an array");
        };
        debug_assert_eq!(element_size, 1);
        &bytes[index * element_size..(index + 1) * element_size]
    }

    fn get_array_size(&self, element_size: usize) -> usize {
        let SpecialStringObject::Bytes { bytes, .. } = self else {
            panic!("not an array");
        };
        debug_assert_eq!(element_size, 1);
        bytes.len()
    }

    unsafe fn get_u8_array_const(&self) -> *const u8 {
        let SpecialStringObject::Bytes { bytes, .. } = self else {
            panic!("not an array");
        };
        bytes.as_ptr()
    }

    fn get_monitor(&self) -> &ObjectMonitor {
        match self {
            SpecialStringObject::Bytes { monitor, .. } => monitor,
            SpecialStringObject::String { monitor, .. } => monitor,
        }
    }
}
impl SpecialObject for SpecialStringObject {}

impl SpecialStringObject {
    pub(in crate::runtime) fn get_bytes(&self) -> &[u8] {
        match self {
            SpecialStringObject::Bytes { bytes, .. } => bytes,
            SpecialStringObject::String { bytes, .. } => bytes,
        }
    }
}
