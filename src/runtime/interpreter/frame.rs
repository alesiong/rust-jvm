use std::fmt::{Debug, Formatter};
use std::sync::{Arc, RwLock};

use crate::descriptor::ReturnType;
use crate::runtime::interpreter::{global, InterpreterEnv};
use crate::runtime::Heap;
use crate::{descriptor::FieldType, runtime};

use super::Next;

pub struct Thread {
    top_frame: Option<Box<Frame>>,
    max_frame_size: usize,
    pc: usize,
}

pub struct Frame {
    pub(super) class: Arc<runtime::Class>,
    pub(super) code: Arc<[u8]>,
    pub(super) return_type: ReturnType,
    pub(super) locals: Vec<Variable>,
    pub(super) stack: Vec<Variable>,
    pub(super) previous_frame: Option<Box<Frame>>,
}

#[derive(Copy, Clone)]
pub union Variable {
    // boolean: bool,
    // byte: i8,
    // char: u16,
    // short: i16,
    pub(super) int: i32,
    pub(super) float: f32,
    pub(super) reference: u32,
    pub(super) return_address: u32,
    pub(super) void: (),
}

impl Debug for Variable {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Variable")
    }
}

impl Variable {
    /// # Safety
    ///
    /// should ensure the underlying type is int
    pub unsafe fn get_int(self) -> i32 {
        self.int
    }
}

impl Thread {
    pub fn new(max_frame_size: usize) -> Thread {
        Thread {
            top_frame: None,
            max_frame_size,
            pc: 0,
        }
    }
    pub fn new_frame(
        &mut self,
        class: Arc<runtime::Class>,
        method_name: &str,
        param_descriptor: &[FieldType],
        return_address: usize,
    ) {
        Self::new_frame_inner(
            &mut self.top_frame,
            class,
            method_name,
            param_descriptor,
            return_address,
            false,
        )
    }

    fn new_frame_inner(
        top_frame: &mut Option<Box<Frame>>,
        class: Arc<runtime::Class>,
        method_name: &str,
        param_descriptor: &[FieldType],
        return_address: usize,
        need_this: bool,
    ) {
        let Some(method_info) = class.resolve_method(method_name, param_descriptor) else {
            panic!("method not found: {}", method_name);
        };
        for attr in &method_info.attributes {
            match attr {
                runtime::AttributeInfo::Code(code) => {
                    let mut previous_frame = top_frame.take();
                    let mut locals = Vec::with_capacity(code.max_locals as _);
                    if let Some(previous_frame) = previous_frame.as_mut() {
                        let mut param_size = 0;
                        for param in &method_info.descriptor.parameters {
                            match param {
                                FieldType::Long | FieldType::Double => {
                                    param_size += 2;
                                }
                                _ => {
                                    param_size += 1;
                                }
                            }
                        }
                        if need_this {
                            param_size += 1;
                        }
                        for v in previous_frame
                            .stack
                            .drain((previous_frame.stack.len() - param_size)..)
                        {
                            locals.push(v);
                        }
                    }

                    let mut frame = Frame {
                        code: Arc::clone(&code.code),
                        locals,
                        stack: Vec::with_capacity(code.max_stack as usize + 2),
                        return_type: method_info.descriptor.return_type.clone(),
                        class,
                        previous_frame,
                    };

                    // return address
                    let lower = return_address as u32;
                    let upper = (return_address >> 32) as u32;
                    frame.stack.push(Variable {
                        return_address: upper,
                    });
                    frame.stack.push(Variable {
                        return_address: lower,
                    });

                    *top_frame = Some(Box::new(frame));
                    return;
                }
                _ => continue,
            }
        }
        panic!("method code attributes not found: {}", method_name);
    }

    pub fn top_frame(&mut self) -> Option<&mut Frame> {
        self.top_frame.as_deref_mut()
    }

    pub fn execute(&mut self) {
        let mut_pc = &mut self.pc;
        while let Some(mut frame) = self.top_frame.take() {
            let mut env = InterpreterEnv::new(mut_pc, &mut frame, &global::HEAP);
            let next = env.execute();

            match next {
                Next::Return { return_pc, v1, v2 } => {
                    let (is_void, is_long) = match frame.return_type {
                        Some(FieldType::Long | FieldType::Double) => (false, true),
                        Some(_) => (false, false),
                        None => (true, false),
                    };
                    self.top_frame = frame.previous_frame;
                    *mut_pc = return_pc;
                    if let Some(ref mut frame) = self.top_frame {
                        if !is_void {
                            frame.stack.push(v1);
                            if is_long {
                                frame.stack.push(v2);
                            }
                        }
                    }
                }
                Next::InvokeSpecial {
                    class,
                    name_and_type,
                } => {
                    // TODO: resolve method
                    let class = Arc::clone(&frame.class);
                    self.top_frame = Some(frame);
                    Self::new_frame_inner(
                        &mut self.top_frame,
                        // TODO: only current class
                        class,
                        &name_and_type.name,
                        &name_and_type.descriptor.parameters,
                        *mut_pc + 1,
                        true,
                    );
                    *mut_pc = 0;
                }
                Next::InvokeStatic {
                    class,
                    name_and_type,
                } => {
                    // TODO: resolve method
                    let class = Arc::clone(&frame.class);
                    self.top_frame = Some(frame);
                    Self::new_frame_inner(
                        &mut self.top_frame,
                        // TODO: only current class
                        class,
                        &name_and_type.name,
                        &name_and_type.descriptor.parameters,
                        *mut_pc + 1,
                        false,
                    );
                    *mut_pc = 0;
                }
            }
        }
    }
}

impl Frame {
    pub fn add_local_int(&mut self, int: i32) {
        self.locals.push(Variable { int });
    }

    pub fn add_local_long(&mut self, long: i64) {
        let lower = long as i32;
        let upper = (long >> 32) as i32;

        self.locals.reserve(2);
        self.locals.push(Variable { int: upper });
        self.locals.push(Variable { int: lower });
    }

    pub fn add_local_reference(&mut self, reference: u32) {
        self.locals.push(Variable { reference });
    }
}
