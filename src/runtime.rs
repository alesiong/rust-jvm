mod class_loader;
mod interpreter;
mod native;
mod structs;

use crate::runtime::global::BOOTSTRAP_CLASS_LOADER;
pub use class_loader::*;
pub use interpreter::*;
pub(crate) use structs::*;

pub use native::*;

pub fn init_bootstrap_class_loader(modules: Vec<Box<dyn ModuleLoader + Send + Sync + 'static>>) {
    let mut bootstrap_class_loader = BootstrapClassLoader::new();
    for module in modules {
        bootstrap_class_loader.add_module(module);
    }
    BOOTSTRAP_CLASS_LOADER.set(bootstrap_class_loader).unwrap()
}
