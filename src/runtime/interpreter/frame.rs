use crate::{
    class::JavaStr,
    consts::{ClassAccessFlag, MethodAccessFlag},
    descriptor::{FieldType, ReturnType},
    runtime,
    runtime::{
        CodeAttribute, Exception, ExceptionTableItem, NativeResult, VmEnv, VtableIndex,
        class_loader::initialize_class,
        global::BOOTSTRAP_CLASS_LOADER,
        inheritance::is_same_or_sub_class_of,
        interpreter::{InterpreterEnv, Next, global, instructions},
    },
};
use std::{
    fmt::{Debug, Formatter},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

pub struct Thread<'t> {
    pub(in crate::runtime) top_frame: Option<Frame>,
    max_frame_size: usize,
    thread_id: usize,
    pub(in crate::runtime) previous_thread: Option<&'t Thread<'t>>,
}

pub struct Frame {
    pub(in crate::runtime) class: Arc<runtime::Class>,
    pub(super) code: Arc<[u8]>,
    pub(super) return_type: ReturnType,
    pub(super) locals: Vec<Variable>,
    pub(super) stack: Vec<Variable>,
    pub(in crate::runtime) previous_frame: Option<Box<Frame>>,
    pub(in crate::runtime) method_name: String,
    pub(super) param_descriptor: Vec<FieldType>,
    pub(super) is_static: bool,
    pub(super) exception_table: Vec<ExceptionTableItem>,
}

impl Frame {
    pub(in crate::runtime) fn clone_dummy(&self) -> Frame {
        Frame {
            class: Arc::clone(&self.class),
            code: Arc::new([]),
            return_type: self.return_type.clone(),
            locals: vec![],
            stack: vec![],
            previous_frame: self
                .previous_frame
                .as_ref()
                .map(|f| Box::new(f.clone_dummy())),
            method_name: self.method_name.clone(),
            param_descriptor: self.param_descriptor.clone(),
            is_static: self.is_static,
            exception_table: vec![],
        }
    }

    fn is_dummy(&self) -> bool {
        self.code.is_empty()
    }
}

#[derive(Copy, Clone)]
pub union Variable {
    // boolean: bool,
    // byte: i8,
    // char: u16,
    // short: i16,
    pub(in crate::runtime) int: i32,
    pub(in crate::runtime) float: f32,
    pub(in crate::runtime) reference: u32,
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
    #[inline]
    pub unsafe fn get_int(self) -> i32 {
        unsafe { self.int }
    }

    /// # Safety
    ///
    /// should ensure the underlying type is long
    #[inline]
    pub unsafe fn get_long(pre: Self, suf: Self) -> i64 {
        let upper = unsafe { pre.get_int() as i64 };
        let lower = unsafe { suf.get_int() as i64 };
        (upper << 32) | lower
    }

    #[inline]
    pub fn put_long(long: i64) -> (Variable, Variable) {
        let lower = long as i32;
        let upper = (long >> 32) as i32;
        (Variable { int: upper }, Variable { int: lower })
    }

    /// # Safety
    ///
    /// should ensure the underlying type is double
    #[inline]
    pub unsafe fn get_double(pre: Self, suf: Self) -> f64 {
        let upper = unsafe { pre.get_int() as u64 };
        let lower = unsafe { suf.get_int() as u64 };
        f64::from_bits((upper << 32) | lower)
    }

    #[inline]
    pub fn put_double(double: f64) -> (Variable, Variable) {
        let long = double.to_bits();
        let lower = long as i32;
        let upper = (long >> 32) as i32;
        (Variable { int: upper }, Variable { int: lower })
    }
}

