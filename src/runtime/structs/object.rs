use crate::runtime::heap::HeapObject;
use crate::runtime::{Class, Variable};
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
pub(in crate::runtime) unsafe fn put_array_index<T, O>(obj: &O, index: usize, v: T)
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
pub(in crate::runtime) unsafe fn get_array_index<T, O>(obj: &O, index: usize) -> T
where
    O: Object + ?Sized,
    T: ArrayType,
{
    unsafe {
        let element_size = size_of::<T>();
        let bytes = obj.get_array_index_raw(index, element_size);
        let mut res = T::default();
        (&mut res as *mut T)
            .copy_from_nonoverlapping(bytes as *const [u8] as *const _, 1);
        res
    }
}

trait Sealed {}
impl Sealed for i8 {}
impl Sealed for u16 {}
impl Sealed for f32 {}
impl Sealed for f64 {}
impl Sealed for i16 {}
impl Sealed for i32 {}
impl Sealed for i64 {}
impl Sealed for u32 {}

#[allow(private_bounds)]
pub trait ArrayType: Default + Copy + Sealed {}
impl ArrayType for i8 {}
impl ArrayType for u16 {}
impl ArrayType for f32 {}
impl ArrayType for f64 {}
impl ArrayType for i16 {}
impl ArrayType for i32 {}
impl ArrayType for i64 {}
impl ArrayType for u32 {}
