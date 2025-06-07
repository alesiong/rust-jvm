mod frame;
pub(crate) mod global;
mod instructions;

pub use frame::*;
use std::sync::{Arc, RwLock};

use super::{Class, CpClassInfo, CpNameAndTypeInfo};
use crate::consts::FieldAccessFlag;
use crate::descriptor::{self, FieldType, MethodDescriptor};
use crate::runtime::global::BOOTSTRAP_CLASS_LOADER;
use crate::runtime::Heap;
use crate::runtime::{self};

pub(self) struct InterpreterEnv<'t: 'f, 'f> {
    pc: &'t mut usize,
    frame: &'f mut Frame,
    heap: &'static RwLock<Heap>,
}

pub(self) enum Next {
    Return {
        v1: Variable,
        v2: Variable,
        return_pc: usize,
    },
    InvokeSpecial {
        class: Arc<Class>,
        name_and_type: CpNameAndTypeInfo<MethodDescriptor>,
    },
    InvokeStatic {
        class: Arc<Class>,
        name_and_type: CpNameAndTypeInfo<MethodDescriptor>,
    },
}

impl<'t, 'f> InterpreterEnv<'t, 'f> {
    pub fn new(pc: &'t mut usize, frame: &'f mut Frame, heap: &'static RwLock<Heap>) -> Self {
        Self { pc, frame, heap }
    }

