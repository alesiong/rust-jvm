use std::sync::{Arc, Weak};

use crate::{descriptor::FieldType, runtime};

use super::instructions;

pub struct Thread {
    top_frame: Option<Box<Frame>>,
    max_frame_size: usize,
    pc: usize,
}

pub struct Frame {
    class: Arc<runtime::Class>,
    code: Arc<[u8]>,
    locals: Vec<Variable>,
    stack: Vec<Variable>,
    previous_frame: Option<Box<Frame>>,
}

#[derive(Copy, Clone)]
pub union Variable {
    // boolean: bool,
    // byte: i8,
    // char: u16,
    // short: i16,
    pub int: i32,
    float: f32,
    reference: u32,
    return_address: u32,
    void: (),
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
    ) {
        let Some(method_info) = class.resolve_method(method_name, param_descriptor) else {
            panic!("method not found: {}", method_name);
        };
        for attr in &method_info.attributes {
            match attr {
                runtime::AttributeInfo::Code(code) => {
                    let previous_frame = self.top_frame.take();

                    self.top_frame = Some(Box::new(Frame {
                        code: Arc::clone(&code.code),
                        locals: Vec::with_capacity(code.max_locals as _),
                        stack: Vec::with_capacity(code.max_stack as _),
                        class,
                        previous_frame,
                    }));
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

    pub fn execute(&mut self) -> Variable {
        let frame = &mut self.top_frame.as_mut().unwrap();
        loop {
            let op = frame.code[self.pc];
            match op {
                instructions::ALOAD_0 | instructions::ILOAD_0 => {
                    frame.stack.push(frame.locals[0]);
                }
                instructions::ILOAD_1 => {
                    frame.stack.push(frame.locals[1]);
                }
                instructions::IADD => {
                    // SAFETY: rely on class file checking to ensure correct type
                    let a = unsafe { frame.stack.pop().unwrap().int };
                    let b = unsafe { frame.stack.pop().unwrap().int };
                    frame.stack.push(Variable { int: a + b });
                }
                instructions::INVOKESPECIAL => {
                    let indexbyte1 = frame.code[self.pc + 1] as u16;
                    let indexbyte2 = frame.code[self.pc + 2] as u16;
                    self.pc += 2;
                    let index = (indexbyte1 << 8) | indexbyte2;

                    // let method = self.class.resolve_method_constant(index).unwrap();
                    // println!("call method: {:?}", method);
                    // TODO:
                }
                instructions::RETURN => {
                    return Variable { void: () };
                }
                instructions::IRETURN => {
                    return frame.stack.pop().unwrap();
                }
                instructions::NOP => {}
                _ => {
                    // skip unknown instructions
                    eprintln!("unknown instruction: {}", op);
                }
            }

            self.pc += 1;
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
}
