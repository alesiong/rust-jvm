use std::sync::{LazyLock, OnceLock, RwLock};

use crate::runtime::class_loader::BootstrapClassLoader;
use crate::runtime::{Heap, StringTable};

pub(in crate::runtime) static HEAP: RwLock<Heap> = RwLock::new(Heap::new());
pub(in crate::runtime) static STRING_TABLE: LazyLock<RwLock<StringTable>> =
    LazyLock::new(|| RwLock::new(StringTable::new()));

pub(in crate::runtime) static BOOTSTRAP_CLASS_LOADER: OnceLock<BootstrapClassLoader> =
    OnceLock::new();
