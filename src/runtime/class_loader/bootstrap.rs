use crate::class::parser;
use crate::runtime::AttributeInfo;
use crate::{descriptor, runtime};
use dashmap::{DashMap, Entry};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
pub(in crate::runtime) struct BootstrapClassLoader {
    rt_path: PathBuf,
    modules: Vec<Module>,
    class_registry: DashMap<String, Arc<runtime::Class>>,
}

#[derive(Debug)]
struct Module {
    name: String,
    module_info: runtime::Class,
    packages: HashSet<Arc<str>>,
}

impl BootstrapClassLoader {
    pub(in crate::runtime) fn new(rt_path: impl Into<PathBuf>, modules: &[&str]) -> Self {
        let path = rt_path.into();
        Self {
            modules: modules
                .iter()
                .map(|name| load_module(&path, name))
                .collect(),
            rt_path: path,
            class_registry: DashMap::new(),
        }
    }

    pub(in crate::runtime) fn resolve_class(&self, class_name: &str) -> Arc<runtime::Class> {
        if let Some(class) = self.class_registry.get(class_name) {
            return Arc::clone(&class);
        }
        let mut need_init = false;
        let class = match self.class_registry.entry(class_name.to_string()) {
            Entry::Occupied(entry) => Arc::clone(entry.get()),
            Entry::Vacant(entry) => {
                let class = Arc::new(load_class(&self.rt_path, &self.modules, class_name));
                println!("loaded {}", class_name);
                entry.insert(Arc::clone(&class));
                need_init = true;
                class
            }
        };
        if need_init {
            // execute clinit
            if let Some(clinit) = class.methods.iter().find(|m| m.name.as_ref() == "<clinit>") {
                println!("clinit found for {:?}", clinit);
                let mut init_thread = runtime::Thread::new(1024);
                init_thread.new_frame(
                    Arc::clone(&class),
                    &clinit.name,
                    &clinit.descriptor.parameters,
                    0,
                );
                init_thread.execute();
            }
        }
        class
    }
}

fn load_class(rt_path: &Path, modules: &[Module], name: &str) -> runtime::Class {
    let package = if let Some((pkg, _)) = name.rsplit_once('/') {
        pkg
    } else {
        ""
    };
    for module in modules {
        if !module.packages.contains(package) {
            continue;
        }
        return parse_class(&rt_path.join(&module.name).join(name.to_string() + ".class"));
    }

    panic!("class not found {}", name)
}

fn load_module(path: &Path, name: &str) -> Module {
    let module_info_path = path.join(name).join("module-info.class");
    let module_info = parse_class(&module_info_path);

    let mut packages = HashSet::new();

    for attr in &module_info.attributes {
        if let AttributeInfo::ModulePackages(pks) = attr {
            packages.extend(pks.iter().map(Arc::clone));
        }
    }
    Module {
        name: name.to_string(),
        module_info,
        packages,
    }
}

fn parse_class(path: &Path) -> runtime::Class {
    // TODO: unwrap
    let file = fs::read(path).unwrap();
    let (_, cls) = parser::class_file(&file).unwrap();
    runtime::parse_class(&cls)
}