    fn execute(&mut self) -> Next {
        use instructions as inst;
        loop {
            let op = self.frame.code[*self.pc];
            match op {
                // load
                inst::ALOAD_0 | inst::ILOAD_0 | inst::FLOAD_0 => {
                    self.load_n(0);
                }
                inst::LLOAD_0 | inst::DLOAD_0 => {
                    self.load_n_long(0);
                }
                inst::ALOAD_1 | inst::ILOAD_1 | inst::FLOAD_1 => {
                    self.load_n(1);
                }
                inst::LLOAD_1 | inst::DLOAD_1 => {
                    self.load_n_long(1);
                }
                inst::ALOAD_2 | inst::ILOAD_2 | inst::FLOAD_2 => {
                    self.load_n(2);
                }
                inst::LLOAD_2 | inst::DLOAD_2 => {
                    self.load_n_long(2);
                }
                inst::ALOAD_3 | inst::ILOAD_3 | inst::FLOAD_3 => {
                    self.load_n(3);
                }
                inst::LLOAD_3 | inst::DLOAD_3 => {
                    self.load_n_long(3);
                }
                inst::AALOAD => {
                    // SAFETY: rely on class file checking to ensure correct type
                    let index = unsafe { self.frame.stack.pop().unwrap().get_int() };
                    let arr = unsafe { self.frame.stack.pop().unwrap().reference };
                    // TODO: get arr[index]
                    if arr == 0 {
                        // TODO: exception
                        panic!("NullPointerException");
                    }
                    // self.frame.stack.push(value);
                }
                inst::ALOAD | inst::ILOAD | inst::FLOAD => {
                    let index = self.get_u8_args();
                    self.frame.stack.push(self.frame.locals[index as usize]);
                }

                // store
                inst::ASTORE_0 | inst::ISTORE_0 | inst::FSTORE_0 => {
                    self.store_n(0);
                }
                inst::LSTORE_0 | inst::DSTORE_0 => {
                    self.store_n_long(0);
                }
                inst::ASTORE_1 | inst::ISTORE_1 | inst::FSTORE_1 => {
                    self.store_n(1);
                }
                inst::LSTORE_1 | inst::DSTORE_1 => {
                    self.store_n_long(1);
                }
                inst::ASTORE_2 | inst::ISTORE_2 | inst::FSTORE_2 => {
                    self.store_n(2);
                }
                inst::LSTORE_2 | inst::DSTORE_2 => {
                    self.store_n_long(2);
                }
                inst::ASTORE_3 | inst::ISTORE_3 | inst::FSTORE_3 => {
                    self.store_n(3);
                }
                inst::LSTORE_3 | inst::DSTORE_3 => {
                    self.store_n_long(3);
                }
                inst::AASTORE => {
                    // SAFETY: rely on class file checking to ensure correct type
                    let value = unsafe { self.frame.stack.pop().unwrap().reference };
                    let index = unsafe { self.frame.stack.pop().unwrap().get_int() };
                    let arr = unsafe { self.frame.stack.pop().unwrap().reference };
                    // TODO: arr[index] = value
                    if arr == 0 {
                        // TODO: exception
                        panic!("NullPointerException");
                    }
                }
                inst::ASTORE | inst::ISTORE | inst::FSTORE => {
                    let index = self.get_u8_args();
                    self.frame.locals[index as usize] = self.frame.stack.pop().unwrap();
                }

                // const
                inst::ICONST_M1 => {
                    self.iconst(-1);
                }
                inst::ICONST_0 => {
                    self.iconst(0);
                }
                inst::ICONST_1 => {
                    self.iconst(1);
                }
                inst::ICONST_2 => {
                    self.iconst(2);
                }
                inst::ICONST_3 => {
                    self.iconst(3);
                }
                inst::ICONST_4 => {
                    self.iconst(4);
                }
                inst::ICONST_5 => {
                    self.iconst(5);
                }
                inst::LCONST_0 => {
                    self.iconst(0);
                    self.iconst(0);
                }
                inst::LCONST_1 => {
                    self.iconst(0);
                    self.iconst(1);
                }
                inst::ACONST_NULL => {
                    self.frame.stack.push(Variable { reference: 0 });
                }

                inst::BIPUSH => {
                    let byte = self.get_i8_args();
                    self.frame.stack.push(Variable { int: byte as i32 });
                }

                inst::LDC => {
                    let index = self.get_u8_args() as u16;
                    self.ldc(index);
                }
                inst::LDC_W => {
                    let index = self.get_u16_args();
                    self.ldc(index);
                }
                inst::LDC2_W => {
                    let index = self.get_u16_args();
                    self.ldc2(index);
                }

                // stacks
                inst::DUP => {
                    self.frame.stack.push(
                        *self
                            .frame
                            .stack
                            .last()
                            .expect("stack must not be empty when dup"),
                    );
                }
                inst::POP => {
                    self.frame.stack.truncate(self.frame.stack.len() - 1);
                }
                inst::POP2 => {
                    self.frame.stack.truncate(self.frame.stack.len() - 2);
                }

                // arithmetic
                inst::IADD => {
                    // SAFETY: rely on class file checking to ensure correct type
                    let a = unsafe { self.frame.stack.pop().unwrap().int };
                    let b = unsafe { self.frame.stack.pop().unwrap().int };
                    self.frame.stack.push(Variable {
                        int: a.wrapping_add(b),
                    });
                }
                inst::LADD => {
                    let a = self.pop_long();
                    let b = self.pop_long();
                    self.push_long(a.wrapping_add(b));
                }
                inst::I2L => {
                    // SAFETY: rely on class file checking to ensure correct type
                    let v = unsafe { self.frame.stack.pop().unwrap().int };
                    self.frame.stack.push(Variable {
                        int: if v.is_negative() { -1 } else { 0 },
                    });
                    self.frame.stack.push(Variable { int: v });
                }

                // branch
                inst::IF_ACMPEQ => {
                    let offset = self.get_i16_args();
                    // SAFETY: rely on class file checking to ensure correct type
                    let a = unsafe { self.frame.stack.pop().unwrap().reference };
                    let b = unsafe { self.frame.stack.pop().unwrap().reference };
                    if a == b {
                        *self.pc = self.pc.wrapping_add_signed((offset - 2) as isize);
                        continue;
                    }
                }
                inst::IF_ACMPNE => {
                    let offset = self.get_i16_args();
                    // SAFETY: rely on class file checking to ensure correct type
                    let a = unsafe { self.frame.stack.pop().unwrap().reference };
                    let b = unsafe { self.frame.stack.pop().unwrap().reference };
                    if a != b {
                        *self.pc = self.pc.wrapping_add_signed((offset - 2) as isize);
                        continue;
                    }
                }
                inst::GOTO => {
                    let offset = self.get_i16_args();
                    *self.pc = self.pc.wrapping_add_signed((offset - 2) as isize);
                    continue;
                }

                // oop
                inst::NEW => {
                    self.new_object();
                }
                inst::PUTFIELD => {
                    self.put_field();
                }
                inst::GETFIELD => {
                    self.get_field();
                }

                // call
                // TODO: invokevirtual should lookup vtable
                inst::INVOKESPECIAL | inst::INVOKEVIRTUAL => {
                    let cp_index = self.get_u16_args();
                    let runtime::ConstantPoolInfo::Methodref {
                        class,
                        name_and_type,
                    } = self.frame.class.get_constant(cp_index)
                    else {
                        panic!("invalid constant type {}", cp_index);
                    };
                    let class_to_invoke = self.resolve_class(&class.name, &class.class);
                    return Next::InvokeSpecial {
                        class: class_to_invoke,
                        name_and_type: name_and_type.clone(),
                    };
                }
                inst::INVOKESTATIC => {
                    let cp_index = self.get_u16_args();
                    let runtime::ConstantPoolInfo::Methodref {
                        class,
                        name_and_type,
                    } = self.frame.class.get_constant(cp_index)
                    else {
                        panic!("invalid constant type {}", cp_index);
                    };
                    let class_to_invoke = self.resolve_class(&class.name, &class.class);

                    return Next::InvokeStatic {
                        class: class_to_invoke,
                        name_and_type: name_and_type.clone(),
                    };
                }

                // return
                inst::RETURN => {
                    return Next::Return {
                        return_pc: self.pop_return_addr(),
                        v1: Variable { void: () },
                        v2: Variable { void: () },
                    };
                }
                inst::IRETURN | inst::ARETURN | inst::FRETURN => {
                    return Next::Return {
                        v1: self.frame.stack.pop().unwrap(),
                        v2: Variable { void: () },
                        return_pc: self.pop_return_addr(),
                    };
                }
                inst::LRETURN => {
                    return Next::Return {
                        v2: self.frame.stack.pop().unwrap(),
                        v1: self.frame.stack.pop().unwrap(),
                        return_pc: self.pop_return_addr(),
                    };
                }
                inst::NOP => {}
                _ => {
                    // skip unknown instructions
                    eprintln!("unknown instruction: {}", op);
                }
            }

            *self.pc += 1;
        }
    }

