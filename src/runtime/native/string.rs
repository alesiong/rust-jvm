use crate::runtime::{NativeEnv, NativeResult, NativeVariable, native::NATIVE_FUNCTIONS};

// private static native boolean isBigEndian();
fn native_stringutf16_isbegendian(_env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    if cfg!(target_endian = "big") {
        Ok(Some(NativeVariable::Boolean(true)))
    } else {
        Ok(Some(NativeVariable::Boolean(false)))
    }
}

pub(super) fn register_natives() {
    NATIVE_FUNCTIONS.insert(
        (
            "java/lang/StringUTF16".to_string(),
            "isBigEndian".to_string(),
            vec![],
        ),
        native_stringutf16_isbegendian,
    );
}
