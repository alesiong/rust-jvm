use crate::{
    descriptor::FieldType,
    runtime::native::{NativeEnv, NativeResult, NativeVariable, NATIVE_FUNCTIONS},
};

pub fn register_natives() {
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/Double".to_string(),
            "doubleToRawLongBits".to_string(),
            vec![FieldType::Double],
        ),
        double_to_raw_long_bits,
    );
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/Double".to_string(),
            "longBitsToDouble".to_string(),
            vec![FieldType::Long],
        ),
        long_bits_to_double,
    );
}

fn double_to_raw_long_bits(env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    let value = env.args[0].get_double();
    let bits = value.to_bits() as i64;
    Ok(Some(NativeVariable::Long(bits)))
}

fn long_bits_to_double(env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    let bits = env.args[0].get_long();
    let value = f64::from_bits(bits as u64);
    Ok(Some(NativeVariable::Double(value)))
}