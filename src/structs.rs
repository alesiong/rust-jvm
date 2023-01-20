#![allow(unused)] // FIXME:

use std::sync::Arc;

#[derive(Debug)]
pub struct Class {
    pub(crate) minor_version: u16,
    pub(crate) major_version: u16,
    pub(crate) constant_pool: Vec<ConstantPoolInfo>,
    pub(crate) access_flags: ClassAccessFlag,
    pub(crate) this_class: CpClassInfo,
    pub(crate) super_class: CpClassInfo,
    pub(crate) interfaces: Vec<u16>,
    pub(crate) fields: Vec<FieldInfo>,
    pub(crate) methods: Vec<MethodInfo>,
    pub(crate) attributes: Vec<AttributeInfo>,
}

impl Class {
    fn resolve_constant(&self, index: u16) -> Option<Arc<String>> {
        Self::resolve_utf8_constant(&self.constant_pool, index)
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
        if let ConstantPoolInfo::Class { name_index } = pool[(index - 1) as usize] {
            return Self::resolve_utf8_constant(pool, name_index).map(|name| CpClassInfo { name });
        }
        None
    }
}

#[derive(Debug)]
pub enum ConstantPoolInfo {
    Utf8 {
        bytes: Arc<String>,
    },
    Integer,
    Float,
    Long,
    Double,
    Class {
        name_index: u16,
    },
    String {
        string_index: u16,
    },
    Fieldref {
        class_index: u16,
        name_and_type_index: u16,
    },
    Methodref {
        class_index: u16,
        name_and_type_index: u16,
    },
    InterfaceMethodref {
        class_index: u16,
        name_and_type_index: u16,
    },
    NameAndType {
        name_index: u16,
        descriptor_index: u16,
    },
    MethodHandle,
    MethodType,
    Dynamic,
    InvokeDynamic,
    Module,
    Package,
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
    pub(crate) descriptor: Arc<String>,
    pub(crate) attributes: Vec<AttributeInfo>,
}

#[derive(Debug)]
pub enum AttributeInfo {
    Code(CodeAttribute),
    SourceFile { sourcefile: Arc<String> },
    LineNumberTable(Vec<LineNumberTableItem>),
    Unknown(Arc<String>, Vec<u8>),
}

#[derive(Debug)]
pub struct CodeAttribute {
    pub(crate) max_stack: u16,
    pub(crate) max_locals: u16,
    pub(crate) code: Vec<u8>,
    pub(crate) exception_table: Vec<ExceptionTableItem>,
    pub(crate) attributes: Vec<AttributeInfo>,
}

#[derive(Debug)]
pub struct LineNumberTableItem {
    pub(crate) start_pc: u16,
    pub(crate) line_number: u16,
}

#[derive(Debug)]
pub struct ExceptionTableItem {
    pub(crate) start_pc: u16,
    pub(crate) end_pc: u16,
    pub(crate) handler_pc: u16,
    pub(crate) catch_type: u16,
}

#[derive(Debug)]
pub struct MethodInfo {
    pub(crate) access_flags: MethodAccessFlag,
    pub(crate) name: Arc<String>,
    pub(crate) descriptor: Arc<String>,
    pub(crate) attributes: Vec<AttributeInfo>,
}

#[derive(Debug)]
pub struct CpClassInfo {
    pub(crate) name: Arc<String>,
}
