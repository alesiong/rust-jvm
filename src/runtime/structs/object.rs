use crate::runtime::{Class, Variable};
use std::alloc::{alloc, Layout};
use std::cell::UnsafeCell;
use std::ptr::addr_of_mut;
use std::sync::Arc;

#[repr(C)]
#[derive(Debug)]
pub struct Object {
    class: Arc<Class>,
    fields: [UnsafeCell<Variable>],
}

// SAFETY: actually not safe, but JVM allows data race over objects
unsafe impl Sync for Object {}

impl Object {
    /// # Safety
    ///
    /// Must ensure there is no concurrent read/write
    pub unsafe fn put_field(&self, index: usize, v: Variable) {
        self.fields[index].get().write(v);
    }

    /// # Safety
    ///
    /// Must ensure there is no concurrent write
    pub unsafe fn get_field(&self, index: usize) -> Variable {
        *self.fields[index].get()
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
    pub unsafe fn allocate<F>(&mut self, size: usize, class: Arc<Class>, mut init_fields: F) -> u32
    where
        F: FnMut(usize, *mut Variable),
    {
        assert!(self.next_id < u32::MAX - 1, "heap oom");
        let id = self.next_id;
        let (layout, _) = Layout::new::<Arc<Class>>()
            .extend(Layout::array::<UnsafeCell<Variable>>(size).unwrap())
            .unwrap();
        let layout = layout.pad_to_align();
        let ptr = alloc(layout);
        let ptr = std::ptr::slice_from_raw_parts_mut(ptr, size) as *mut Object;
        addr_of_mut!((*ptr).class).write(class);
        let slice_ptr = unsafe { addr_of_mut!((*ptr).fields) as *mut UnsafeCell<Variable> };

        for i in 0..size {
            init_fields(i, (*slice_ptr.add(i)).get());
        }
        self.heap.push(Some(Box::from_raw(ptr).into()));
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
