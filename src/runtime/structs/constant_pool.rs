use std::sync::Arc;

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
    Module,
    Package,
    Empty,
}

#[derive(Debug)]
pub struct CpClassInfo {
    pub(crate) name: Arc<String>,
    // TODO: array
    pub(crate) class: Option<Arc<Class>>,
}

#[derive(Debug)]
pub struct CpNameAndTypeInfo<T> {
    pub(crate) name: Arc<String>,
    pub(crate) descriptor: T,
}
