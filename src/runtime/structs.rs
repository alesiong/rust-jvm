use std::cell::Cell;
use std::sync::{Arc, RwLock};

pub use attributes::*;
pub use constant_pool::*;
pub use object::*;
pub use string_table::*;

use crate::class::JavaStr;
use crate::runtime::Variable;
use crate::{
    consts::{ClassAccessFlag, FieldAccessFlag, MethodAccessFlag},
    descriptor::{FieldDescriptor, FieldType, MethodDescriptor},
};

mod attributes;
mod constant_pool;
mod object;
mod string_table;

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
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum ClinitStatus {
    NotInit,
    Init,
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

    pub(super) fn get_static_field(&self, index: u16) -> Variable {
        *self.static_fields[index as usize].read().unwrap()
    }

    pub(super) fn set_static_field(&self, index: u16, value: Variable) {
        *self.static_fields[index as usize].write().unwrap() = value;
    }

    pub(super) fn is_array(&self) -> bool {
        self.class_name.starts_with("[")
    }
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub(crate) access_flags: FieldAccessFlag,
    pub(crate) name: Arc<JavaStr>,
    pub(crate) descriptor: FieldDescriptor,
    pub(crate) attributes: Vec<AttributeInfo>,
    pub(crate) index: u16,
}

#[derive(Debug)]
pub struct MethodInfo {
    pub(crate) access_flags: MethodAccessFlag,
    pub(crate) name: Arc<JavaStr>,
    pub(crate) descriptor: MethodDescriptor,
    pub(crate) attributes: Vec<AttributeInfo>,
}

#[derive(Debug)]
pub struct Exception {
    exception_type: String,
    message: String,
}

impl Exception {
    pub(crate) fn new(exception_type: &str) -> Self {
        Self {
            exception_type: exception_type.to_string(),
            message: Default::default(),
        }
    }
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for Exception {
    fn from(err: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        Self {
            exception_type: "java/lang/ClassFormatError".to_string(),
            message: format!("{:?}", err),
        }
    }
}

pub type NativeResult<T> = ::std::result::Result<T, Exception>;
