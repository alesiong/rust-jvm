use crate::class::JavaStr;
use crate::runtime::heap::SpecialObject;
use crate::runtime::{Class, Object, Variable};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SpecialClassObject {
    class_class: Arc<Class>,
    class: Arc<Class>,
}

impl Object for SpecialClassObject {
    fn get_class(&self) -> &Arc<Class> {
        &self.class_class
    }

    unsafe fn put_field(&self, index: usize, v: Variable) {
        todo!()
    }

    unsafe fn get_field(&self, index: usize) -> Variable {
        let field = self
            .class_class
            .instance_fields_info
            .iter()
            .find(|f| f.index == index as _)
            .expect("invalid field");

        // if field.name.as_ref() == JavaStr::from_str("value").as_ref() {
        //     Variable {
        //         reference: *bytes_id,
        //     }
        // }
        todo!()
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
}

impl SpecialObject for SpecialClassObject {}