impl Thread<'_> {
    pub fn new(max_frame_size: usize) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        let mut last = COUNTER.load(Ordering::Relaxed);
        let thread_id = loop {
            let Some(id) = last.checked_add(1) else {
                panic!("thread id overflow");
            };

            match COUNTER.compare_exchange_weak(last, id, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break id,
                Err(id) => last = id,
            }
        };

        Thread {
            top_frame: None,
            max_frame_size,
            thread_id,
            previous_thread: None,
        }
    }

    pub fn new_main_frame(
        &mut self,
        main_class: &str,
        method_name: &str,
        param_descriptor: &[FieldType],
    ) {
        let loader = BOOTSTRAP_CLASS_LOADER.get().unwrap();
        let main_class = loader
            .resolve_class(main_class)
            .expect("cannot load main class");
        initialize_class(&VmEnv::new(self, &global::HEAP), &main_class).unwrap();
        self.new_frame(
            main_class,
            &JavaStr::from_str(method_name),
            param_descriptor,
            0,
        );
    }
    pub(in crate::runtime) fn new_frame(
        &mut self,
        class: Arc<runtime::Class>,
        method_name: &JavaStr,
        param_descriptor: &[FieldType],
        return_address: usize,
    ) {
        let top_frame = &mut self.top_frame;
        let Some(method_info) = class.resolve_method(method_name, param_descriptor) else {
            panic!("{method_name:?}");
        };
        Self::new_frame_with_method_info(
            top_frame,
            Arc::clone(&class),
            method_info,
            return_address,
            false,
        );
    }

    pub fn new_native_frame_group(&self, frame: Option<Frame>) -> Thread<'_> {
        Thread {
            top_frame: frame,
            max_frame_size: self.max_frame_size,
            thread_id: self.thread_id,
            previous_thread: Some(self),
        }
    }

    fn new_frame_resolved(
        top_frame: &mut Option<Frame>,
        class: Arc<runtime::Class>,
        index: usize,
        return_address: usize,
        need_this: bool,
    ) {
        let method_info = class
            .methods
            .get(index)
            .unwrap_or_else(|| panic!("method not found {index}"));
        Self::new_frame_with_method_info(
            top_frame,
            Arc::clone(&class),
            method_info,
            return_address,
            need_this,
        );
    }
    fn new_frame_with_method_info(
        top_frame: &mut Option<Frame>,
        class: Arc<runtime::Class>,
        method_info: &runtime::MethodInfo,
        return_address: usize,
        need_this: bool,
    ) {
        // find code attribute
        let mut code_attribute = None;
        for attr in &method_info.attributes {
            match attr {
                runtime::AttributeInfo::Code(code) => {
                    code_attribute = Some(code);
                    break;
                }
                _ => continue,
            }
        }

        // native method
        let native_code_attribute;
        if method_info.access_flags.contains(MethodAccessFlag::NATIVE) {
            let return_inst = match method_info.descriptor.return_type {
                None => instructions::RETURN,
                Some(FieldType::Long) => instructions::LRETURN,
                Some(
                    FieldType::Byte
                    | FieldType::Char
                    | FieldType::Int
                    | FieldType::Short
                    | FieldType::Boolean,
                ) => instructions::IRETURN,
                Some(FieldType::Double) => instructions::DRETURN,
                Some(FieldType::Float) => instructions::FRETURN,
                Some(FieldType::Object(_) | FieldType::Array(_)) => instructions::ARETURN,
            };

            native_code_attribute = CodeAttribute {
                max_stack: 2,
                max_locals: method_info.descriptor.parameters.len() as u16,
                code: Arc::new([instructions::INVOKENATIVE, return_inst]),
                exception_table: vec![],
                attributes: vec![],
            };
            code_attribute = Some(&native_code_attribute)
        }
        let Some(code) = code_attribute else {
            panic!(
                "method code attributes not found: {}",
                method_info.name.to_str()
            );
        };

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
            previous_frame: previous_frame.map(Box::new),
            method_name: method_info.name.to_str().into_owned(),
            param_descriptor: method_info.descriptor.parameters.to_vec(),
            is_static: !need_this,
            exception_table: code.exception_table.clone(),
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

        *top_frame = Some(frame);
    }

    pub fn top_frame(&mut self) -> Option<&mut Frame> {
        self.top_frame.as_mut()
    }

    pub fn execute(&mut self) -> NativeResult<()> {
        let mut pc = 0;
        while let Some(mut frame) = self.top_frame.take() {
            if frame.is_dummy() {
                break;
            }

            let native_frame_group = self.new_native_frame_group(Some(frame.clone_dummy()));
            let mut env =
                InterpreterEnv::new(&mut pc, &mut frame, &global::HEAP, native_frame_group);
            let next = env.execute();

            match next {
                Next::Return { return_pc, v1, v2 } => {
                    let (is_void, is_long) = match frame.return_type {
                        Some(FieldType::Long | FieldType::Double) => (false, true),
                        Some(_) => (false, false),
                        None => (true, false),
                    };
                    self.top_frame = frame.previous_frame.map(|f| *f);
                    pc = return_pc;
                    if let Some(ref mut frame) = self.top_frame {
                        if !is_void {
                            frame.stack.push(v1);
                            if is_long {
                                frame.stack.push(v2);
                            }
                        }
                    }
                    print!(
                        "return from {}.{}({:?})",
                        frame.class.class_name, frame.method_name, frame.param_descriptor
                    );
                    if !is_void {
                        if is_long {
                            print!(" with {}L", unsafe { Variable::get_long(v1, v2) });
                        } else if let Some(FieldType::Object(cls)) = frame.return_type
                            && cls == "java/lang/String"
                        {
                            let str_ref = unsafe { v1.reference };
                            let obj = global::HEAP.read().unwrap().get(str_ref);
                            let bytes_ref = unsafe { obj.get_field(0).reference };
                            let obj = global::HEAP.read().unwrap().get(bytes_ref);
                            let len = obj.get_array_size(1);
                            print!(" with ");
                            for i in 0..len {
                                print!("{}", unsafe { obj.get_array_index_raw(i, 1)[0] as char })
                            }
                        } else {
                            print!(" with {}", unsafe { v1.int });
                        }
                    }
                    println!();
                }
                Next::Exception(exception) => {
                    self.handle_exception(exception, frame, &mut pc)?;
                }
                Next::InvokeSpecial {
                    static_class,
                    index,
                    vtable_index,
                    is_virtual,
                    this,
                } => {
                    self.top_frame = Some(frame);

                    if !is_virtual || vtable_index < 0 {
                        if cfg!(debug_assertions) && is_virtual {
                            let statically_resolved_method = &static_class.methods[index];
                            assert!(
                                statically_resolved_method
                                    .access_flags
                                    .contains(MethodAccessFlag::PRIVATE)
                                    || statically_resolved_method
                                        .access_flags
                                        .contains(MethodAccessFlag::FINAL)
                                    || static_class.access_flags.contains(ClassAccessFlag::FINAL)
                            );
                        }
                        println!("invokespecial {}.{}", static_class.class_name, index);
                        // invokespecial
                        Self::new_frame_resolved(
                            &mut self.top_frame,
                            static_class,
                            index,
                            pc + 1,
                            true,
                        );
                    } else {
                        let this_obj = global::HEAP.read().unwrap().get(this);
                        let this_class = this_obj.get_class();
                        let vtable_entry = &this_class.vtable[vtable_index as usize];
                        let (class, method) = match &vtable_entry.index {
                            VtableIndex::InThisClass(index) => {
                                (this_class, &this_class.methods[*index])
                            }
                            VtableIndex::OtherClass { class, index } => {
                                (class, &class.methods[*index])
                            }
                            VtableIndex::OtherInterface { class, index } => {
                                (class, &class.methods[*index])
                            }
                        };
                        println!("invokevirtual {}.{:?}", this_class.class_name, method.name);

                        Self::new_frame_with_method_info(
                            &mut self.top_frame,
                            Arc::clone(&class),
                            method,
                            pc + 1,
                            true,
                        );
                    }
                    pc = 0;
                }
                Next::InvokeStatic { class, index } => {
                    self.top_frame = Some(frame);
                    Self::new_frame_resolved(&mut self.top_frame, class, index, pc + 1, false);
                    pc = 0;
                    self.print_frames();
                }
            }
        }
        Ok(())
    }

    fn handle_exception(
        &mut self,
        exception: Exception,
        mut frame: Frame,
        pc: &mut usize,
    ) -> NativeResult<()> {
        // TODO: if this return exception, attach the original stack
        let (exp_class, obj_ref) = match exception {
            // TODO: change to UserException, put stack in
            Exception::VmException {
                ref exception_type, ..
            } => (Arc::clone(exception_type), 0),
            Exception::UserException(obj_ref) => (
                Arc::clone(global::HEAP.read().unwrap().get(obj_ref).get_class()),
                obj_ref,
            ),
        };

        let mut handler = -1;
        for item in &frame.exception_table {
            if !(item.start_pc as usize <= *pc && *pc < item.end_pc as usize) {
                continue;
            }
            if let Some(cp_class) = &item.catch_type {
                let bootstrap_class_loader = BOOTSTRAP_CLASS_LOADER.get().unwrap();
                let handler_class = cp_class
                    .get_or_load_class(|| bootstrap_class_loader.resolve_class(&cp_class.name))?;
                if !is_same_or_sub_class_of(&exp_class, &handler_class) {
                    continue;
                }
                handler = item.handler_pc as i32;
            } else {
                handler = item.handler_pc as i32;
                break;
            }
        }
        if handler == -1 {
            if let Some(frame) = self.top_frame.take()
                && !frame.is_dummy()
            {
                // return address
                // SAFETY: the first two must be return address
                let upper = unsafe { frame.stack[0].return_address } as usize;
                let lower = unsafe { frame.stack[1].return_address } as usize;
                *pc = (upper << 32) | lower;

                return self.handle_exception(exception, frame, pc);
            }
            return Err(exception);
        } else {
            *pc = handler as usize;
            frame.stack.clear();
            frame.stack.push(Variable { reference: obj_ref });
            self.top_frame = Some(frame);
        }

        Ok(())
    }

    pub fn print_frames(&self) {
        let mut cur = Some(self);
        while let Some(t) = cur {
            let mut frame = t.top_frame.as_ref();
            while let Some(f) = frame {
                print!("{}.{}[(", f.class.class_name, f.method_name);
                for field in &f.param_descriptor {
                    print!("{field}, ");
                }
                print!(")");
                if let Some(ret) = &f.return_type {
                    print!(" -> {ret}");
                }
                print!("] <- ");
                frame = f.previous_frame.as_deref();
            }
            cur = t.previous_thread;
        }
        println!()
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
