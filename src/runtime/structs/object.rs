use crate::runtime::structs::string_table::{SpecialStringObject, StringTable, StringTableEntry};
use crate::runtime::{Class, Variable};
use std::alloc::{Layout, alloc};
use std::cell::UnsafeCell;
use std::ptr::addr_of_mut;
use std::sync::Arc;
use std::{mem, slice};

pub trait Object {
    fn get_class(&self) -> &Arc<Class>;

    /// # Safety
    ///
    /// Must ensure that this object is not array
    /// Must ensure there is no concurrent read/write
    unsafe fn put_field(&self, index: usize, v: Variable);

    /// # Safety
    ///
    /// Must ensure that this object is not array
    /// Must ensure there is no concurrent write
    unsafe fn get_field(&self, index: usize) -> Variable;

    fn as_heap_object(&self) -> Option<&HeapObject> {
        None
    }

    /// # Safety
    ///
    /// Must ensure that this object is array with element of size element_size
    /// Must ensure there is no concurrent read/write
    unsafe fn put_array_index_raw(&self, index: usize, v: &[u8], element_size: usize);

    /// # Safety
    ///
    /// Must ensure that this object is array with element of size element_size
    /// Must ensure there is no concurrent write
    unsafe fn get_array_index_raw(&self, index: usize, element_size: usize) -> &[u8];

    fn get_array_size(&self, element_size: usize) -> usize;
}

/// # Safety
///
/// Must ensure that this object is array of type T
/// Must ensure there is no concurrent read/write
#[allow(private_bounds)]
pub(in crate::runtime) unsafe fn put_array_index<O, T>(obj: &O, index: usize, v: T)
where
    O: Object + ?Sized,
    T: ArrayType,
{
    unsafe {
        let v_ptr = mem::transmute::<*const T, *const u8>(&v as *const T);
        let element_size = size_of::<T>();
        let v_ref = slice::from_raw_parts(v_ptr, element_size);
        obj.put_array_index_raw(index, v_ref, element_size)
    }
}

