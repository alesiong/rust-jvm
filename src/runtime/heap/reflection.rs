use crate::{
    class::JavaStr,
    runtime::{Class, Object, Variable, famous_classes::CLASS_CLASS, heap::SpecialObject},
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering::Relaxed},
    },
};
use crate::runtime::structs::ObjectMonitor;

pub struct ClassTable {
    pub(in crate::runtime) map: HashMap<Arc<str>, u32>,
}

impl ClassTable {
    pub(in crate::runtime) fn new() -> Self {
        Self {
            map: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct SpecialClassObject {
    pub(in crate::runtime) class: Arc<Class>,
    pub(in crate::runtime) monitor: ObjectMonitor,
    pub(in crate::runtime) name_str: AtomicU32,
    pub(super) package_name_str: AtomicU32,
}

impl Object for SpecialClassObject {
    fn get_class(&self) -> &Arc<Class> {
        CLASS_CLASS.get().expect("class must be loaded")
    }

    unsafe fn put_field(&self, index: usize, v: Variable) {
        let field = self
            .get_class()
            .instance_fields_info
            .iter()
            .find(|f| f.index == index as _)
            .expect("invalid field");

        if field.name.as_ref() == JavaStr::from_str("packageName").as_ref() {
            // SAFETY: class verification guarantees that the field is a String
            self.package_name_str.store(unsafe { v.reference }, Relaxed);
        } else {
            panic!("invalid field");
        }
    }

    unsafe fn get_field(&self, index: usize) -> Variable {
        let field = self
            .get_class()
            .instance_fields_info
            .iter()
            .find(|f| f.index == index as _)
            .expect("invalid field");

        if field.name.as_ref() == JavaStr::from_str("name").as_ref() {
            Variable {
                reference: self.name_str.load(Relaxed),
            }
        } else if field.name.as_ref() == JavaStr::from_str("packageName").as_ref() {
            Variable {
                reference: self.package_name_str.load(Relaxed),
            }
        } else if field.name.as_ref() == JavaStr::from_str("classRedefinedCount").as_ref() {
            // TODO:
            Variable { int: 0 }
        } else if field.name.as_ref() == JavaStr::from_str("classLoader").as_ref() {
            // TODO: always bootstrap loader
            Variable { reference: 0 }
        } else {
            panic!("invalid field");
        }
    }

    unsafe fn put_array_index_raw(&self, _index: usize, _v: &[u8], _element_size: usize) {
        panic!("not array");
    }

    unsafe fn get_array_index_raw(&self, _index: usize, _element_size: usize) -> &[u8] {
        panic!("not array");
    }

    fn get_array_size(&self, _element_size: usize) -> usize {
        panic!("not array");
    }

    fn get_monitor(&self) -> &ObjectMonitor {
        &self.monitor
    }
}

impl SpecialObject for SpecialClassObject {}
