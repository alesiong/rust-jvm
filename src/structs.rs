#![allow(unused)] // FIXME:

use std::sync::Arc;

mod attributes;
mod constant_pool;

pub use attributes::*;
pub use constant_pool::*;

use crate::descriptor::{FieldDescriptor, MethodDescriptor};

#[derive(Debug)]
pub struct Class {
    pub(crate) minor_version: u16,
    pub(crate) major_version: u16,
    pub(crate) constant_pool: Vec<ConstantPoolInfo>,
    pub(crate) access_flags: ClassAccessFlag,
    pub(crate) this_class: CpClassInfo,
    pub(crate) super_class: Option<CpClassInfo>,
    pub(crate) interfaces: Vec<u16>,
    pub(crate) fields: Vec<FieldInfo>,
    pub(crate) methods: Vec<MethodInfo>,
    pub(crate) attributes: Vec<AttributeInfo>,
}

impl Class {
    pub(crate) fn resolve_method_constant(&self, index: u16) -> Option<CpMethodrefInfo> {
        if let ConstantPoolInfo::Methodref {
            class_index,
            name_and_type_index,
        } = self.constant_pool[(index - 1) as usize]
        {
            if let (Some(class), Some(name_and_type)) = (
                Self::resolve_class_constant(&self.constant_pool, class_index),
                Self::resolve_name_and_type_constant(&self.constant_pool, name_and_type_index),
            ) {
                return Some(CpMethodrefInfo {
                    class,
                    name_and_type,
                });
            } else {
                return None;
            }
        }
        None
    }

    pub(crate) fn resolve_utf8_constant(
        pool: &[ConstantPoolInfo],
        index: u16,
    ) -> Option<Arc<String>> {
        if let ConstantPoolInfo::Utf8 { ref bytes } = pool[(index - 1) as usize] {
            return Some(Arc::clone(bytes));
        }
        None
    }

    pub(crate) fn resolve_class_constant(
        pool: &[ConstantPoolInfo],
        index: u16,
    ) -> Option<CpClassInfo> {
        if index == 0 {
            return None;
        }
        if let ConstantPoolInfo::Class { name_index } = pool[(index - 1) as usize] {
            return Self::resolve_utf8_constant(pool, name_index).map(|name| CpClassInfo { name });
        }
        None
    }

    pub(crate) fn resolve_name_and_type_constant(
        pool: &[ConstantPoolInfo],
        index: u16,
    ) -> Option<CpNameAndTypeInfo> {
        if let ConstantPoolInfo::NameAndType {
            name_index,
            descriptor_index,
        } = pool[(index - 1) as usize]
        {
            if let (Some(name), Some(descriptor)) = (
                Self::resolve_utf8_constant(pool, name_index),
                Self::resolve_utf8_constant(pool, descriptor_index),
            ) {
                return Some(CpNameAndTypeInfo { name, descriptor });
            } else {
                return None;
            }
        }
        None
    }
}

bitflags::bitflags! {
    pub struct ClassAccessFlag: u16 {
        const PUBLIC = 0x0001;
        const FINAL = 0x0010;
        const SUPER = 0x0020;
        const INTERFACE = 0x0200;
        const ABSTRACT = 0x0400;
        const SYNTHETIC = 0x1000;
        const ANNOTATION = 0x2000;
        const ENUM = 0x4000;
        const MODULE = 0x8000;
    }

    pub struct FieldAccessFlag: u16 {
        const PUBLIC = 0x0001;
        const PRIVATE = 0x0002;
        const PROTECTED	 = 0x0004;
        const STATIC = 0x0008;
        const FINAL = 0x0010;
        const VOLATILE = 0x0040;
        const TRANSIENT = 0x0080;
        const SYNTHETIC = 0x1000;
        const ENUM = 0x4000;
    }
    pub struct MethodAccessFlag: u16 {
        const PUBLIC = 0x0001;
        const PRIVATE = 0x0002;
        const PROTECTED	 = 0x0004;
        const STATIC = 0x0008;
        const FINAL = 0x0010;
        const SYNCHRONIZED = 0x0020;
        const BRIDGE = 0x0040;
        const VARARGS = 0x0080;
        const NATIVE = 0x0100;
        const ABSTRACT = 0x0400;
        const STRICT = 0x0800;
        const SYNTHETIC = 0x1000;
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
