use crate::{
    class::JavaStr,
    runtime::{Class, Object, Variable, heap::SpecialObject},
};
use std::{collections::HashMap, sync::Arc};

pub struct StringTable {
    pub(in crate::runtime) map: HashMap<Arc<[u8]>, StringTableEntry>,
}

impl StringTable {
    pub(in crate::runtime) fn new() -> Self {
        Self {
            map: HashMap::new(),
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
        bytes_class: Arc<Class>,
        bytes: Arc<[u8]>,
    },
    String {
        string_class: Arc<Class>,
        bytes_id: u32,
        bytes: Arc<[u8]>,
        hash: i32,
        has_multi_bytes: bool,
    },
}

impl Object for SpecialStringObject {
    fn get_class(&self) -> &Arc<Class> {
        match self {
            SpecialStringObject::Bytes { bytes_class, .. } => bytes_class,
            SpecialStringObject::String { string_class, .. } => string_class,
        }
    }

    unsafe fn put_field(&self, index: usize, v: Variable) {
        panic!("cannot modify interned string");
    }

    unsafe fn get_field(&self, index: usize) -> Variable {
        let SpecialStringObject::String {
            string_class,
            bytes_id,
            bytes,
            hash,
            has_multi_bytes,
        } = self
        else {
            panic!("not an object");
        };

        let field = string_class
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
        let SpecialStringObject::Bytes { bytes_class, bytes } = self else {
            panic!("not an array");
        };
        debug_assert_eq!(element_size, 1);
        &bytes[index * element_size..(index + 1) * element_size]
    }

    fn get_array_size(&self, element_size: usize) -> usize {
        let SpecialStringObject::Bytes { bytes_class, bytes } = self else {
            panic!("not an array");
        };
        debug_assert_eq!(element_size, 1);
        bytes.len()
    }
}
impl SpecialObject for SpecialStringObject {}
