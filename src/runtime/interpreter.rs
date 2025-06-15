mod frame;
pub(crate) mod global;
mod instructions;

use super::{
    ArrayType, Class, CpClassInfo, CpNameAndTypeInfo, Exception, FieldResolve, NativeEnv,
    NativeResult, NativeVariable, Object, VmEnv,
};
use crate::descriptor::{
    self, FieldDescriptor, FieldType, MethodDescriptor, parse_field_descriptor,
};
use crate::runtime::Heap;
use crate::runtime::class_loader::resolve_field;
use crate::runtime::global::BOOTSTRAP_CLASS_LOADER;
use crate::runtime::native::NATIVE_FUNCTIONS;
use crate::runtime::{self};
pub use frame::*;
use std::cmp::Ordering;
use std::mem;
use std::ops::Rem;
use std::sync::{Arc, RwLock};

struct InterpreterEnv<'t: 'f, 'f> {
    pc: &'t mut usize,
    frame: &'f mut Frame,
    heap: &'static RwLock<Heap>,
    next_native_thread: Thread<'t>,
}

enum Next {
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
    Exception(Exception),
}

impl<'t, 'f> InterpreterEnv<'t, 'f> {
    pub fn new(
        pc: &'t mut usize,
        frame: &'f mut Frame,
        heap: &'static RwLock<Heap>,
        next_native_thread: Thread<'t>,
    ) -> Self {
        Self {
            pc,
            frame,
            heap,
            next_native_thread,
        }
    }

