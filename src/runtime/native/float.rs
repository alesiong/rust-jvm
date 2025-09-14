use crate::{
    descriptor::FieldType,
    runtime::native::{NativeEnv, NativeResult, NativeVariable, NATIVE_FUNCTIONS},
};

pub fn register_natives() {
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/Float".to_string(),
            "floatToRawIntBits".to_string(),
            vec![FieldType::Float],
        ),
        float_to_raw_int_bits,
    );
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/Float".to_string(),
            "intBitsToFloat".to_string(),
            vec![FieldType::Int],
        ),
        int_bits_to_float,
    );
}

fn float_to_raw_int_bits(env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    let value = env.args[0].get_float();
    let bits = value.to_bits() as i32;
    Ok(Some(NativeVariable::Int(bits)))
}

fn int_bits_to_float(env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    let bits = env.args[0].get_int();
    let value = f32::from_bits(bits as u32);
    Ok(Some(NativeVariable::Float(value)))
}