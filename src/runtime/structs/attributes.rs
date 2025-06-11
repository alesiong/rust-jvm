use super::CpClassInfo;
use crate::class::JavaStr;
use crate::descriptor::{FieldDescriptor, ReturnType};
use std::sync::Arc;

#[derive(Debug)]
pub enum AttributeInfo {
    Code(CodeAttribute),
    SourceFile(Arc<JavaStr>),
    LineNumberTable(Vec<LineNumberTableItem>),
    ConstantValue(Const),
    RuntimeVisibleAnnotations(Vec<Annotation>),
    LocalVariableTable(Vec<LocalVariable>),
    StackMapTable(Vec<StackMapFrame>),
    Deprecated,
    Signature(Arc<JavaStr>),
    Exceptions,
    Module(Module),
    ModulePackages(Vec<Arc<JavaStr>>),
    ModuleHashes,
    ModuleTarget(Arc<JavaStr>),
    InnerClasses,
    Unknown(Arc<JavaStr>),
}

#[derive(Debug)]
pub struct CodeAttribute {
    pub(crate) max_stack: u16,
    pub(crate) max_locals: u16,
    pub(crate) code: Arc<[u8]>,
    pub(crate) exception_table: Vec<ExceptionTableItem>,
    pub(crate) attributes: Vec<AttributeInfo>,
}

#[derive(Debug)]
pub struct Annotation {
    pub(crate) type_descriptor: FieldDescriptor,
    pub(crate) element_value_pairs: Vec<ElementValuePair>,
}

#[derive(Debug)]
pub struct ElementValuePair {
    pub(crate) element_name: Arc<JavaStr>,
    pub(crate) value: ElementValue,
}

#[derive(Debug)]
pub enum ElementValue {
    Const(Const),
    Enum {
        type_name: Arc<JavaStr>,
        const_name: Arc<JavaStr>,
    },
    Class(ReturnType),
    Annotation(Annotation),
    Array(Vec<ElementValue>),
}

#[derive(Debug)]
pub enum Const {
    Byte(i32),
    Char(i32),
    Double(f64),
    Float(f32),
    Int(i32),
    Long(i64),
    Short(i32),
    Boolean(i32),
    String(Arc<JavaStr>),
}

impl Const {
    pub(crate) fn into_byte(self) -> Self {
        let Const::Int(v) = self else {
            panic!("not int type");
        };
        Self::Byte(v)
    }
    pub(crate) fn into_char(self) -> Self {
        let Const::Int(v) = self else {
            panic!("not int type");
        };
        Self::Char(v)
    }
    pub(crate) fn into_short(self) -> Self {
        let Const::Int(v) = self else {
            panic!("not int type");
        };
        Self::Short(v)
    }
    pub(crate) fn into_boolean(self) -> Self {
        let Const::Int(v) = self else {
            panic!("not int type");
        };
        Self::Boolean(v)
    }
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
    pub(crate) catch_type: Option<CpClassInfo>,
}

#[derive(Debug)]
pub struct LocalVariable {
    pub(crate) start_pc: u16,
    pub(crate) length: u16,
    pub(crate) name: Arc<JavaStr>,
    pub(crate) descriptor: FieldDescriptor,
    pub(crate) index: u16,
}

#[derive(Debug)]
pub struct StackMapFrame {
    // TODO:
}

#[derive(Debug)]
pub struct Module {
    pub(crate) exports: Vec<ModuleExport>,
}

#[derive(Debug)]
pub struct ModuleExport {
    pub(crate) exports: Arc<JavaStr>,
    pub(crate) exports_flags: u16,
    pub(crate) exports_to: Vec<Arc<JavaStr>>,
}
