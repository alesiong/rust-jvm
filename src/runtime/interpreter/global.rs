use std::sync::{LazyLock, OnceLock, RwLock};

use crate::runtime::{StringTable, class_loader::BootstrapClassLoader, heap::Heap};
use crate::runtime::heap::reflection::ClassTable;

pub(in crate::runtime) static HEAP: RwLock<Heap> = RwLock::new(Heap::new());
pub(in crate::runtime) static STRING_TABLE: LazyLock<RwLock<StringTable>> =
    LazyLock::new(|| RwLock::new(StringTable::new()));
pub(in crate::runtime) static CLASS_TABLE: LazyLock<RwLock<ClassTable>> =
    LazyLock::new(|| RwLock::new(ClassTable::new()));

pub(in crate::runtime) static BOOTSTRAP_CLASS_LOADER: OnceLock<BootstrapClassLoader> =
    OnceLock::new();
