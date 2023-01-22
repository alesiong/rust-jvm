use std::sync::Arc;

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