    #[inline]
    fn load_n(&mut self, n: usize) {
        self.frame.stack.push(self.frame.locals[n]);
    }

    #[inline]
    fn load_n_long(&mut self, n: usize) {
        self.frame.stack.push(self.frame.locals[n]);
        self.frame.stack.push(self.frame.locals[n + 1]);
    }

    #[inline]
    fn store_n(&mut self, n: usize) {
        let v = self.frame.stack.pop().unwrap();

        if self.frame.locals.len() < n + 1 {
            self.frame.locals.resize(n + 1, Variable { void: () });
        }
        self.frame.locals[n] = v;
    }

    #[inline]
    fn store_n_long(&mut self, n: usize) {
        let v2 = self.frame.stack.pop().unwrap();
        let v1 = self.frame.stack.pop().unwrap();
        if self.frame.locals.len() < n + 2 {
            self.frame.locals.resize(n + 2, Variable { void: () });
        }
        self.frame.locals[n] = v1;
        self.frame.locals[n + 1] = v2;
    }

    #[inline]
    fn iconst(&mut self, i: i32) {
        self.frame.stack.push(Variable { int: i });
    }

    #[inline]
    fn fconst(&mut self, f: f32) {
        self.frame.stack.push(Variable { float: f });
    }

    #[inline]
    fn get_u8_args(&mut self) -> u8 {
        let byte = self.frame.code[*self.pc + 1];
        *self.pc += 1;
        byte
    }
    #[inline]
    fn get_i8_args(&mut self) -> i8 {
        let byte = self.frame.code[*self.pc + 1] as i8;
        *self.pc += 1;
        byte
    }

    #[inline]
    fn get_u16_args(&mut self) -> u16 {
        let byte1 = self.frame.code[*self.pc + 1] as u16;
        let byte2 = self.frame.code[*self.pc + 2] as u16;
        *self.pc += 2;
        (byte1 << 8) | byte2
    }

    #[inline]
    fn get_i16_args(&mut self) -> i16 {
        self.get_u16_args() as _
    }

    #[inline]
    fn pop_return_addr(&mut self) -> usize {
        let lower = unsafe { self.frame.stack.pop().unwrap().return_address } as usize;
        let upper = unsafe { self.frame.stack.pop().unwrap().return_address } as usize;

        (upper << 32) | lower
    }

    #[inline]
    fn pop_long(&mut self) -> i64 {
        let lower = unsafe { self.frame.stack.pop().unwrap().int } as i64;
        let upper = unsafe { self.frame.stack.pop().unwrap().int } as i64;

        (upper << 32) | lower
    }

    #[inline]
    fn push_long(&mut self, l: i64) {
        let lower = l as i32;
        let upper = (l >> 32) as i32;
        self.iconst(upper);
        self.iconst(lower);
    }

    fn new_object(&mut self) {
        let cp_index = self.get_u16_args();
        let runtime::ConstantPoolInfo::Class(CpClassInfo { name, class }) =
            self.frame.class.get_constant(cp_index)
        else {
            panic!("invalid constant type {}", cp_index);
        };
        let new_class = self.resolve_class(name, class);

        let mut fields_types = Vec::new();

        for f in &new_class.fields {
            if f.access_flags.contains(FieldAccessFlag::STATIC) {
                continue;
            }
            if f.descriptor.0.is_long() {
                fields_types.push(&f.descriptor);
            }
            fields_types.push(&f.descriptor);
        }

        let mut heap = self.heap.write().unwrap();
        let id = unsafe {
            heap.allocate(new_class.field_var_size, Arc::clone(&new_class), |i, v| {
                use FieldType::*;
                let var = match fields_types[i].0 {
                    Byte | Char | Int | Short | Boolean | Long => Variable { int: 0 },
                    Float | Double => Variable { float: 0.0 },
                    Object(_) | Array(_) => Variable { reference: 0 },
                };

                v.write(var);
            })
        };
        self.frame.stack.push(Variable { reference: id });
    }

