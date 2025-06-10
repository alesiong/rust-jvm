use crate::runtime::{Class, Variable};
use std::alloc::{Layout, alloc};
use std::cell::UnsafeCell;
use std::mem::{ManuallyDrop, forget};
use std::ptr::addr_of_mut;
use std::sync::Arc;

#[repr(C)]
#[derive(Debug)]
pub struct Object {
    class: Arc<Class>,
    // fields: [Variable]
    // array: [i8], [i16], etc.
    fields_or_array: UnsafeCell<[u8]>,
}

// SAFETY: actually not safe, but JVM allows data race over objects
unsafe impl Sync for Object {}

impl Object {
    /// # Safety
    ///
    /// Must ensure that this object is not array
    /// Must ensure there is no concurrent read/write
    pub unsafe fn put_field(&self, index: usize, v: Variable) {
        unsafe {
            (*self.get_object_fields())[index] = v;
        }
    }

    /// # Safety
    ///
    /// Must ensure that this object is not array
    /// Must ensure there is no concurrent write
    pub unsafe fn get_field(&self, index: usize) -> Variable {
        unsafe { (*self.get_object_fields())[index] }
    }

    /// # Safety
    ///
    /// Must ensure that this object is array of type T
    /// Must ensure there is no concurrent read/write
    #[allow(private_bounds)]
    pub unsafe fn put_array_index<T: ArrayType>(&self, index: usize, v: T) {
        unsafe {
            let array = self.get_array_fields::<T>();
            (*array)[index] = v;
        }
    }

    /// # Safety
    ///
    /// Must ensure that this object is array of type T
    /// Must ensure there is no concurrent read/write
    #[allow(private_bounds)]
    pub unsafe fn get_array_index<T: ArrayType>(&self, index: usize) -> T {
        unsafe {
            let array = self.get_array_fields::<T>();
            (*array)[index]
        }
    }

    /// # Safety
    ///
    /// Must ensure that this object is not array
    unsafe fn get_object_fields(&self) -> *mut [Variable] {
        let fields = unsafe { &mut *self.fields_or_array.get() };
        std::ptr::slice_from_raw_parts_mut(
            fields as *mut [u8] as *mut Variable,
            fields.len() / size_of::<Variable>(),
        )
    }

    /// # Safety
    ///
    /// Must ensure that this object is array of type T
    unsafe fn get_array_fields<T>(&self) -> *mut [T] {
        let fields = unsafe { &mut *self.fields_or_array.get() };
        std::ptr::slice_from_raw_parts_mut(
            fields as *mut [u8] as *mut T,
            fields.len() / size_of::<T>(),
        )
    }
}

pub struct Heap {
    heap: Vec<Option<Arc<Object>>>,
    next_id: u32,
}

impl Heap {
    pub const fn new() -> Heap {
        Heap {
            heap: vec![],
            next_id: 0,
        }
    }

    /// # Safety
    ///
    /// `init_fields` must write legal `Variable`
    pub unsafe fn allocate_object(
        &mut self,
        size: usize,
        class: Arc<Class>,
        init_fields: impl FnMut(usize, *mut Variable),
    ) -> u32 {
        unsafe { self.allocate(size, class, init_fields) }
    }

    #[allow(private_bounds)]
    pub fn allocate_array<T: ArrayType>(&mut self, size: usize, class: Arc<Class>) -> u32 {
        unsafe { self.allocate::<T>(size, class, |_, v| v.write(T::default())) }
    }

    unsafe fn allocate<T>(
        &mut self,
        size: usize,
        class: Arc<Class>,
        mut init_fields: impl FnMut(usize, *mut T),
    ) -> u32 {
        assert!(self.next_id < u32::MAX - 1, "heap oom");
        let id = self.next_id;
        let (layout, _) = Layout::new::<Arc<Class>>()
            .extend(Layout::array::<UnsafeCell<T>>(size).unwrap())
            .unwrap();
        let layout = layout.pad_to_align();
        let ptr = unsafe { alloc(layout) };
        let ptr = std::ptr::slice_from_raw_parts_mut(ptr, size * size_of::<T>()) as *mut Object;
        unsafe {
            addr_of_mut!((*ptr).class).write(class);
        }
        let slice_ptr = unsafe { addr_of_mut!((*ptr).fields_or_array) as *mut T };

        for i in 0..size {
            init_fields(i, unsafe { slice_ptr.add(i) });
        }
        let object = unsafe { Some(Box::from_raw(ptr).into()) };
        if self.heap.len() <= id as usize {
            self.heap.resize(id as usize + 1, None);
        }
        self.heap[id as usize] = object;
        while (self.next_id as usize) < self.heap.len()
            && self.heap[self.next_id as usize].is_some()
        {
            self.next_id += 1;
        }
        id + 1
    }

    pub fn deallocate(&mut self, id: u32) {
        self.heap[(id - 1) as usize].take();
        self.next_id = id;
    }

    pub fn get(&self, id: u32) -> Arc<Object> {
        Arc::clone(
            self.heap[(id - 1) as usize]
                .as_ref()
                .expect("unavailable object id"),
        )
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

trait ArrayType: Default + Copy {}
impl ArrayType for i8 {}
impl ArrayType for u16 {}
impl ArrayType for f32 {}
impl ArrayType for f64 {}
impl ArrayType for i16 {}
impl ArrayType for i32 {}
impl ArrayType for i64 {}
impl ArrayType for u32 {}
