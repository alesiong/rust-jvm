use std::sync::Arc;

use crate::class::JavaStr;
use crate::runtime::NativeResult;
use crate::{
    descriptor::{FieldDescriptor, MethodDescriptor},
    runtime::Class,
};

#[derive(Debug)]
pub enum ConstantPoolInfo {
    Utf8(Arc<JavaStr>),
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    Class(CpClassInfo),
    String(Arc<JavaStr>),
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
    NameAndType(CpNameAndTypeInfo<Arc<JavaStr>>),
    MethodHandle,
    MethodType,
    Dynamic,
    InvokeDynamic,
    Module(Arc<JavaStr>),
    Package(Arc<JavaStr>),
    Empty,
}

#[derive(Debug)]
pub struct CpClassInfo {
    pub(crate) name: Arc<str>,
    // TODO: array
    pub(crate) class: Arc<once_cell::sync::OnceCell<Arc<Class>>>,
}

impl CpClassInfo {
    pub(crate) fn set_class(&self, class: &Arc<Class>) {
        self.class.set(Arc::clone(class)).unwrap();
    }

    pub(crate) fn get_or_load_class(
        &self,
        resolver: impl FnOnce() -> NativeResult<Arc<Class>>,
    ) -> NativeResult<Arc<Class>> {
        Ok(Arc::clone(self.class.get_or_try_init(resolver)?))
    }
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
    pub(crate) name: Arc<JavaStr>,
    pub(crate) descriptor: T,
}

#[derive(Debug, Copy, Clone)]
pub enum FieldIndex {
    Unresolved,
    NotThisClass,
    Instance(u16),
    Static(u16),
}
