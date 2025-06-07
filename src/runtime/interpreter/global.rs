use std::sync::{Arc, LazyLock, RwLock};

use dashmap::DashMap;

use crate::runtime::{self, Heap};

pub(in crate::runtime) static HEAP: RwLock<Heap> = RwLock::new(Heap::new());
pub(in crate::runtime) static CLASS_REGISTRY: LazyLock<DashMap<String, Arc<runtime::Class>>> =
    LazyLock::new(DashMap::new);
