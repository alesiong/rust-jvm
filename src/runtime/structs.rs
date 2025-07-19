use std::{
    cell::Cell,
    sync::{Arc, RwLock},
};

pub use crate::runtime::heap::string_table::*;
pub use attributes::*;
pub use constant_pool::*;
pub use object::*;

use crate::{
    class::JavaStr,
    consts::{ClassAccessFlag, FieldAccessFlag, MethodAccessFlag},
    descriptor::{FieldDescriptor, FieldType, MethodDescriptor},
    runtime::{Variable, famous_classes::CLASS_FORMAT_ERROR_CLASS},
};

mod attributes;
mod constant_pool;
mod object;

#[derive(Debug)]
pub struct Class {
    pub(crate) constant_pool: Vec<ConstantPoolInfo>,
    pub(crate) access_flags: ClassAccessFlag,
    pub(crate) class_name: Arc<str>,
    pub(crate) super_class: Option<Arc<Class>>,
    pub(crate) interfaces: Vec<Arc<Class>>,
    pub(crate) static_fields_info: Vec<FieldInfo>,
    pub(crate) instance_fields_info: Vec<FieldInfo>,
    pub(crate) methods: Vec<MethodInfo>,
    pub(crate) attributes: Vec<AttributeInfo>,
    pub(crate) static_fields: Vec<RwLock<Variable>>,
    // only for arrays of reference type
    pub(crate) array_element_type: Option<Arc<Class>>,
    pub(in crate::runtime) clinit_call: parking_lot::ReentrantMutex<Cell<ClinitStatus>>,
    // contains all methods inherited from super classes, and default methods from super interfaces
    pub(crate) vtable: Vec<VtableEntry>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum ClinitStatus {
    NotInit,
    Init,
}

#[derive(Debug, Clone)]
pub struct VtableEntry {
    pub(in crate::runtime) root_class: Option<Arc<Class>>,
    pub(in crate::runtime) name: Arc<JavaStr>,
    pub(in crate::runtime) descriptor: MethodDescriptor,
    pub(in crate::runtime) index: VtableIndex,
}

#[derive(Debug, Clone)]
pub enum VtableIndex {
    InThisClass(usize),
    OtherClass { class: Arc<Class>, index: usize },
    OtherInterface { class: Arc<Class>, index: usize },
}

impl Class {
    pub(super) fn resolve_method(
        &self,
        name: &JavaStr,
        param_descriptor: &[FieldType],
    ) -> Option<&MethodInfo> {
        for method_info in &self.methods {
            if method_info.name.as_ref() != name {
                continue;
            }
            if method_info.descriptor.parameters != param_descriptor {
                continue;
            }
            return Some(method_info);
        }
        None
    }
    pub(super) fn get_constant(&self, index: u16) -> &ConstantPoolInfo {
        &self.constant_pool[index as usize - 1]
    }

    pub(super) fn get_static_field(&self, index: usize) -> Variable {
        *self.static_fields[index].read().unwrap()
    }

    pub(super) fn set_static_field(&self, index: usize, value: Variable) {
        *self.static_fields[index].write().unwrap() = value;
    }

    pub(super) fn is_array(&self) -> bool {
        self.class_name.starts_with("[")
    }

    pub(super) fn package_name(&self) -> &str {
        let Some((package, _)) = self.class_name.rsplit_once('/') else {
            return "";
        };
        package
    }
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub(crate) access_flags: FieldAccessFlag,
    pub(crate) name: Arc<JavaStr>,
    pub(crate) descriptor: FieldDescriptor,
    pub(crate) attributes: Vec<AttributeInfo>,
    pub(crate) index: usize,
}

#[derive(Debug)]
pub struct MethodInfo {
    pub(crate) access_flags: MethodAccessFlag,
    pub(crate) name: Arc<JavaStr>,
    pub(crate) descriptor: MethodDescriptor,
    pub(crate) attributes: Vec<AttributeInfo>,
}

#[derive(Debug)]
pub enum Exception {
    VmException {
        exception_type: Arc<Class>,
        message: String,
    },
    UserException(u32),
}

impl Exception {
    pub(crate) fn new_vm(exception_type: &Arc<Class>) -> Self {
        Exception::VmException {
            exception_type: Arc::clone(exception_type),
            message: Default::default(),
        }
    }

    pub(crate) fn new_vm_msg(exception_type: &Arc<Class>, message: &str) -> Self {
        Exception::VmException {
            exception_type: Arc::clone(exception_type),
            message: message.to_string(),
        }
    }

    pub(crate) fn new(exception: u32) -> Self {
        Exception::UserException(exception)
    }
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for Exception {
    fn from(err: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        Exception::VmException {
            exception_type: Arc::clone(CLASS_FORMAT_ERROR_CLASS.get().expect("must init")),
            message: format!("{err:?}"),
        }
    }
}

pub type NativeResult<T> = ::std::result::Result<T, Exception>;
