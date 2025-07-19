use crate::runtime::{
    ArrayType, Class, Object, SpecialStringObject, StringTable, StringTableEntry, Variable,
    heap::reflection::{ClassTable, SpecialClassObject},
    structs::ObjectMonitor,
};
use std::{
    alloc::{Layout, alloc},
    cell::UnsafeCell,
    ptr::addr_of_mut,
    sync::Arc,
};

pub mod reflection;
pub mod string_table;

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
            .extend(Layout::new::<ObjectMonitor>())
            .unwrap()
            .0
            .extend(Layout::array::<UnsafeCell<T>>(size).unwrap())
            .unwrap();
        let layout = layout.pad_to_align();
        let ptr = unsafe { alloc(layout) };
        let ptr = std::ptr::slice_from_raw_parts_mut(ptr, size * size_of::<T>()) as *mut HeapObject;
        unsafe {
            addr_of_mut!((*ptr).class).write(class);
            addr_of_mut!((*ptr).monitor).write(ObjectMonitor::default());
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

    pub(in crate::runtime) fn get(&self, id: u32) -> Arc<dyn Object> {
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

    pub(in crate::runtime) fn clone(&mut self, obj: &dyn Object) -> u32 {
        if let Some(obj) = obj.as_heap_object() {
            return self.clone_object(obj);
        }
        // TODO: exception, support clone for array
        panic!("not allow clone")
    }

    fn clone_object(&mut self, obj: &HeapObject) -> u32 {
        assert!(self.next_id < Self::MAX_OBJECT_ID - 1, "heap oom");

        let layout = Layout::for_value(obj);
        let ptr = unsafe { alloc(layout) };
        let u8_size = obj.get_u8_array_size();
        let ptr = std::ptr::slice_from_raw_parts_mut(ptr, u8_size) as *mut HeapObject;
        unsafe {
            addr_of_mut!((*ptr).class).write(Arc::clone(&obj.class));
            addr_of_mut!((*ptr).monitor).write(ObjectMonitor::default());
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
    ) -> u32 {
        if let Some(entry) = string_table.map.get(&string) {
            return entry.string_id;
        }

        assert!(
            self.special_heap.next_id + 1 < Self::MAX_OBJECT_ID - 1,
            "heap oom"
        );

        let bytes_obj = Box::new(SpecialStringObject::Bytes {
            monitor: ObjectMonitor::new(),
            bytes: Arc::clone(&string),
        });
        let bytes_id = allocate_id_for_obj(
            &mut self.special_heap.heap,
            &mut self.special_heap.next_id,
            Box::into_raw(bytes_obj),
        ) | Self::MAX_OBJECT_ID;

        let string_obj = Box::new(SpecialStringObject::String {
            monitor: ObjectMonitor::new(),
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

    pub fn get_class_object(&mut self, class: Arc<Class>, class_table: &mut ClassTable) -> u32 {
        let class_name = Arc::clone(&class.class_name);
        if let Some(entry) = class_table.map.get(&class_name) {
            return *entry;
        }
        assert!(
            self.special_heap.next_id < Self::MAX_OBJECT_ID - 1,
            "heap oom"
        );

        let class_obj = Box::new(SpecialClassObject {
            class,
            monitor: ObjectMonitor::default(),
            name_str: Default::default(),
            package_name_str: Default::default(),
        });

        let class_id = allocate_id_for_obj(
            &mut self.special_heap.heap,
            &mut self.special_heap.next_id,
            Box::into_raw(class_obj),
        ) | Self::MAX_OBJECT_ID;

        class_table.map.insert(class_name, class_id);

        class_id
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
#[repr(C)]
#[derive(Debug)]
pub(in crate::runtime) struct HeapObject {
    class: Arc<Class>,
    monitor: ObjectMonitor,
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
            "element_size invalid: {element_size}"
        );
        debug_assert_eq!(v.len(), element_size, "v.len() != element_size");
        let array = unsafe { &mut *self.fields_or_array.get() };
        array[index * element_size..(index + 1) * element_size].copy_from_slice(v);
    }

    unsafe fn get_array_index_raw(&self, index: usize, element_size: usize) -> &[u8] {
        debug_assert_eq!(
            self.get_u8_array_size() % element_size,
            0,
            "element_size invalid: {element_size}"
        );
        let array = unsafe { &mut *self.fields_or_array.get() };
        &array[index * element_size..(index + 1) * element_size]
    }

    fn get_array_size(&self, element_size: usize) -> usize {
        let u8_size = self.get_u8_array_size();
        debug_assert_eq!(
            u8_size % element_size,
            0,
            "element_size invalid: {element_size}, u8_size: {u8_size}"
        );

        u8_size / element_size
    }

    unsafe fn get_u8_array_const(&self) -> *const u8 {
        self.get_u8_array() as *const u8
    }

    fn get_monitor(&self) -> &ObjectMonitor {
        &self.monitor
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

pub(in crate::runtime) trait SpecialObject: Object {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{
        gen_array_class,
        structs::{get_array_index, put_array_index},
    };

    #[test]
    fn test_ordinary_object() {
        let mut heap = Heap::new();
        let id = unsafe { heap.allocate_object(2, get_class(), |i, v| *v = Variable { int: 0 }) };
        let object = heap.get(id);
        unsafe {
            object.put_field(1, Variable { reference: 1 });
            assert_eq!(object.get_field(0).int, 0);
            assert_eq!(object.get_field(1).reference, 1);
        }
        heap.deallocate(id);
    }

    #[test]
    fn test_ordinary_array() {
        let mut heap = Heap::new();
        let id = heap.allocate_array::<i8>(2, get_class());
        let object = heap.get(id);
        unsafe {
            object.put_array_index_raw(1, &[1], 1);
            assert_eq!(object.get_array_index_raw(0, 1), &[0]);
            assert_eq!(object.get_array_index_raw(1, 1), &[1]);
        }
        heap.deallocate(id);
    }

    #[test]
    fn test_multibyte_array() {
        let mut heap = Heap::new();
        let id = heap.allocate_array::<i32>(2, get_class());
        let object = heap.get(id);
        unsafe {
            put_array_index(object.as_ref(), 1, 1i32);
            assert_eq!(get_array_index::<i32, _>(object.as_ref(), 1), 1i32);
            assert_eq!(object.get_array_index_raw(0, 4), &[0, 0, 0, 0]);
        }
        heap.deallocate(id);
    }

    fn get_class() -> Arc<Class> {
        let class = gen_array_class(Arc::from("[I"));

        Arc::new(class)
    }
}
