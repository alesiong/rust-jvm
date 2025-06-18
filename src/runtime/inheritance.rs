use crate::consts::ClassAccessFlag;
use crate::descriptor::{FieldDescriptor, FieldType, parse_field_descriptor};
use crate::runtime::{Class, Object};
use std::sync::Arc;

/// source: class of value to be assigned to array
/// target: class of *element* of the target array
pub(in crate::runtime) fn is_array_assignable_to(source: &Arc<Class>, target: &Arc<Class>) -> bool {
    if let Some(source_type) = get_array_type(source) {
        // source is array
        if let Some(target_type) = get_array_type(target) {
            // target is array
            if source_type.is_primitive() || target_type.is_primitive() {
                return source_type == target_type;
            }
            let source_arr_type = source
                .array_element_type
                .as_ref()
                .expect("must be reference array");
            let target_arr_type = target
                .array_element_type
                .as_ref()
                .expect("must be reference array");
            is_array_assignable_to(source_arr_type, target_arr_type)
        } else {
            // target is not array
            if target.access_flags.contains(ClassAccessFlag::INTERFACE) {
                target.class_name.as_ref() == "java/lang/Cloneable"
                    || target.class_name.as_ref() == "java/io/Serializable"
            } else {
                // target is class, must be Object
                target.class_name.as_ref() == "java/lang/Object"
            }
        }
    } else {
        // source is class
        if target.access_flags.contains(ClassAccessFlag::INTERFACE) {
            // target is interface
            is_class_implements(source, target)
        } else {
            is_same_or_sub_class_of(source, target)
        }
    }
}

pub(in crate::runtime) fn is_class_implements(class: &Arc<Class>, interface: &Arc<Class>) -> bool {
    for class_intf in &class.interfaces {
        if class_intf.class_name == interface.class_name {
            return true;
        }
    }
    if let Some(super_class) = &class.super_class {
        return is_class_implements(super_class, interface);
    }
    false
}

pub(in crate::runtime) fn is_same_or_sub_class_of(
    source: &Arc<Class>,
    target: &Arc<Class>,
) -> bool {
    if source.class_name == target.class_name {
        return true;
    }
    if let Some(super_class) = &source.super_class {
        return is_same_or_sub_class_of(super_class, target);
    }
    false
}

pub(in crate::runtime) fn get_array_type(class: &Arc<Class>) -> Option<FieldType> {
    if !class.is_array() {
        return None;
    }
    let (_, FieldDescriptor(field_type)) =
        parse_field_descriptor(&class.class_name).expect("invalid array type");
    let FieldType::Array(field_type) = field_type else {
        panic!("invalid array type");
    };
    Some(*field_type)
}

pub(in crate::runtime) fn get_array_len(object: &Object) -> usize {
    let field_type = get_array_type(&object.class).expect("not an array");
    object.get_u8_array_size() / field_type.get_field_type_size()
}