    fn execute(&mut self) -> Next {
        macro_rules! except {
            ($expr:expr $(,)?) => {
                match $expr {
                    ::std::result::Result::Ok(val) => val,
                    ::std::result::Result::Err(err) => {
                        return Next::Exception(err);
                    }
                }
            };
        }
        let mut wide = false;

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
                inst::ALOAD | inst::ILOAD | inst::FLOAD => {
                    let index = if wide {
                        wide = false;
                        self.get_u16_args() as usize
                    } else {
                        self.get_u8_args() as usize
                    };
                    self.load_n(index);
                }
                inst::LLOAD | inst::DLOAD => {
                    let index = if wide {
                        wide = false;
                        self.get_u16_args() as usize
                    } else {
                        self.get_u8_args() as usize
                    };
                    self.load_n_long(index);
                }
                inst::AALOAD => {
                    let value = except!(self.arr_load::<u32>());
                    self.frame.stack.push(Variable { reference: value });
                }
                inst::IALOAD => {
                    let value = except!(self.arr_load::<i32>());
                    self.push_int(value);
                }
                inst::BALOAD => {
                    let value = except!(self.arr_load::<i8>());
                    self.push_int(value as _);
                }
                inst::CALOAD => {
                    let value = except!(self.arr_load::<u16>());
                    self.push_int(value as _);
                }
                inst::SALOAD => {
                    let value = except!(self.arr_load::<i16>());
                    self.push_int(value as _);
                }
                inst::FALOAD => {
                    let value = except!(self.arr_load::<f32>());
                    self.fconst(value);
                }
                inst::LALOAD => {
                    let value = except!(self.arr_load::<i64>());
                    self.push_long(value);
                }
                inst::DALOAD => {
                    let value = except!(self.arr_load::<f64>());
                    self.push_double(value);
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
                    // TODO: arr type check
                    except!(self.arr_store(value));
                }
                inst::IASTORE => {
                    let value = self.pop_int();
                    except!(self.arr_store(value));
                }
                inst::BASTORE => {
                    let value = self.pop_int() as i8;
                    except!(self.arr_store(value));
                }
                inst::CASTORE => {
                    let value = self.pop_int() as u16;
                    except!(self.arr_store(value));
                }
                inst::SASTORE => {
                    let value = self.pop_int() as i16;
                    except!(self.arr_store(value));
                }
                inst::FASTORE => {
                    let value = self.pop_float();
                    except!(self.arr_store(value));
                }
                inst::LASTORE => {
                    let value = self.pop_long();
                    except!(self.arr_store(value));
                }
                inst::DASTORE => {
                    let value = self.pop_double();
                    except!(self.arr_store(value));
                }

                inst::ASTORE | inst::ISTORE | inst::FSTORE => {
                    let index = if wide {
                        wide = false;
                        self.get_u16_args() as usize
                    } else {
                        self.get_u8_args() as usize
                    };
                    self.store_n(index);
                }
                inst::LSTORE | inst::DSTORE => {
                    let index = if wide {
                        wide = false;
                        self.get_u16_args() as usize
                    } else {
                        self.get_u8_args() as usize
                    };
                    self.store_n_long(index);
                }

                // array
                inst::ARRAYLENGTH => {
                    let arr = unsafe { self.frame.stack.pop().unwrap().reference };
                    if arr == 0 {
                        return Next::Exception(Exception::new("java/lang/NullPointerException"));
                    }
                    let arr_obj = self.heap.read().unwrap().get(arr);

                    let arr_len = Self::get_array_len(&arr_obj);
                    self.push_int(arr_len as _)
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
                inst::FCONST_0 => {
                    self.fconst(0.0);
                }
                inst::FCONST_1 => {
                    self.fconst(1.0);
                }
                inst::FCONST_2 => {
                    self.fconst(2.0);
                }
                inst::DCONST_0 => {
                    self.push_double(0.0);
                }
                inst::DCONST_1 => {
                    self.push_double(1.0);
                }
                inst::ACONST_NULL => {
                    self.frame.stack.push(Variable { reference: 0 });
                }
                inst::BIPUSH => {
                    let byte = self.get_i8_args();
                    self.frame.stack.push(Variable { int: byte as i32 });
                }
                inst::SIPUSH => {
                    let short = self.get_i16_args();
                    self.frame.stack.push(Variable { int: short as i32 });
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
                inst::DUP_X1 => {
                    let v1 = self.frame.stack.pop().unwrap();
                    let v2 = self.frame.stack.pop().unwrap();
                    self.frame.stack.push(v1);
                    self.frame.stack.push(v2);
                    self.frame.stack.push(v1);
                }
                inst::DUP_X2 => {
                    let v1 = self.frame.stack.pop().unwrap();
                    let v2 = self.frame.stack.pop().unwrap();
                    let v3 = self.frame.stack.pop().unwrap();
                    self.frame.stack.push(v1);
                    self.frame.stack.push(v3);
                    self.frame.stack.push(v2);
                    self.frame.stack.push(v1);
                }
                inst::DUP2 => {
                    let v1 = self.frame.stack.pop().unwrap();
                    let v2 = self.frame.stack.pop().unwrap();
                    self.frame.stack.push(v1);
                    self.frame.stack.push(v2);
                    self.frame.stack.push(v1);
                    self.frame.stack.push(v2);
                }
                inst::DUP2_X1 => {
                    let v1 = self.frame.stack.pop().unwrap();
                    let v2 = self.frame.stack.pop().unwrap();
                    let v3 = self.frame.stack.pop().unwrap();
                    self.frame.stack.push(v2);
                    self.frame.stack.push(v1);
                    self.frame.stack.push(v3);
                    self.frame.stack.push(v2);
                    self.frame.stack.push(v1);
                }
                inst::DUP2_X2 => {
                    let v1 = self.frame.stack.pop().unwrap();
                    let v2 = self.frame.stack.pop().unwrap();
                    let v3 = self.frame.stack.pop().unwrap();
                    let v4 = self.frame.stack.pop().unwrap();
                    self.frame.stack.push(v2);
                    self.frame.stack.push(v1);
                    self.frame.stack.push(v4);
                    self.frame.stack.push(v3);
                    self.frame.stack.push(v2);
                    self.frame.stack.push(v1);
                }
                inst::POP => {
                    self.frame.stack.truncate(self.frame.stack.len() - 1);
                }
                inst::POP2 => {
                    self.frame.stack.truncate(self.frame.stack.len() - 2);
                }
                inst::SWAP => {
                    let v1 = self.frame.stack.pop().unwrap();
                    let v2 = self.frame.stack.pop().unwrap();
                    self.frame.stack.push(v1);
                    self.frame.stack.push(v2);
                }

                // arithmetic
                inst::IADD => {
                    let a = self.pop_int();
                    let b = self.pop_int();
                    self.frame.stack.push(Variable {
                        int: a.wrapping_add(b),
                    });
                }
                inst::LADD => {
                    let a = self.pop_long();
                    let b = self.pop_long();
                    self.push_long(a.wrapping_add(b));
                }
                inst::FADD => {
                    let a = self.pop_float();
                    let b = self.pop_float();
                    self.frame.stack.push(Variable { float: a + b });
                }
                inst::DADD => {
                    let a = self.pop_double();
                    let b = self.pop_double();
                    self.push_double(a + b);
                }

                inst::ISUB => {
                    let a = self.pop_int();
                    let b = self.pop_int();
                    self.frame.stack.push(Variable {
                        int: b.wrapping_add(-a),
                    });
                }
                inst::LSUB => {
                    let a = self.pop_long();
                    let b = self.pop_long();
                    self.push_long(b.wrapping_add(-a));
                }
                inst::FSUB => {
                    let a = self.pop_float();
                    let b = self.pop_float();
                    self.frame.stack.push(Variable { float: b - a });
                }
                inst::DSUB => {
                    let a = self.pop_double();
                    let b = self.pop_double();
                    self.push_double(b - a);
                }

                inst::IMUL => {
                    let a = self.pop_int();
                    let b = self.pop_int();
                    self.push_int(a.wrapping_mul(b));
                }
                inst::LMUL => {
                    let a = self.pop_long();
                    let b = self.pop_long();
                    self.push_long(a.wrapping_mul(b));
                }
                inst::FMUL => {
                    let a = self.pop_float();
                    let b = self.pop_float();
                    self.fconst(a * b);
                }
                inst::DMUL => {
                    let a = self.pop_double();
                    let b = self.pop_double();
                    self.push_double(a * b);
                }
                inst::IDIV => {
                    let a = self.pop_int();
                    let b = self.pop_int();
                    if a == 0 {
                        return Next::Exception(Exception::new("java/lang/ArithmeticException"));
                    }
                    self.push_int(b.wrapping_div(a))
                }
                inst::LDIV => {
                    let a = self.pop_long();
                    let b = self.pop_long();
                    if a == 0 {
                        return Next::Exception(Exception::new("java/lang/ArithmeticException"));
                    }
                    self.push_long(b.wrapping_div(a));
                }
                inst::FDIV => {
                    let a = self.pop_float();
                    let b = self.pop_float();
                    self.fconst(b / a);
                }
                inst::DDIV => {
                    let a = self.pop_double();
                    let b = self.pop_double();
                    self.push_double(b / a);
                }

                inst::IREM => {
                    let a = self.pop_int();
                    let b = self.pop_int();
                    if a == 0 {
                        return Next::Exception(Exception::new("java/lang/ArithmeticException"));
                    }
                    self.frame.stack.push(Variable {
                        int: b.wrapping_rem(a),
                    });
                }
                inst::LREM => {
                    let a = self.pop_long();
                    let b = self.pop_long();
                    if a == 0 {
                        return Next::Exception(Exception::new("java/lang/ArithmeticException"));
                    }
                    self.push_long(b.wrapping_rem(a));
                }
                inst::FREM => {
                    let a = self.pop_float();
                    let b = self.pop_float();
                    self.fconst(b.rem(a));
                }
                inst::DREM => {
                    let a = self.pop_double();
                    let b = self.pop_double();
                    self.push_double(b.rem(a));
                }
                inst::INEG => {
                    let a = self.pop_int();
                    self.push_int(a.wrapping_neg());
                }
                inst::LNEG => {
                    let a = self.pop_long();
                    self.push_long(a.wrapping_neg());
                }
                inst::FNEG => {
                    let a = self.pop_float();
                    self.fconst(-a);
                }
                inst::DNEG => {
                    let a = self.pop_double();
                    self.push_double(-a);
                }

                inst::IINC => {
                    let (index, con) = if wide {
                        wide = false;
                        (self.get_u16_args() as usize, self.get_u16_args() as i32)
                    } else {
                        (self.get_u8_args() as usize, self.get_u8_args() as i32)
                    };
                    // SAFETY: rely on class file checking to ensure correct type
                    unsafe { self.frame.locals[index].int += con };
                }
                inst::ISHL => {
                    let v2 = self.pop_int();
                    let v1 = self.pop_int();
                    self.push_int(v1 << (v2 & 0x1F));
                }
                inst::ISHR => {
                    let v2 = self.pop_int();
                    let v1 = self.pop_int();
                    self.push_int(v1 >> (v2 & 0x1F));
                }
                inst::IUSHR => {
                    let v2 = self.pop_int();
                    let v1 = self.pop_int();
                    self.push_int(((v1 as u32) >> (v2 & 0x1F)) as i32);
                }
                inst::LSHL => {
                    let v2 = self.pop_int();
                    let v1 = self.pop_long();
                    self.push_long(v1 << (v2 & 0x1F));
                }
                inst::LSHR => {
                    let v2 = self.pop_int();
                    let v1 = self.pop_long();
                    self.push_long(v1 >> (v2 & 0x1F));
                }
                inst::LUSHR => {
                    let v2 = self.pop_int();
                    let v1 = self.pop_long();
                    self.push_long(((v1 as u64) >> (v2 & 0x1F)) as i64);
                }

                inst::IAND => {
                    let a = self.pop_int();
                    let b = self.pop_int();
                    self.push_int(a & b);
                }
                inst::LAND => {
                    let a = self.pop_long();
                    let b = self.pop_long();
                    self.push_long(a & b);
                }
                inst::IOR => {
                    let a = self.pop_int();
                    let b = self.pop_int();
                    self.push_int(a | b);
                }
                inst::LOR => {
                    let a = self.pop_long();
                    let b = self.pop_long();
                    self.push_long(a | b);
                }
                inst::IXOR => {
                    let a = self.pop_int();
                    let b = self.pop_int();
                    self.push_int(a ^ b);
                }
                inst::LXOR => {
                    let a = self.pop_long();
                    let b = self.pop_long();
                    self.push_long(a ^ b);
                }

                // conversion
                inst::I2L => {
                    let v = self.pop_int();
                    self.push_long(v as i64);
                }
                inst::I2B => {
                    let v = self.pop_int();
                    self.frame.stack.push(Variable {
                        int: v as i8 as i32,
                    });
                }
                inst::I2C => {
                    let v = self.pop_int();
                    self.frame.stack.push(Variable {
                        int: v as u16 as i32,
                    });
                }
                inst::I2S => {
                    let v = self.pop_int();
                    self.frame.stack.push(Variable {
                        int: v as i16 as i32,
                    });
                }
                inst::I2F => {
                    let v = self.pop_int();
                    self.frame.stack.push(Variable { float: v as f32 });
                }
                inst::I2D => {
                    let v = self.pop_int();
                    self.push_double(v as f64);
                }
                inst::L2I => {
                    let v = self.pop_long();
                    self.push_int(v as i32);
                }
                inst::L2F => {
                    let v = self.pop_long();
                    self.fconst(v as f32);
                }
                inst::L2D => {
                    let v = self.pop_long();
                    self.push_double(v as f64);
                }
                inst::F2I => {
                    let v = self.pop_float();
                    self.push_int(v as i32);
                }
                inst::F2L => {
                    let v = self.pop_float();
                    self.push_long(v as i64);
                }
                inst::F2D => {
                    let v = self.pop_float();
                    self.push_double(v as f64);
                }
                inst::D2I => {
                    let v = self.pop_double();
                    self.push_int(v as i32);
                }
                inst::D2L => {
                    let v = self.pop_double();
                    self.push_long(v as i64);
                }
                inst::D2F => {
                    let v = self.pop_double();
                    self.fconst(v as f32);
                }

                // comparing
                inst::LCMP => {
                    let v2 = self.pop_long();
                    let v1 = self.pop_long();
                    match v1.cmp(&v2) {
                        Ordering::Less => self.push_int(-1),
                        Ordering::Equal => self.push_int(0),
                        Ordering::Greater => self.push_int(1),
                    }
                }
                inst::FCMPG => self.fcmp(1),
                inst::FCMPL => self.fcmp(-1),
                inst::DCMPG => self.dcmp(1),
                inst::DCMPL => self.dcmp(-1),

                // branch
                inst::IF_ACMPEQ => {
                    // SAFETY: rely on class file checking to ensure correct type
                    let a = unsafe { self.frame.stack.pop().unwrap().reference };
                    let b = unsafe { self.frame.stack.pop().unwrap().reference };
                    if self.goto(a == b) {
                        continue;
                    }
                }
                inst::IF_ACMPNE => {
                    // SAFETY: rely on class file checking to ensure correct type
                    let a = unsafe { self.frame.stack.pop().unwrap().reference };
                    let b = unsafe { self.frame.stack.pop().unwrap().reference };
                    if self.goto(a != b) {
                        continue;
                    }
                }
                inst::IF_ICMPEQ => {
                    let v2 = self.pop_int();
                    let v1 = self.pop_int();
                    if self.goto(v1 == v2) {
                        continue;
                    }
                }
                inst::IF_ICMPNE => {
                    let v2 = self.pop_int();
                    let v1 = self.pop_int();
                    if self.goto(v1 != v2) {
                        continue;
                    }
                }
                inst::IF_ICMPLT => {
                    let v2 = self.pop_int();
                    let v1 = self.pop_int();
                    if self.goto(v1 < v2) {
                        continue;
                    }
                }
                inst::IF_ICMPGT => {
                    let v2 = self.pop_int();
                    let v1 = self.pop_int();
                    if self.goto(v1 > v2) {
                        continue;
                    }
                }
                inst::IF_ICMPLE => {
                    let v2 = self.pop_int();
                    let v1 = self.pop_int();
                    if self.goto(v1 <= v2) {
                        continue;
                    }
                }
                inst::IF_ICMPGE => {
                    let v2 = self.pop_int();
                    let v1 = self.pop_int();
                    if self.goto(v1 >= v2) {
                        continue;
                    }
                }
                inst::IFEQ => {
                    let v2 = 0;
                    let v1 = self.pop_int();
                    if self.goto(v1 == v2) {
                        continue;
                    }
                }
                inst::IFNE => {
                    let v2 = 0;
                    let v1 = self.pop_int();
                    if self.goto(v1 != v2) {
                        continue;
                    }
                }
                inst::IFLT => {
                    let v2 = 0;
                    let v1 = self.pop_int();
                    if self.goto(v1 < v2) {
                        continue;
                    }
                }
                inst::IFGT => {
                    let v2 = 0;
                    let v1 = self.pop_int();
                    if self.goto(v1 > v2) {
                        continue;
                    }
                }
                inst::IFLE => {
                    let v2 = 0;
                    let v1 = self.pop_int();
                    if self.goto(v1 <= v2) {
                        continue;
                    }
                }
                inst::IFGE => {
                    let v2 = 0;
                    let v1 = self.pop_int();
                    if self.goto(v1 >= v2) {
                        continue;
                    }
                }
                inst::IFNULL => {
                    let a = unsafe { self.frame.stack.pop().unwrap().reference };
                    if self.goto(a == 0) {
                        continue;
                    }
                }
                inst::IFNONNULL => {
                    let a = unsafe { self.frame.stack.pop().unwrap().reference };
                    if self.goto(a != 0) {
                        continue;
                    }
                }
                inst::GOTO => {
                    self.goto(true);
                    continue;
                }
                inst::GOTO_W => {
                    self.goto_w();
                    continue;
                }
                inst::JSR => {
                    let offset = self.get_i16_args();
                    self.frame.stack.push(Variable {
                        return_address: *self.pc as _,
                    });
                    *self.pc = self.pc.wrapping_add_signed(offset as _);
                }
                inst::JSR_W => {
                    let offset = self.get_i32_args();
                    self.frame.stack.push(Variable {
                        return_address: *self.pc as _,
                    });
                    *self.pc = self.pc.wrapping_add_signed(offset as _);
                }
                inst::RET => {
                    let index = if wide {
                        wide = false;
                        self.get_u16_args() as usize
                    } else {
                        self.get_u8_args() as usize
                    };
                    // SAFETY: rely on class file checking to ensure correct type
                    let return_pc = unsafe { self.frame.locals[index].return_address };
                    *self.pc = return_pc as _;
                }
                inst::LOOKUPSWITCH => {
                    self.lookup_switch();
                    continue;
                }
                inst::TABLESWITCH => {
                    self.table_switch();
                    continue;
                }

                // oop
                inst::NEW => {
                    except!(self.new_object());
                }
                inst::NEWARRAY => {
                    except!(self.new_array());
                }

                // TODO: check static / instance field
                inst::PUTFIELD => {
                    except!(self.put_field());
                }
                inst::GETFIELD => {
                    except!(self.get_field());
                }
                inst::GETSTATIC => {
                    except!(self.get_static());
                }
                inst::PUTSTATIC => {
                    except!(self.put_static());
                }
                inst::CHECKCAST => {
                    // TODO: do real check
                    let cp_index = self.get_u16_args();
                }
                inst::INSTANCEOF => {
                    let cp_index = self.get_u16_args();
                    // SAFETY: rely on class file checking to ensure correct type
                    let obj_ref = unsafe { self.frame.stack.pop().unwrap().reference };
                    if obj_ref == 0 {
                        self.push_int(0);
                    } else {
                        // TODO: do real check
                        self.push_int(0);
                    }
                }

                // call
                // TODO: invokevirtual should lookup vtable
                //  do monitor ops for synchronized
                inst::INVOKESPECIAL | inst::INVOKEVIRTUAL => {
                    let cp_index = self.get_u16_args();
                    let runtime::ConstantPoolInfo::Methodref {
                        class,
                        name_and_type,
                    } = self.frame.class.get_constant(cp_index)
                    else {
                        panic!("invalid constant type {}", cp_index);
                    };
                    let class_to_invoke = except!(self.resolve_class(&class));
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
                    let class_to_invoke = except!(self.resolve_class(&class));

                    return Next::InvokeStatic {
                        class: class_to_invoke,
                        name_and_type: name_and_type.clone(),
                    };
                }
                inst::INVOKENATIVE => {
                    except!(self.invoke_native());
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
                inst::LRETURN | inst::DRETURN => {
                    return Next::Return {
                        v2: self.frame.stack.pop().unwrap(),
                        v1: self.frame.stack.pop().unwrap(),
                        return_pc: self.pop_return_addr(),
                    };
                }

                inst::WIDE => {
                    wide = true;
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
    fn get_i32_args(&mut self) -> i32 {
        let byte1 = self.frame.code[*self.pc + 1] as i32;
        let byte2 = self.frame.code[*self.pc + 2] as i32;
        let byte3 = self.frame.code[*self.pc + 3] as i32;
        let byte4 = self.frame.code[*self.pc + 4] as i32;
        *self.pc += 4;
        (byte1 << 24) | (byte2 << 16) | (byte3 << 8) | byte4
    }
    #[inline]
    fn get_i32_args_from(&self, pc: usize) -> i32 {
        let byte1 = self.frame.code[pc + 1] as i32;
        let byte2 = self.frame.code[pc + 2] as i32;
        let byte3 = self.frame.code[pc + 3] as i32;
        let byte4 = self.frame.code[pc + 4] as i32;
        (byte1 << 24) | (byte2 << 16) | (byte3 << 8) | byte4
    }

    #[inline]
    fn pop_return_addr(&mut self) -> usize {
        let lower = unsafe { self.frame.stack.pop().unwrap().return_address } as usize;
        let upper = unsafe { self.frame.stack.pop().unwrap().return_address } as usize;

        (upper << 32) | lower
    }

    #[inline]
    fn pop_int(&mut self) -> i32 {
        // SAFETY: rely on class file checking to ensure correct type
        unsafe { self.frame.stack.pop().unwrap().get_int() }
    }

    #[inline]
    fn push_int(&mut self, i: i32) {
        self.frame.stack.push(Variable { int: i });
    }

    #[inline]
    fn pop_float(&mut self) -> f32 {
        // SAFETY: rely on class file checking to ensure correct type
        unsafe { self.frame.stack.pop().unwrap().float }
    }

    #[inline]
    fn pop_long(&mut self) -> i64 {
        let i2 = self.frame.stack.pop().unwrap();
        let i1 = self.frame.stack.pop().unwrap();
        // SAFETY: rely on class file checking to ensure correct type
        unsafe { Variable::get_long(i1, i2) }
    }

    #[inline]
    fn push_long(&mut self, l: i64) {
        let (upper, lower) = Variable::put_long(l);
        self.frame.stack.push(upper);
        self.frame.stack.push(lower);
    }

    #[inline]
    fn pop_double(&mut self) -> f64 {
        let i2 = self.frame.stack.pop().unwrap();
        let i1 = self.frame.stack.pop().unwrap();
        // SAFETY: rely on class file checking to ensure correct type
        unsafe { Variable::get_double(i1, i2) }
    }

    #[inline]
    fn push_double(&mut self, f: f64) {
        let (upper, lower) = Variable::put_double(f);
        self.frame.stack.push(upper);
        self.frame.stack.push(lower);
    }

    fn new_object(&mut self) -> NativeResult<()> {
        let cp_index = self.get_u16_args();
        let runtime::ConstantPoolInfo::Class(cp_info) = self.frame.class.get_constant(cp_index)
        else {
            panic!("invalid constant type {}", cp_index);
        };
        let new_class = self.resolve_class(cp_info)?;

        let mut fields_types = Vec::new();

        for f in &new_class.instance_fields_info {
            if f.descriptor.0.is_long() {
                fields_types.push(&f.descriptor);
            }
            fields_types.push(&f.descriptor);
        }

        let mut heap = self.heap.write().unwrap();
        let id = unsafe {
            heap.allocate_object(fields_types.len(), Arc::clone(&new_class), |i, v| {
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
        Ok(())
    }

    fn new_array(&mut self) -> NativeResult<()> {
        let atype = self.get_i8_args();
        let count = self.pop_int();
        if count < 0 {
            return Err(Exception::new("java/lang/NegativeArraySizeException"));
        }
        let arr_type = match atype {
            // bool
            4 => FieldType::Boolean,
            // byte
            8 => FieldType::Byte,
            // char
            5 => FieldType::Char,
            // float
            6 => FieldType::Float,
            // double
            7 => FieldType::Double,
            // short
            9 => FieldType::Short,
            // int
            10 => FieldType::Int,
            // long
            11 => FieldType::Long,
            _ => panic!("invalid array type {}", atype),
        };

        // build array class
        let bootstrap_class_loader = BOOTSTRAP_CLASS_LOADER.get().unwrap();
        let new_class = bootstrap_class_loader.resolve_primitive_array_class(
            &VmEnv::with_cur_frame(&self.next_native_thread, self.frame),
            &arr_type,
        )?;

        let mut heap = self.heap.write().unwrap();

        let id = match arr_type {
            FieldType::Boolean | FieldType::Byte => {
                heap.allocate_array::<i8>(count as _, new_class)
            }
            // char
            FieldType::Char => heap.allocate_array::<u16>(count as _, new_class),
            // float
            FieldType::Float => heap.allocate_array::<f32>(count as _, new_class),
            // double
            FieldType::Double => heap.allocate_array::<f64>(count as _, new_class),
            // short
            FieldType::Short => heap.allocate_array::<i16>(count as _, new_class),
            // int
            FieldType::Int => heap.allocate_array::<i32>(count as _, new_class),
            // long
            FieldType::Long => heap.allocate_array::<i64>(count as _, new_class),
            _ => panic!("invalid array type {}", atype),
        };
        self.frame.stack.push(Variable { reference: id });
        Ok(())
    }

    fn put_field(&mut self) -> NativeResult<()> {
        let (index, is_long) = self.resolve_instance_field()?;
        let v1;
        let mut v2 = None;
        if is_long {
            v2 = Some(self.frame.stack.pop().unwrap());
            v1 = self.frame.stack.pop().unwrap();
        } else {
            v1 = self.frame.stack.pop().unwrap();
        }

        let this = unsafe { self.frame.stack.pop().unwrap().reference };
        if this == 0 {
            return Err(Exception::new("java/lang/NullPointerException"));
        }
        let this_obj = self.heap.read().unwrap().get(this);
        unsafe {
            this_obj.put_field(index as usize, v1);
            if let Some(v2) = v2 {
                this_obj.put_field((index + 1) as usize, v2);
            }
        }

        Ok(())
    }

    fn get_field(&mut self) -> NativeResult<()> {
        let (index, is_long) = self.resolve_instance_field()?;

        let this = unsafe { self.frame.stack.pop().unwrap().reference };
        if this == 0 {
            return Err(Exception::new("java/lang/NullPointerException"));
        }
        let this_obj = self.heap.read().unwrap().get(this);

        self.frame
            .stack
            .push(unsafe { this_obj.get_field(index as usize) });

        if is_long {
            self.frame
                .stack
                .push(unsafe { this_obj.get_field((index + 1) as usize) });
        }
        Ok(())
    }

    fn resolve_instance_field(&mut self) -> NativeResult<(u16, bool)> {
        let cp_index = self.get_u16_args();
        let runtime::ConstantPoolInfo::Fieldref(
            field_ref @ runtime::Fieldref {
                name_and_type,
                resolve,
                ..
            },
        ) = self.frame.class.get_constant(cp_index)
        else {
            panic!("invalid constant type {}", cp_index);
        };

        let resolve = resolve.get_or_try_init(|| self.resolve_field(field_ref, false))?;
        let index = resolve.get_index();
        let is_long = name_and_type.descriptor.0.is_long();

        Ok((index, is_long))
    }

    fn get_static(&mut self) -> NativeResult<()> {
        let (class, index, is_long) = self.resolve_static_field()?;

        self.frame.stack.push(class.get_static_field(index));
        if is_long {
            self.frame.stack.push(class.get_static_field(index + 1));
        }

        Ok(())
    }

    fn put_static(&mut self) -> NativeResult<()> {
        let (class, index, is_long) = self.resolve_static_field()?;

        if is_long {
            class.set_static_field(index + 1, self.frame.stack.pop().unwrap());
            class.set_static_field(index, self.frame.stack.pop().unwrap());
        } else {
            class.set_static_field(index, self.frame.stack.pop().unwrap());
        }
        Ok(())
    }

    fn resolve_static_field(&mut self) -> Result<(Arc<Class>, u16, bool), Exception> {
        let cp_index = self.get_u16_args();
        let runtime::ConstantPoolInfo::Fieldref(
            field_ref @ runtime::Fieldref {
                name_and_type,
                resolve,
                ..
            },
        ) = self.frame.class.get_constant(cp_index)
        else {
            panic!("invalid constant type {}", cp_index);
        };

        let resolve = resolve.get_or_try_init(|| self.resolve_field(field_ref, true))?;
        let (class, &index) = match resolve {
            FieldResolve::InThisClass(index) => (&self.frame.class, index),
            FieldResolve::OtherClass { class, index } => (class, index),
        };

        let is_long = matches!(
            name_and_type.descriptor.0,
            descriptor::FieldType::Long | descriptor::FieldType::Double
        );
        Ok((Arc::clone(class), index, is_long))
    }

    #[inline]
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

    #[inline]
    fn ldc2(&mut self, index: u16) {
        match self.frame.class.get_constant(index) {
            runtime::ConstantPoolInfo::Long(l) => {
                self.push_long(*l);
            }
            runtime::ConstantPoolInfo::Double(d) => {
                self.push_double(*d);
            }
            _ => {
                panic!("ldc2 error, invalid constant type");
            }
        }
    }

    #[inline]
    fn fcmp(&mut self, nan: i32) {
        let v2 = self.pop_float();
        let v1 = self.pop_float();
        match v1.partial_cmp(&v2) {
            None => self.push_int(nan),
            Some(Ordering::Less) => self.push_int(-1),
            Some(Ordering::Equal) => self.push_int(0),
            Some(Ordering::Greater) => self.push_int(1),
        }
    }

    #[inline]
    fn dcmp(&mut self, nan: i32) {
        let v2 = self.pop_double();
        let v1 = self.pop_double();
        match v1.partial_cmp(&v2) {
            None => self.push_int(nan),
            Some(Ordering::Less) => self.push_int(-1),
            Some(Ordering::Equal) => self.push_int(0),
            Some(Ordering::Greater) => self.push_int(1),
        }
    }

    #[inline]
    fn goto(&mut self, jump: bool) -> bool {
        let offset = self.get_i16_args();
        if jump {
            *self.pc = self.pc.wrapping_add_signed((offset - 2) as isize);
            return true;
        }
        false
    }

    fn goto_w(&mut self) {
        let offset = self.get_i32_args();
        *self.pc = self.pc.wrapping_add_signed((offset - 4) as isize);
    }

    fn invoke_native(&mut self) -> NativeResult<()> {
        let class_name = self.frame.class.class_name.to_string();
        let method_name = self.frame.method_name.to_string();
        let param_descriptor = self.frame.param_descriptor.clone();

        let mut args = Vec::with_capacity(self.frame.locals.len());
        let mut i = 0;
        let locals = &self.frame.locals;
        if !self.frame.is_static {
            // this arg
            // SAFETY: rely on class file checking to ensure correct type
            args.push(NativeVariable::Reference(unsafe { locals[i].reference }));
            i += 1;
        }

        for field in &param_descriptor {
            // SAFETY: rely on class file checking to ensure correct type
            let arg = unsafe {
                match field {
                    FieldType::Byte => NativeVariable::Byte(locals[i].get_int() as _),
                    FieldType::Char => NativeVariable::Char(locals[i].get_int() as _),
                    FieldType::Double => {
                        let double = Variable::get_double(locals[i], locals[i + 1]);
                        i += 1;
                        NativeVariable::Double(double)
                    }
                    FieldType::Float => NativeVariable::Float(locals[i].float),
                    FieldType::Int => NativeVariable::Int(locals[i].get_int()),
                    FieldType::Long => {
                        let long = Variable::get_long(locals[i], locals[i + 1]);
                        i += 1;
                        NativeVariable::Long(long)
                    }
                    FieldType::Object(_) | FieldType::Array(_) => {
                        NativeVariable::Reference(locals[i].reference)
                    }
                    FieldType::Short => NativeVariable::Short(locals[i].get_int() as _),
                    FieldType::Boolean => NativeVariable::Boolean(locals[i].get_int() == 1),
                }
            };
            i += 1;
            args.push(arg);
        }

        let method = NATIVE_FUNCTIONS
            .get(&(class_name, method_name, param_descriptor))
            .unwrap();

        let ret = method(NativeEnv {
            args,
            heap: self.heap,
            class: Arc::clone(&self.frame.class),
        })?;
        // TODO: check actual return type
        let stack = &mut self.frame.stack;
        match ret {
            None => {}
            Some(NativeVariable::Byte(b)) => self.iconst(b as _),
            Some(NativeVariable::Boolean(b)) => self.iconst(b as _),
            Some(NativeVariable::Char(c)) => self.iconst(c as _),
            Some(NativeVariable::Short(s)) => self.iconst(s as _),
            Some(NativeVariable::Int(i)) => self.iconst(i),
            Some(NativeVariable::Long(l)) => self.push_long(l),
            Some(NativeVariable::Float(f)) => self.fconst(f),
            Some(NativeVariable::Double(d)) => self.push_double(d),
            Some(NativeVariable::Reference(r)) => stack.push(Variable { reference: r }),
        }
        Ok(())
    }

    fn arr_load<T: ArrayType>(&mut self) -> NativeResult<T> {
        let index = unsafe { self.frame.stack.pop().unwrap().get_int() };
        let arr = unsafe { self.frame.stack.pop().unwrap().reference };
        if arr == 0 {
            return Err(Exception::new("java/lang/NullPointerException"));
        }
        let arr_object = self.heap.read().unwrap().get(arr);

        let field_type = Self::get_array_type(&arr_object.class);
        let type_size = Self::get_field_type_size(field_type);
        let arr_len = arr_object.get_u8_array_size() / type_size;
        // check array type
        if type_size != size_of::<T>() {
            panic!("invalid array type");
        }
        // check array size
        if index >= arr_len as _ {
            return Err(Exception::new("java/lang/ArrayIndexOutOfBoundsException"));
        }
        Ok(unsafe { arr_object.get_array_index::<T>(index as _) })
    }

    fn arr_store<T: ArrayType>(&mut self, value: T) -> NativeResult<()> {
        let index = self.pop_int();
        let arr = unsafe { self.frame.stack.pop().unwrap().reference };
        if arr == 0 {
            return Err(Exception::new("java/lang/NullPointerException"));
        }

        let arr_object = self.heap.read().unwrap().get(arr);

        let field_type = Self::get_array_type(&arr_object.class);
        let type_size = Self::get_field_type_size(field_type);
        let arr_len = arr_object.get_u8_array_size() / type_size;
        // check array type
        // TODO: check for object type
        if type_size != size_of::<T>() {
            return Err(Exception::new("java/lang/ArrayStoreException"));
        }
        // check array size
        if index >= arr_len as _ {
            return Err(Exception::new("java/lang/ArrayIndexOutOfBoundsException"));
        }

        unsafe {
            // SAFETY: must be array
            arr_object.put_array_index(index as _, value);
        }
        Ok(())
    }

    fn lookup_switch(&mut self) {
        let start_pc = *self.pc;
        *self.pc = (*self.pc & 4) + 3;
        let default = self.get_i32_args();
        let npairs = self.get_i32_args();
        let key = self.pop_int();

        for _ in 0..npairs {
            let mat = self.get_i32_args();
            let offset = self.get_i32_args();
            if key == mat {
                *self.pc = start_pc.wrapping_add_signed(offset as isize);
                return;
            }
        }
        *self.pc = start_pc.wrapping_add_signed(default as isize);
    }
    fn table_switch(&mut self) {
        let start_pc = *self.pc;
        *self.pc = (*self.pc & 4) + 3;
        let default = self.get_i32_args();
        let low = self.get_i32_args();
        let high = self.get_i32_args();
        let index = self.pop_int();

        if index < low || index > high {
            *self.pc = start_pc.wrapping_add_signed(default as isize);
            return;
        }
        let pos = index - low;
        let offset = self.get_i32_args_from(*self.pc + 4 * pos as usize);
        *self.pc = start_pc.wrapping_add_signed(offset as isize);
    }

    fn resolve_class(&self, class: &CpClassInfo) -> NativeResult<Arc<Class>> {
        if class.name == self.frame.class.class_name {
            Ok(Arc::clone(&self.frame.class))
        } else {
            let bootstrap_class_loader = BOOTSTRAP_CLASS_LOADER.get().unwrap();
            class.get_or_load_class(|| {
                bootstrap_class_loader.resolve_class(
                    &VmEnv::with_cur_frame(&self.next_native_thread, self.frame),
                    &class.name,
                )
            })
        }
    }

    fn resolve_field(
        &self,
        field_ref: &runtime::Fieldref,
        is_static: bool,
    ) -> NativeResult<FieldResolve> {
        let bootstrap_class_loader = BOOTSTRAP_CLASS_LOADER.get().unwrap();
        let class = bootstrap_class_loader.resolve_class(
            &VmEnv::with_cur_frame(&self.next_native_thread, self.frame),
            &field_ref.class_name,
        )?;
        // TODO: maybe exception
        Ok(resolve_field(&class, field_ref, is_static).expect("field cannot be resolved"))
    }

    fn get_array_type(class: &Arc<Class>) -> FieldType {
        if !class.class_name.starts_with("[") {
            panic!("not an array");
        }
        let (_, FieldDescriptor(field_type)) =
            parse_field_descriptor(&class.class_name).expect("invalid array type");
        field_type
    }

    fn get_array_len(object: &Object) -> usize {
        let field_type = Self::get_array_type(&object.class);
        let raw_size = object.get_u8_array_size();

        let size = Self::get_field_type_size(field_type);
        raw_size / size
    }

    fn get_field_type_size(field_type: FieldType) -> usize {
        match field_type {
            FieldType::Byte => 1,
            FieldType::Char => 2,
            FieldType::Double => 8,
            FieldType::Float => 4,
            FieldType::Int => 4,
            FieldType::Long => 8,
            FieldType::Object(_) => 4,
            FieldType::Short => 2,
            FieldType::Boolean => 1,
            FieldType::Array(_) => 4,
        }
    }
}
