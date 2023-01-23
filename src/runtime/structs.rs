use std::sync::Arc;

pub use attributes::*;
pub use constant_pool::*;

use crate::{
    consts::{ClassAccessFlag, FieldAccessFlag, MethodAccessFlag},
    descriptor::{FieldDescriptor, FieldType, MethodDescriptor},
};

mod attributes;
mod constant_pool;

#[derive(Debug)]
pub struct Class {
    pub(crate) constant_pool: Vec<ConstantPoolInfo>,
    pub(crate) access_flags: ClassAccessFlag,
    pub(crate) class_name: Arc<String>,
    pub(crate) super_class: Option<Arc<Class>>,
    pub(crate) interfaces: Vec<Arc<Class>>,
    pub(crate) fields: Vec<FieldInfo>,
    pub(crate) methods: Vec<MethodInfo>,
    pub(crate) attributes: Vec<AttributeInfo>,
}

impl Class {
    pub(super) fn resolve_method(&self, name: &str, param_descriptor: &[FieldType]) -> Option<&MethodInfo> {
        for method_info in &self.methods {
            if method_info.name.as_str() != name {
                continue;
            }
            if method_info.descriptor.parameters != param_descriptor {
                continue;
            }
            return Some(method_info);
        }
        None
    }
}

#[derive(Debug)]
pub struct FieldInfo {
    pub(crate) access_flags: FieldAccessFlag,
    pub(crate) name: Arc<String>,
    pub(crate) descriptor: FieldDescriptor,
    pub(crate) attributes: Vec<AttributeInfo>,
}

#[derive(Debug)]
pub struct MethodInfo {
    pub(crate) access_flags: MethodAccessFlag,
    pub(crate) name: Arc<String>,
    pub(crate) descriptor: MethodDescriptor,
    pub(crate) attributes: Vec<AttributeInfo>,
}
