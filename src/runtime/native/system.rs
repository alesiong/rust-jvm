use crate::{
    descriptor::FieldType,
    runtime::{
        Exception, NativeEnv, NativeResult, NativeVariable, Object,
        inheritance::{get_array_type, is_array_assignable_to},
        native::NATIVE_FUNCTIONS,
        structs::get_array_index,
    },
};
use std::sync::LazyLock;

//     public static native void arraycopy(Object src,  int  srcPos,
//                                         Object dest, int destPos,
//                                         int length);
fn native_system_arraycopy(env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    let src_ref = env.args[0].get_ref();
    let src_pos = env.args[1].get_int();
    let dest_ref = env.args[2].get_ref();
    let dest_pos = env.args[3].get_int();
    let mut length = env.args[4].get_int();

    // TODO: check bound and type
    if dest_ref == 0 || src_ref == 0 {
        return Err(Exception::new("java/lang/NullPointerException"));
    }
    let src = env.heap.read().unwrap().get(src_ref);
    let dest = env.heap.read().unwrap().get(dest_ref);
    let Some(src_type) = get_array_type(src.get_class()) else {
        return Err(Exception::new("java/lang/ArrayStoreException"));
    };
    let Some(dest_type) = get_array_type(dest.get_class()) else {
        return Err(Exception::new("java/lang/ArrayStoreException"));
    };
    if (src_type.is_primitive() || dest_type.is_primitive()) && src_type != dest_type {
        return Err(Exception::new("java/lang/ArrayStoreException"));
    }

    let src_ele_size = src_type.get_field_type_size();
    let src_len = src.get_array_size(src_ele_size);
    let dest_ele_size = dest_type.get_field_type_size();
    let dest_len = dest.get_array_size(dest_ele_size);

    debug_assert_eq!(dest_ele_size, src_ele_size, "dest_ele_size != src_ele_size");

    if src_pos < 0
        || dest_pos < 0
        || length < 0
        || src_pos + length > src_len as i32
        || dest_pos + length > dest_len as i32
    {
        return Err(Exception::new("java/lang/IndexOutOfBoundsException"));
    }

    let mut arr_store_exception = None;

    if !src_type.is_primitive() {
        for i in src_pos..src_pos + length {
            let ele_ref: u32 = unsafe { get_array_index(src.as_ref(), i as usize) };
            if ele_ref == 0 {
                continue;
            }
            let src_ele = env.heap.read().unwrap().get(ele_ref);
            if !is_array_assignable_to(
                src_ele.get_class(),
                dest.get_class()
                    .array_element_type
                    .as_ref()
                    .expect("must be array"),
            ) {
                length = i - src_pos;
                arr_store_exception = Some(Exception::new("java/lang/IndexOutOfBoundsException"));
                break;
            }
        }
    }

    // TODO: support special object
    let real_src = unsafe {
        src.as_heap_object()
            .expect("must be array")
            .get_u8_array()
            .add(src_pos as usize * src_ele_size)
    };
    let real_dest = unsafe {
        dest.as_heap_object()
            .expect("must be array")
            .get_u8_array()
            .add(dest_pos as usize * dest_ele_size)
    };
    let real_length = length as usize * src_ele_size;
    if src_ref == dest_ref {
        unsafe {
            real_dest.copy_from(real_src, real_length);
        }
    } else {
        unsafe {
            real_dest.copy_from_nonoverlapping(real_src, real_length);
        }
    }
    if let Some(e) = arr_store_exception {
        return Err(e);
    }

    Ok(None)
}

// public static native long currentTimeMillis();
fn current_time_millis(_env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    Ok(Some(NativeVariable::Long(millis)))
}

// public static native long nanoTime();
fn nano_time(_env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    static INSTANT_BASE: LazyLock<std::time::Instant> = LazyLock::new(std::time::Instant::now);
    let nanos = INSTANT_BASE.elapsed().as_nanos() as i64;
    Ok(Some(NativeVariable::Long(nanos)))
}

fn native_system_register_natives(_env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/System".to_string(),
            "arraycopy".to_string(),
            vec![
                FieldType::Object("java/lang/Object".to_string()),
                FieldType::Int,
                FieldType::Object("java/lang/Object".to_string()),
                FieldType::Int,
                FieldType::Int,
            ],
        ),
        native_system_arraycopy,
    );
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/System".to_string(),
            "currentTimeMillis".to_string(),
            vec![],
        ),
        current_time_millis,
    );

    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/System".to_string(),
            "nanoTime".to_string(),
            vec![],
        ),
        nano_time,
    );
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/System".to_string(),
            "identityHashCode".to_string(),
            vec![FieldType::Object("java/lang/Object".to_string())],
        ),
        super::object::native_object_hash_code,
    );

    Ok(None)
}

pub(super) fn register_natives() {
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/System".to_string(),
            "registerNatives".to_string(),
            vec![],
        ),
        native_system_register_natives,
    );
}