/// # Safety
///
/// Must ensure that this object is array of type T
/// Must ensure there is no concurrent write
#[allow(private_bounds)]
pub(in crate::runtime) unsafe fn get_array_index<O, T>(obj: &O, index: usize) -> T
where
    O: Object + ?Sized,
    T: ArrayType,
{
    unsafe {
        let element_size = size_of::<T>();
        let bytes = obj.get_array_index_raw(index, element_size);
        let mut res = T::default();
        (&mut res as *mut T)
            .copy_from_nonoverlapping(bytes as *const [u8] as *const _, element_size);
        res
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct HeapObject {
    class: Arc<Class>,
    // fields: [Variable]
    // array: [i8], [i16], etc.
    fields_or_array: UnsafeCell<[u8]>,
}

// SAFETY: actually not safe, but JVM allows data race over objects
unsafe impl Sync for HeapObject {}

impl Object for Box<HeapObject> {
    fn get_class(&self) -> &Arc<Class> {
        &self.class
    }

    /// # Safety
    ///
    /// Must ensure that this object is not array
    /// Must ensure there is no concurrent read/write
    unsafe fn put_field(&self, index: usize, v: Variable) {
        unsafe {
            (*self.get_object_fields())[index] = v;
        }
    }

    /// # Safety
    ///
    /// Must ensure that this object is not array
    /// Must ensure there is no concurrent write
    unsafe fn get_field(&self, index: usize) -> Variable {
        unsafe { (*self.get_object_fields())[index] }
    }

    fn as_heap_object(&self) -> Option<&HeapObject> {
        Some(self)
    }

    unsafe fn put_array_index_raw(&self, index: usize, v: &[u8], element_size: usize) {
        debug_assert_eq!(
            self.get_u8_array_size() % element_size,
            0,
            "element_size invalid: {}",
            element_size
        );
        debug_assert_eq!(v.len(), element_size, "v.len() != element_size");
        let array = unsafe { &mut *self.fields_or_array.get() };
        array[index * element_size..(index + 1) * element_size].copy_from_slice(v);
    }

    unsafe fn get_array_index_raw(&self, index: usize, element_size: usize) -> &[u8] {
        debug_assert_eq!(
            self.get_u8_array_size() % element_size,
            0,
            "element_size invalid: {}",
            element_size
        );
        let array = unsafe { &mut *self.fields_or_array.get() };
        &array[index * element_size..(index + 1) * element_size]
    }

    fn get_array_size(&self, element_size: usize) -> usize {
        let u8_size = self.get_u8_array_size();
        debug_assert_eq!(
            u8_size % element_size,
            0,
            "element_size invalid: {}, u8_size: {}",
            element_size,
            u8_size
        );

        u8_size / element_size
    }
}

impl HeapObject {
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
    unsafe fn get_array_fields<T: ArrayType>(&self) -> *mut [T] {
        let fields = unsafe { &mut *self.fields_or_array.get() };
        std::ptr::slice_from_raw_parts_mut(
            fields as *mut [u8] as *mut T,
            fields.len() / size_of::<T>(),
        )
    }

    fn get_u8_array_size(&self) -> usize {
        let arr = self.fields_or_array.get();
        arr.len()
    }

    pub fn get_u8_array(&self) -> *mut u8 {
        self.fields_or_array.get() as *mut u8
    }
}

pub struct Heap {
    heap: Vec<Option<Arc<Box<HeapObject>>>>,
    next_id: u32,
    special_heap: SpecialHeap,
}

impl Heap {
    const MAX_OBJECT_ID: u32 = 0b10000000_00000000_00000000_00000000;

    pub const fn new() -> Heap {
        Heap {
            heap: vec![],
            next_id: 0,
            special_heap: SpecialHeap {
                heap: vec![],
                next_id: 0,
            },
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
        // upper half for special objects
        // TODO: error
        assert!(self.next_id < Self::MAX_OBJECT_ID - 1, "heap oom");
        let (layout, _) = Layout::new::<Arc<Class>>()
            .extend(Layout::array::<UnsafeCell<T>>(size).unwrap())
            .unwrap();
        let layout = layout.pad_to_align();
        let ptr = unsafe { alloc(layout) };
        let ptr = std::ptr::slice_from_raw_parts_mut(ptr, size * size_of::<T>()) as *mut HeapObject;
        unsafe {
            addr_of_mut!((*ptr).class).write(class);
        }
        let slice_ptr = unsafe { addr_of_mut!((*ptr).fields_or_array) as *mut T };

        for i in 0..size {
            init_fields(i, unsafe { slice_ptr.add(i) });
        }

        allocate_id_for_obj(
            &mut self.heap,
            &mut self.next_id,
            Box::into_raw(Box::new(unsafe { Box::from_raw(ptr) })),
        )
    }

    pub fn deallocate(&mut self, id: u32) {
        self.heap[(id - 1) as usize].take();
        self.next_id = id;
    }

    pub fn get(&self, id: u32) -> Arc<dyn Object> {
        if id & Self::MAX_OBJECT_ID == 0 {
            Arc::clone(
                self.heap[(id - 1) as usize]
                    .as_ref()
                    .expect("unavailable object id"),
            ) as Arc<dyn Object>
        } else {
            let id = id & !Self::MAX_OBJECT_ID;
            Arc::clone(
                self.special_heap.heap[(id - 1) as usize]
                    .as_ref()
                    .expect("unavailable object id"),
            ) as Arc<dyn Object>
        }
    }

    pub fn clone_object(&mut self, obj: &HeapObject) -> u32 {
        assert!(self.next_id < Self::MAX_OBJECT_ID - 1, "heap oom");

        let layout = Layout::for_value(obj);
        let ptr = unsafe { alloc(layout) };
        let u8_size = obj.get_u8_array_size();
        let ptr = std::ptr::slice_from_raw_parts_mut(ptr, u8_size) as *mut HeapObject;
        unsafe {
            addr_of_mut!((*ptr).class).write(Arc::clone(&obj.class));
        }

        unsafe {
            std::ptr::copy_nonoverlapping(
                obj.fields_or_array.get() as *const u8,
                addr_of_mut!((*ptr).fields_or_array) as *mut u8,
                u8_size,
            );
        }
        allocate_id_for_obj(
            &mut self.heap,
            &mut self.next_id,
            Box::into_raw(Box::new(unsafe { Box::from_raw(ptr) })),
        )
    }

    pub fn intern_string(
        &mut self,
        string: Arc<[u8]>,
        has_multi_bytes: bool,
        string_table: &mut StringTable,
        bytes_class: Arc<Class>,
        string_class: Arc<Class>,
    ) -> u32 {
        if let Some(entry) = string_table.map.get(&string) {
            return entry.string_id;
        }

        assert!(
            self.special_heap.next_id + 1 < Self::MAX_OBJECT_ID - 1,
            "heap oom"
        );

        let bytes_obj = Box::new(SpecialStringObject::Bytes {
            bytes_class,
            bytes: Arc::clone(&string),
        });
        let bytes_id = allocate_id_for_obj(
            &mut self.special_heap.heap,
            &mut self.special_heap.next_id,
            Box::into_raw(bytes_obj),
        ) | Self::MAX_OBJECT_ID;

        let string_obj = Box::new(SpecialStringObject::String {
            string_class,
            bytes_id,
            bytes: Arc::clone(&string),
            hash: 0,
            has_multi_bytes,
        });

        let string_id = allocate_id_for_obj(
            &mut self.special_heap.heap,
            &mut self.special_heap.next_id,
            Box::into_raw(string_obj),
        ) | Self::MAX_OBJECT_ID;

        let table_entry = StringTableEntry {
            string_id,
            bytes_id,
            hash: 0,
            has_multi_bytes: false,
        };

        string_table.map.insert(string, table_entry);

        string_id
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SpecialHeap {
    heap: Vec<Option<Arc<dyn SpecialObject + Sync + Send>>>,
    next_id: u32,
}

fn allocate_id_for_obj<T: ?Sized>(
    heap: &mut Vec<Option<Arc<T>>>,
    next_id: &mut u32,
    object_ptr: *mut T,
) -> u32 {
    let id = *next_id;
    let object = unsafe { Some(Box::from_raw(object_ptr).into()) };
    if heap.len() <= id as usize {
        heap.resize_with(id as usize + 1, || None);
    }
    heap[id as usize] = object;
    while (*next_id as usize) < heap.len() && heap[*next_id as usize].is_some() {
        *next_id += 1;
    }
    id + 1
}

pub(super) trait SpecialObject: Object {}

trait Sealed {}
impl Sealed for i8 {}
impl Sealed for u16 {}
impl Sealed for f32 {}
impl Sealed for f64 {}
impl Sealed for i16 {}
impl Sealed for i32 {}
impl Sealed for i64 {}
impl Sealed for u32 {}

pub trait ArrayType: Default + Copy + Sealed {}
impl ArrayType for i8 {}
impl ArrayType for u16 {}
impl ArrayType for f32 {}
impl ArrayType for f64 {}
impl ArrayType for i16 {}
impl ArrayType for i32 {}
impl ArrayType for i64 {}
impl ArrayType for u32 {}
