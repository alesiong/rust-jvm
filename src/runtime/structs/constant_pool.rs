use std::sync::Arc;

use crate::{
    class::JavaStr,
    descriptor::{FieldDescriptor, MethodDescriptor},
    runtime::{Class, MethodInfo, NativeResult},
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
    Fieldref(Fieldref),
    Methodref(Methodref),
    InterfaceMethodref(Methodref),
    NameAndType(CpNameAndTypeInfo<Arc<JavaStr>>),
    MethodHandle(MethodHandle),
    MethodType,
    Dynamic {
        bootstrap_method_attr_index: u16,
        name_and_type: CpNameAndTypeInfo<FieldDescriptor>,
    },
    InvokeDynamic {
        bootstrap_method_attr_index: u16,
        name_and_type: CpNameAndTypeInfo<MethodDescriptor>,
    },
    Module(Arc<JavaStr>),
    Package(Arc<JavaStr>),
    Empty,
}

#[derive(Debug)]
pub struct CpClassInfo {
    pub(crate) name: Arc<str>,
    pub(crate) class: once_cell::sync::OnceCell<Arc<Class>>,
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

#[derive(Debug, Clone)]
pub struct Fieldref {
    pub(crate) class_name: Arc<str>,
    pub(crate) name_and_type: CpNameAndTypeInfo<FieldDescriptor>,
    pub(crate) resolve: once_cell::sync::OnceCell<FieldResolve>,
}

impl Fieldref {
    pub(crate) fn is_resolved(&self) -> bool {
        self.resolve.get().is_some()
    }
}

#[derive(Debug, Clone)]
pub(crate) enum FieldResolve {
    InThisClass(usize),
    OtherClass { class: Arc<Class>, index: usize },
}

impl FieldResolve {
    pub(crate) fn get_index(&self) -> usize {
        match self {
            FieldResolve::InThisClass(index) => *index,
            FieldResolve::OtherClass { index, .. } => *index,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Methodref {
    pub(crate) class_name: Arc<str>,
    pub(crate) name_and_type: CpNameAndTypeInfo<MethodDescriptor>,
    pub(crate) resolve: once_cell::sync::OnceCell<MethodResolve>,
}

impl Methodref {
    pub fn is_signature_equal(&self, method_info: &MethodInfo) -> bool {
        self.name_and_type.name == method_info.name
            && self.name_and_type.descriptor == method_info.descriptor
    }
}

#[derive(Debug, Clone)]
pub(crate) enum MethodResolve {
    InThisClass {
        index: usize,
        vtable_index: isize,
    },
    OtherClass {
        class: Arc<Class>,
        index: usize,
        vtable_index: isize,
    },
}

impl MethodResolve {
    pub(crate) fn get_index(&self) -> usize {
        match self {
            MethodResolve::InThisClass { index, .. } => *index,
            MethodResolve::OtherClass { index, .. } => *index,
        }
    }

    pub(crate) fn get_class_and_index<'a>(
        &'a self,
        this_class: &'a Arc<Class>,
    ) -> (&'a Arc<Class>, usize, isize) {
        match self {
            MethodResolve::InThisClass {
                index,
                vtable_index,
            } => (this_class, *index, *vtable_index),
            MethodResolve::OtherClass {
                class,
                index,
                vtable_index,
            } => (class, *index, *vtable_index),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MethodHandle {
    pub(crate) reference_kind: ReferenceKind,
    pub(crate) reference_index: u16,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ReferenceKind {
    GetField,
    GetStatic,
    PutField,
    PutStatic,
    InvokeVirtual,
    InvokeStatic,
    InvokeSpecial,
    NewInvokeSpecial,
    InvokeInterface,
}
