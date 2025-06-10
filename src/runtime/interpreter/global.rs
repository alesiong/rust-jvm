use std::sync::{LazyLock, OnceLock, RwLock};

use crate::runtime::class_loader::BootstrapClassLoader;
use crate::runtime::Heap;

pub(in crate::runtime) static HEAP: RwLock<Heap> = RwLock::new(Heap::new());

pub(in crate::runtime) static BOOTSTRAP_CLASS_LOADER: OnceLock<BootstrapClassLoader> =
    OnceLock::new();
