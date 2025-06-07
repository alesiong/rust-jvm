use std::sync::{Arc, RwLock};

use crate::{
    descriptor::{FieldDescriptor, MethodDescriptor},
    runtime::Class,
};

#[derive(Debug)]
pub enum ConstantPoolInfo {
    Utf8(Arc<String>),
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    Class(CpClassInfo),
    String(Arc<String>),
    Fieldref {
        class: CpClassInfo,
        name_and_type: CpNameAndTypeInfo<FieldDescriptor>,
        field_index: FieldIndex,
    },
    Methodref {
        class: CpClassInfo,
        name_and_type: CpNameAndTypeInfo<MethodDescriptor>,
    },
    InterfaceMethodref {
        class: CpClassInfo,
        name_and_type: CpNameAndTypeInfo<MethodDescriptor>,
    },
    NameAndType(CpNameAndTypeInfo<Arc<String>>),
    MethodHandle,
    MethodType,
    Dynamic,
    InvokeDynamic,
    Module(Arc<String>),
    Package(Arc<String>),
    Empty,
}

#[derive(Debug)]
pub struct CpClassInfo {
    pub(crate) name: Arc<String>,
    // TODO: array, oncecell
    pub(crate) class: RwLock<Option<Arc<Class>>>,
}

impl Clone for CpClassInfo {
    fn clone(&self) -> Self {
        CpClassInfo {
            name: Arc::clone(&self.name),
            class: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CpNameAndTypeInfo<T> {
    pub(crate) name: Arc<String>,
    pub(crate) descriptor: T,
}

#[derive(Debug, Copy, Clone)]
pub enum FieldIndex {
    Unresolved,
    NotThisClass,
    Instance(u16),
    Static(u16),
}