    fn put_field(&mut self) {
        let cp_index = self.get_u16_args();
        let runtime::ConstantPoolInfo::Fieldref {
            class,
            name_and_type,
            field_index,
        } = self.frame.class.get_constant(cp_index)
        else {
            panic!("invalid constant type {}", cp_index);
        };
        // TODO: resolve other classes
        let runtime::FieldIndex::Instance(index) = field_index else {
            panic!("invalid field type");
        };
        let v1;
        let mut v2 = None;
        match name_and_type.descriptor.0 {
            descriptor::FieldType::Long | descriptor::FieldType::Double => {
                v2 = Some(self.frame.stack.pop().unwrap());
                v1 = self.frame.stack.pop().unwrap();
            }
            _ => {
                v1 = self.frame.stack.pop().unwrap();
            }
        }
        let this = unsafe { self.frame.stack.pop().unwrap().reference };
        if this == 0 {
            // TODO: NPE
        }
        let this_obj = self.heap.read().unwrap().get(this);
        unsafe {
            this_obj.put_field(*index as usize, v1);
            if let Some(v2) = v2 {
                this_obj.put_field((*index + 1) as usize, v2);
            }
        }
    }

    fn get_field(&mut self) {
        let cp_index = self.get_u16_args();
        let runtime::ConstantPoolInfo::Fieldref {
            class,
            name_and_type,
            field_index,
        } = self.frame.class.get_constant(cp_index)
        else {
            panic!("invalid constant type {}", cp_index);
        };
        // TODO: resolve other classes
        let runtime::FieldIndex::Instance(index) = field_index else {
            panic!("invalid field type");
        };
        let this = unsafe { self.frame.stack.pop().unwrap().reference };
        if this == 0 {
            // TODO: NPE
        }
        let this_obj = self.heap.read().unwrap().get(this);
        self.frame
            .stack
            .push(unsafe { this_obj.get_field(*index as usize) });

        match name_and_type.descriptor.0 {
            descriptor::FieldType::Long | descriptor::FieldType::Double => {
                self.frame
                    .stack
                    .push(unsafe { this_obj.get_field((*index + 1) as usize) });
            }
            _ => {}
        }
    }

    fn ldc(&mut self, index: u16) {
        match self.frame.class.get_constant(index) {
            runtime::ConstantPoolInfo::Integer(i) => self.iconst(*i),
            runtime::ConstantPoolInfo::Float(f) => self.fconst(*f),
            runtime::ConstantPoolInfo::String(_) => todo!(),
            runtime::ConstantPoolInfo::Class { .. } => todo!(),
            runtime::ConstantPoolInfo::MethodHandle => todo!(),
            runtime::ConstantPoolInfo::MethodType => todo!(),
            runtime::ConstantPoolInfo::Dynamic => todo!(),
            _ => {
                panic!("ldc error, invalid constant type");
            }
        }
    }
    fn ldc2(&mut self, index: u16) {
        match self.frame.class.get_constant(index) {
            runtime::ConstantPoolInfo::Long(l) => {
                self.push_long(*l);
            }
            runtime::ConstantPoolInfo::Double(d) => todo!(),
            _ => {
                panic!("ldc2 error, invalid constant type");
            }
        }
    }

    fn resolve_class(&self, name: &Arc<str>, class: &RwLock<Option<Arc<Class>>>) -> Arc<Class> {
        let class_read = class.read().unwrap();
        let new_class;
        if name == &self.frame.class.class_name {
            new_class = Arc::clone(&self.frame.class);
        } else if let Some(cls) = &*class_read {
            new_class = Arc::clone(cls);
        } else {
            drop(class_read);
            let mut class_write = class.write().unwrap();
            if let Some(cls) = &*class_write {
                new_class = Arc::clone(cls);
            } else {
                let mut bootstrap_class_loader =
                    BOOTSTRAP_CLASS_LOADER.get().unwrap().lock().unwrap();
                new_class = bootstrap_class_loader.resolve_class(name);
                class_write.replace(Arc::clone(&new_class));
            }
        }
        new_class
    }
}
