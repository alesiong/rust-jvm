use std::sync::{Arc, RwLock};

pub use attributes::*;
pub use constant_pool::*;
pub use object::*;

use crate::{
    consts::{ClassAccessFlag, FieldAccessFlag, MethodAccessFlag},
    descriptor::{FieldDescriptor, FieldType, MethodDescriptor},
};
use crate::class::JavaStr;
use crate::runtime::Variable;

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
    pub(crate) fields: Vec<FieldInfo>,
    pub(crate) methods: Vec<MethodInfo>,
    pub(crate) attributes: Vec<AttributeInfo>,
    pub(crate) field_var_size: usize,
    pub(crate) static_fields: RwLock<Vec<Variable>>,
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
}

#[derive(Debug)]
pub struct FieldInfo {
    pub(crate) access_flags: FieldAccessFlag,
    pub(crate) name: Arc<JavaStr>,
    pub(crate) descriptor: FieldDescriptor,
    pub(crate) attributes: Vec<AttributeInfo>,
}

#[derive(Debug)]
pub struct MethodInfo {
    pub(crate) access_flags: MethodAccessFlag,
    pub(crate) name: Arc<JavaStr>,
    pub(crate) descriptor: MethodDescriptor,
    pub(crate) attributes: Vec<AttributeInfo>,
}
