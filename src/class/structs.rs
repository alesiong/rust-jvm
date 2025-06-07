#![allow(unused)] // FIXME:

use std::sync::Arc;

mod constant_pool;

pub use constant_pool::*;

use crate::{descriptor::{FieldDescriptor, MethodDescriptor}, consts::{FieldAccessFlag, MethodAccessFlag, ClassAccessFlag}};

#[derive(Debug)]
pub struct Class {
    pub(crate) minor_version: u16,
    pub(crate) major_version: u16,
    pub(crate) constant_pool: Vec<ConstantPoolInfo>,
    pub(crate) access_flags: ClassAccessFlag,
    pub(crate) this_class: u16,
    pub(crate) super_class: u16,
    pub(crate) interfaces: Vec<u16>,
    pub(crate) fields: Vec<FieldInfo>,
    pub(crate) methods: Vec<MethodInfo>,
    pub(crate) attributes: Vec<AttributeInfo>,
}

#[derive(Debug)]
pub struct FieldInfo {
    pub(crate) access_flags: FieldAccessFlag,
    pub(crate) name_index: u16,
    pub(crate) descriptor_index: u16,
    pub(crate) attributes: Vec<AttributeInfo>,
}

#[derive(Debug)]
pub struct MethodInfo {
    pub(crate) access_flags: MethodAccessFlag,
    pub(crate) name_index: u16,
    pub(crate) descriptor_index: u16,
    pub(crate) attributes: Vec<AttributeInfo>,
}

#[derive(Debug)]
pub struct AttributeInfo {
    pub(crate) attribute_name_index: u16,
    pub(crate) info: Vec<u8>,
}