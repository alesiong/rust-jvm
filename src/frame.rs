use crate::{
    instructions,
    structs::{AttributeInfo, Class},
};

pub struct Frame<'a> {
    class: &'a Class,
    ip: usize,
    code: &'a [u8],
    locals: Vec<Variable>,
    stack: Vec<Variable>,
}

#[derive(Copy, Clone)]
pub union Variable {
    boolean: bool,
    byte: i8,
    char: u16,
    short: i16,
    pub int: i32,
    float: f32,
    reference: u32,
    return_address: u32,
    void: (),
}

impl<'a> Frame<'a> {
    pub fn new(class: &'a Class, method: &str) -> Self {
        for method_info in &class.methods {
            if method_info.name.as_str() != method {
                continue;
            }
            for attr in &method_info.attributes {
                match attr {
                    AttributeInfo::Code(code) => {
                        return Frame {
                            class,
                            ip: 0,
                            code: &code.code,
                            locals: Vec::new(),
                            stack: Vec::new(),
                        };
                    }
                    _ => continue,
                }
            }
        }
        panic!("method not found: {}", method)
    }

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

    pub fn execute(&mut self) -> Variable {
        loop {
            let op = self.code[self.ip];
            match op {
                instructions::ALOAD_0 | instructions::ILOAD_0 => {
                    self.stack.push(self.locals[0]);
                }
                instructions::ILOAD_1 => {
                    self.stack.push(self.locals[1]);
                }
                instructions::IADD => {
                    // SAFETY: rely on class file checking to ensure correct type
                    let a = unsafe { self.stack.pop().unwrap().int };
                    let b = unsafe { self.stack.pop().unwrap().int };
                    self.stack.push(Variable { int: a + b });
                }
                instructions::INVOKESPECIAL => {
                    let indexbyte1 = self.code[self.ip + 1] as u16;
                    let indexbyte2 = self.code[self.ip + 2] as u16;
                    self.ip += 2;
                    let index = (indexbyte1 << 8) | indexbyte2;

                    let method = self.class.resolve_method_constant(index).unwrap();
                    println!("call method: {:?}", method);
                    // TODO:
                }
                instructions::RETURN => {
                    return Variable { void: () };
                }
                instructions::IRETURN => {
                    return self.stack.pop().unwrap();
                }

                _ => {
                    // skip unknown instructions
                    eprintln!("unknown instruction: {}", op);
                }
            }

            self.ip += 1;
        }
    }
}
