use crate::descriptor::FieldType;
use crate::runtime::{NativeEnv, NativeResult, NativeVariable, native::NATIVE_FUNCTIONS};

// private static native boolean isDumpingClassList0();
fn is_dumping_class_list0(_env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    Ok(Some(NativeVariable::Boolean(false)))
}

// private static native boolean isDumpingArchive0();
fn is_dumping_archive0(_env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    Ok(Some(NativeVariable::Boolean(false)))
}
// private static native boolean isSharingEnabled0();
fn is_sharing_enabled0(_env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    Ok(Some(NativeVariable::Boolean(false)))
}

// public static native long getRandomSeedForDumping();
fn get_random_seed_for_dumping(_env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    Ok(Some(NativeVariable::Long(0)))
}

// public static native void initializeFromArchive(Class<?> c);
fn initialize_from_archive(_env: NativeEnv) -> NativeResult<Option<NativeVariable>> {
    Ok(None)
}

pub(super) fn register_natives() {
    NATIVE_FUNCTIONS.insert(
        (
            "jdk/internal/misc/CDS".to_string(),
            "isDumpingClassList0".to_string(),
            vec![],
        ),
        is_dumping_class_list0,
    );
    NATIVE_FUNCTIONS.insert(
        (
            "jdk/internal/misc/CDS".to_string(),
            "isDumpingArchive0".to_string(),
            vec![],
        ),
        is_dumping_archive0,
    );
    NATIVE_FUNCTIONS.insert(
        (
            "jdk/internal/misc/CDS".to_string(),
            "isSharingEnabled0".to_string(),
            vec![],
        ),
        is_sharing_enabled0,
    );
    NATIVE_FUNCTIONS.insert(
        (
            "jdk/internal/misc/CDS".to_string(),
            "getRandomSeedForDumping".to_string(),
            vec![],
        ),
        get_random_seed_for_dumping,
    );
    NATIVE_FUNCTIONS.insert(
        (
            "jdk/internal/misc/CDS".to_string(),
            "initializeFromArchive".to_string(),
            vec![FieldType::Object("java/lang/Class".to_string())],
        ),
        initialize_from_archive,
    );
}
