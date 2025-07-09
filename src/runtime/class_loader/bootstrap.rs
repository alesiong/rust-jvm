use dashmap::DashMap;
use once_cell::sync::OnceCell;
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    fs::{self, File},
    io::{Read, Seek},
    mem,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use zip::{ZipArchive, read::ZipFile};

use crate::{
    class::{self, parser},
    consts::{ClassAccessFlag, MethodAccessFlag},
    descriptor::{FieldDescriptor, FieldType, parse_field_descriptor},
    runtime,
    runtime::{
        AttributeInfo, FieldResolve, MethodResolve, NativeResult, VtableEntry, VtableIndex,
        class_loader::{
            resolve_cp_class, resolve_from_vtable, resolve_method_statically_inner,
            resolve_static_field, resolve_static_method_inner,
        },
        gen_array_class,
    },
};

#[derive(Debug)]
pub(in crate::runtime) struct BootstrapClassLoader {
    modules: Vec<Box<dyn ModuleLoader + Send + Sync + 'static>>,
    package_to_module: HashMap<String, usize>,
    // TODO: use Arc<String>
    class_registry: DashMap<String, Arc<OnceCell<Arc<runtime::Class>>>>,
}

pub trait ModuleLoader: Debug {
    fn packages(&self) -> Vec<Arc<str>>;
    fn name(&self) -> &str;
    // must end with .class
    fn get_class_file(&self, class_name: &str) -> OwnedOrRef<'_, class::Class>;
}

impl BootstrapClassLoader {
    pub(in crate::runtime) fn new() -> Self {
        Self {
            modules: vec![],
            package_to_module: HashMap::new(),
            class_registry: Default::default(),
        }
    }
    pub fn add_module(&mut self, module: Box<dyn ModuleLoader + Send + Sync + 'static>) {
        for package in module.packages() {
            self.package_to_module
                .insert(package.to_string(), self.modules.len());
        }
        self.modules.push(module);
    }

    pub(in crate::runtime) fn resolve_class(
        &self,
        class_name: &str,
    ) -> NativeResult<Arc<runtime::Class>> {
        if class_name.starts_with('[') {
            let (_, FieldDescriptor(field_type)) = parse_field_descriptor(class_name).unwrap();
            return self.resolve_array_class_with_field_type(field_type);
        }
        let class_cell = Arc::clone(
            self.class_registry
                .entry(class_name.to_string())
                .or_default()
                .value(),
        );

        let class = class_cell.get_or_try_init(|| self.define_class(class_name))?;

        Ok(Arc::clone(class))
    }
    fn resolve_array_class_with_field_type(
        &self,
        filed_type: FieldType,
    ) -> NativeResult<Arc<runtime::Class>> {
        let FieldType::Array(element) = filed_type else {
            panic!("must be array");
        };
        match *element {
            FieldType::Char
            | FieldType::Double
            | FieldType::Float
            | FieldType::Int
            | FieldType::Long
            | FieldType::Short
            | FieldType::Byte
            | FieldType::Boolean => self.resolve_primitive_array_class(&element),

            FieldType::Object(class_name) => {
                self.resolve_object_array_class(&self.resolve_class(&class_name)?)
            }
            FieldType::Array(_) => self.resolve_array_class_with_field_type(*element),
        }
    }

    pub(in crate::runtime) fn resolve_primitive_array_class(
        &self,
        field_type: &FieldType,
    ) -> NativeResult<Arc<runtime::Class>> {
        let class_name_string = "[".to_string() + &field_type.to_descriptor();
        let class_name: Arc<str> = Arc::from(class_name_string.as_str());
        let class_cell = Arc::clone(
            self.class_registry
                .entry(class_name_string)
                .or_default()
                .value(),
        );
        let class = class_cell.get_or_try_init(|| self.define_array(class_name, None))?;

        // array has no clinit
        Ok(Arc::clone(class))
    }

    pub(in crate::runtime) fn resolve_object_array_class(
        &self,
        ele_class: &Arc<runtime::Class>,
    ) -> NativeResult<Arc<runtime::Class>> {
        let class_name_string = "[".to_string() + &ele_class.class_name;
        let class_name: Arc<str> = Arc::from(class_name_string.as_str());
        let class_cell = Arc::clone(
            self.class_registry
                .entry(class_name_string)
                .or_default()
                .value(),
        );
        let class =
            class_cell.get_or_try_init(|| self.define_array(class_name, Some(ele_class)))?;

        // array has no clinit
        Ok(Arc::clone(class))
    }

    fn define_class(&self, name: &str) -> NativeResult<Arc<runtime::Class>> {
        let package = if let Some((pkg, _)) = name.rsplit_once('/') {
            pkg
        } else {
            ""
        };
        // TODO: unwrap
        let module_id = self.package_to_module.get(package).unwrap();
        let module = &self.modules[*module_id];

        let class_file = &module.get_class_file(&(name.to_string() + ".class"));
        let mut class = runtime::parse_class(class_file);
        self.load_super_class(&mut class, class_file.super_class)?;
        self.load_interfaces(&mut class, &class_file.interfaces)?;

        Self::resolve_this_class_field_ref(&mut class);
        Self::build_vtable(&mut class);

        let class = Arc::new(class);
        Self::resolve_this_class_field_ref_static(&class);
        Self::resolve_this_class_method_ref_static(&class);
        Self::resolve_this_class_method_ref(&class);

        println!("defined {name}");

        println!("vtable:");
        for entry in &class.vtable {
            print!(
                "{}.{}: ",
                entry
                    .root_class
                    .as_ref()
                    .map(|c| c.class_name.as_ref())
                    .unwrap_or(""),
                entry.name.to_str()
            );
            match &entry.index {
                VtableIndex::InThisClass(index) => {
                    println!("{index}");
                }
                VtableIndex::OtherClass { class, index } => {
                    println!("{}: {index}", class.class_name);
                }
                VtableIndex::OtherInterface { class, index } => {
                    println!("{}: {index}", class.class_name);
                }
            }
        }
        println!();

        Ok(class)
    }

    fn define_array(
        &self,
        class_name: Arc<str>,
        ele_class: Option<&Arc<runtime::Class>>,
    ) -> NativeResult<Arc<runtime::Class>> {
        let mut class = gen_array_class(class_name);

        class.super_class = Some(self.resolve_class("java/lang/Object")?);
        class
            .interfaces
            .push(self.resolve_class("java/lang/Cloneable")?);
        class
            .interfaces
            .push(self.resolve_class("java/io/Serializable")?);
        class.array_element_type = ele_class.map(Arc::clone);
        // TODO: vtable

        Ok(Arc::new(class))
    }

    fn load_super_class(&self, class: &mut runtime::Class, class_index: u16) -> NativeResult<()> {
        // java.lang.Object
        if class_index == 0 {
            return Ok(());
        }
        let super_class = resolve_cp_class(&class.constant_pool, class_index);
        let loaded = self.resolve_class(&super_class.name)?;
        super_class.set_class(&loaded);
        class.super_class.replace(Arc::clone(&loaded));
        Ok(())
    }
    fn load_interfaces(&self, class: &mut runtime::Class, interfaces: &[u16]) -> NativeResult<()> {
        for index in interfaces {
            let interface = resolve_cp_class(&class.constant_pool, *index);
            let loaded = self.resolve_class(&interface.name)?;
            interface.set_class(&loaded);
            class.interfaces.push(loaded);
        }
        Ok(())
    }
    fn resolve_this_class_field_ref(class: &mut runtime::Class) {
        // allocates field index for instance fields
        let mut instance_field_num = class
            .super_class
            .as_ref()
            .and_then(|s| s.instance_fields_info.last())
            .map(|f| {
                1 + if f.descriptor.0.is_long() {
                    f.index + 1
                } else {
                    f.index
                }
            })
            .unwrap_or(0);
        for field_info in class.instance_fields_info.iter_mut() {
            field_info.index = instance_field_num;
            if field_info.descriptor.0.is_long() {
                instance_field_num += 2;
            } else {
                instance_field_num += 1;
            }
        }

        // set up map, with fields in current class overwriting fields in super class
        let field_map: HashMap<_, _> = class
            .super_class
            .as_ref()
            .map(|s| &s.instance_fields_info)
            .into_iter()
            .flatten()
            .chain(&class.instance_fields_info)
            .map(|field| ((field.name.as_ref(), &field.descriptor), field.index))
            .collect();

        for cp_in_file in &class.constant_pool {
            let runtime::ConstantPoolInfo::Fieldref(field_ref) = cp_in_file else {
                continue;
            };

            if field_ref.class_name != class.class_name {
                // not in this class, to be resolved at runtime
                continue;
            }
            let name_and_type = &field_ref.name_and_type;

            let key = &(name_and_type.name.as_ref(), &name_and_type.descriptor);

            let index = field_map.get(key);
            if let Some(&index) = index {
                // inside this class
                field_ref
                    .resolve
                    .set(FieldResolve::InThisClass(index))
                    .expect("must be empty now");
            }
            // not found, must be a static field or an error
        }

        let total_field_len = class.instance_fields_info.len()
            + class
                .super_class
                .as_ref()
                .map(|s| s.instance_fields_info.len())
                .unwrap_or(0);
        let instance_fields = mem::replace(
            &mut class.instance_fields_info,
            Vec::with_capacity(total_field_len),
        );
        class.instance_fields_info.extend(
            class
                .super_class
                .as_ref()
                .map(|s| &s.instance_fields_info)
                .into_iter()
                .flatten()
                .cloned(),
        );
        class.instance_fields_info.extend(instance_fields);
    }

    fn resolve_this_class_field_ref_static(class: &Arc<runtime::Class>) {
        let field_map: HashMap<_, _> = class
            .static_fields_info
            .iter()
            .map(|field| ((field.name.as_ref(), &field.descriptor), field.index))
            .collect();

        // for filter
        let instance_field: HashSet<_> = class
            .instance_fields_info
            .iter()
            .map(|field| (field.name.as_ref(), &field.descriptor))
            .collect();

        for cp_in_file in &class.constant_pool {
            let runtime::ConstantPoolInfo::Fieldref(field_ref) = cp_in_file else {
                continue;
            };

            if field_ref.class_name != class.class_name {
                // not in this class, to be resolved at runtime
                continue;
            }
            let name_and_type = &field_ref.name_and_type;

            let key = &(name_and_type.name.as_ref(), &name_and_type.descriptor);
            if instance_field.contains(key) {
                // ignore instance field
                continue;
            }

            let index = field_map.get(key);
            if let Some(&index) = index {
                // inside this class
                field_ref
                    .resolve
                    .set(FieldResolve::InThisClass(index))
                    .expect("must be empty now");
            } else {
                let Some(resolve) = resolve_static_field(class, field_ref, true) else {
                    // instance fields from super class must be put into instance_field_info before this function
                    // TODO: exception ?
                    panic!("static field cannot be resolved");
                };
                field_ref.resolve.set(resolve).expect("must be empty now");
            }
        }
    }

    fn resolve_this_class_method_ref_static(class: &Arc<runtime::Class>) {
        let method_map: HashMap<_, _> = class
            .methods
            .iter()
            .enumerate()
            .filter(|(_, m)| m.access_flags.contains(MethodAccessFlag::STATIC))
            .map(|(i, method)| ((method.name.as_ref(), &method.descriptor), i))
            .collect();

        for cp_in_file in &class.constant_pool {
            let runtime::ConstantPoolInfo::Methodref(method_ref) = cp_in_file else {
                continue;
            };

            if method_ref.class_name != class.class_name {
                // not in this class, to be resolved at runtime
                continue;
            }
            let name_and_type = &method_ref.name_and_type;

            let key = &(name_and_type.name.as_ref(), &name_and_type.descriptor);

            let index = method_map.get(key);
            if let Some(&index) = index {
                // inside this class
                method_ref
                    .resolve
                    .set(MethodResolve::InThisClass {
                        index,
                        vtable_index: -1,
                    })
                    .expect("must be empty now");
            } else if let Some(resolve) = resolve_static_method_inner(class, method_ref, true) {
                method_ref.resolve.set(resolve).expect("must be empty now");
            }
            // if not found, must be a non-static method, resolve at runtime
        }
    }

    fn resolve_this_class_method_ref(class: &Arc<runtime::Class>) {
        let method_map: HashMap<_, _> = class
            .methods
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.access_flags.contains(MethodAccessFlag::STATIC))
            .map(|(i, method)| ((method.name.as_ref(), &method.descriptor), i))
            .collect();

        for cp_in_file in &class.constant_pool {
            let runtime::ConstantPoolInfo::Methodref(method_ref) = cp_in_file else {
                continue;
            };
            // already resolved (static)
            if method_ref.resolve.get().is_some() {
                continue;
            }

            if method_ref.class_name != class.class_name {
                // not in this class, to be resolved at runtime
                continue;
            }
            let name_and_type = &method_ref.name_and_type;

            let key = &(name_and_type.name.as_ref(), &name_and_type.descriptor);

            let index = method_map.get(key);

            if let Some(&index) = index {
                let method = &class.methods[index];
                let vtable_index = resolve_from_vtable(class, method);

                // inside this class
                method_ref
                    .resolve
                    .set(MethodResolve::InThisClass {
                        index,
                        vtable_index,
                    })
                    .expect("must be empty now");
            } else if let Some(resolve) = resolve_method_statically_inner(class, method_ref, true) {
                method_ref.resolve.set(resolve).expect("must be empty now");
            }
            // if not found, must be a non-static method, resolve at runtime
        }
    }

    fn build_vtable(class: &mut runtime::Class) {
        if let Some(super_class) = &class.super_class {
            // super class's vtable goes first
            class.vtable.extend(super_class.vtable.iter().cloned());
        }
        // interface
        if class.access_flags.contains(ClassAccessFlag::INTERFACE) {
            // interface will only have Object's vtable
            return;
        }

        let mut vtable = mem::take(&mut class.vtable);
        // instance methods
        let method_map: HashMap<_, _> = class
            .methods
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.access_flags.contains(MethodAccessFlag::STATIC))
            .filter(|(_, m)| !m.access_flags.contains(MethodAccessFlag::PRIVATE))
            .filter(|(_, m)| m.name.to_str() != "<init>")
            .map(|(i, method)| ((method.name.to_java_string(), method.descriptor.clone()), i))
            .collect();

        let mut overrode_methods = HashSet::new();

        for entry in &mut vtable {
            // check for overrides
            let (super_class, index) = match &entry.index {
                VtableIndex::InThisClass(index) => {
                    // must have super class
                    let index = *index;
                    let class = class.super_class.as_ref().unwrap();
                    entry.index = VtableIndex::OtherClass {
                        class: Arc::clone(class),
                        index,
                    };

                    (class, index)
                }
                VtableIndex::OtherClass { class, index } => (class as &_, *index),
                VtableIndex::OtherInterface { class, index } => (class as &_, *index),
            };

            entry
                .root_class
                .get_or_insert_with(|| Arc::clone(super_class));

            let super_method = &super_class.methods[index];

            // skip non overridable
            // private and final method will not be in vtable
            // TODO: check transitive overridable
            if !super_method.access_flags.contains(MethodAccessFlag::PUBLIC)
                && !super_method
                    .access_flags
                    .contains(MethodAccessFlag::PROTECTED)
                && super_class.package_name() != class.package_name()
            {
                continue;
            }

            let key = (
                super_method.name.to_java_string(),
                super_method.descriptor.clone(),
            );

            if let Some(&self_index) = method_map.get(&key) {
                entry.index = VtableIndex::InThisClass(self_index);
            }
            overrode_methods.insert(key);
        }

        // put new methods in the end
        if !class.access_flags.contains(ClassAccessFlag::FINAL) {
            for (i, method) in class.methods.iter().enumerate() {
                if method.access_flags.contains(MethodAccessFlag::FINAL) {
                    // final method is statically dispatched
                    continue;
                }
                let key = (method.name.to_java_string(), method.descriptor.clone());
                if !method_map.contains_key(&key) {
                    continue;
                }
                // package private method always has a new entry
                if overrode_methods.contains(&key)
                    && (method.access_flags.contains(MethodAccessFlag::PUBLIC)
                        || method.access_flags.contains(MethodAccessFlag::PROTECTED))
                {
                    continue;
                }
                vtable.push(VtableEntry {
                    root_class: None,
                    name: Arc::clone(&method.name),
                    descriptor: method.descriptor.clone(),
                    index: VtableIndex::InThisClass(i),
                });
            }
        }

        // TODO: interface hierarchy
        // put interface methods
        for interface in &class.interfaces {
            for (i, interface_method) in interface.methods.iter().enumerate() {
                // private/static method is not inheritable
                if interface_method
                    .access_flags
                    .contains(MethodAccessFlag::PRIVATE)
                {
                    continue;
                }
                if interface_method
                    .access_flags
                    .contains(MethodAccessFlag::STATIC)
                {
                    continue;
                }
                // add default or abstract method if not overrode
                let key = (
                    interface_method.name.to_java_string(),
                    interface_method.descriptor.clone(),
                );
                if method_map.contains_key(&key) {
                    continue;
                }
                vtable.push(VtableEntry {
                    root_class: Some(Arc::clone(interface)),
                    name: Arc::clone(&interface_method.name),
                    descriptor: interface_method.descriptor.clone(),
                    index: VtableIndex::OtherInterface {
                        class: Arc::clone(interface),
                        index: i,
                    },
                });
            }
        }

        class.vtable = vtable;
    }
}

#[derive(Debug)]
pub struct JModModule {
    name: String,
    module_info: runtime::Class,
    zip_file: Mutex<ZipArchive<File>>,
}

impl JModModule {
    pub fn new(java_home: impl AsRef<Path>, module_name: impl Into<String>) -> JModModule {
        let module_name = module_name.into();
        let jmod_path = java_home
            .as_ref()
            .join("jmods")
            .join(module_name.to_string() + ".jmod");
        // TODO: unwrap
        let mod_file = File::open(jmod_path).unwrap();
        let mut archive = ZipArchive::new(mod_file).unwrap();

        // load module info
        let mut module_info_file = archive.by_name("classes/module-info.class").unwrap();
        let module_info = Self::get_class_bytes(&mut module_info_file);
        drop(module_info_file);
        let module_info = parser::class_file(&module_info).unwrap();
        let module_info = runtime::parse_class(&module_info);

        JModModule {
            name: module_name,
            zip_file: Mutex::new(archive),
            module_info,
        }
    }

    fn get_class_bytes<R: Read + Seek>(class_file: &mut ZipFile<R>) -> Vec<u8> {
        // TODO: unwrap
        let mut content = Vec::with_capacity(class_file.size() as usize);
        class_file.read_to_end(&mut content).unwrap();
        content
    }
}

impl ModuleLoader for JModModule {
    fn packages(&self) -> Vec<Arc<str>> {
        self.module_info
            .attributes
            .iter()
            .filter_map(|attr| match attr {
                AttributeInfo::ModulePackages(pkg) => {
                    Some(pkg.iter().map(|s| Arc::clone(s).to_str_arc()))
                }
                _ => None,
            })
            .flatten()
            .collect()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn get_class_file(&self, class_name: &str) -> OwnedOrRef<'_, class::Class> {
        let mut archive = self.zip_file.lock().unwrap();
        let mut class_file = archive.by_name(&format!("classes/{class_name}")).unwrap();
        let class_bytes = Self::get_class_bytes(&mut class_file);
        drop(class_file);
        drop(archive);

        let class_file = parser::class_file(&class_bytes).expect(class_name);
        class_file.into()
    }
}

#[derive(Debug)]
pub struct ClassPathModule {
    name: String,
    base_path: PathBuf,
}

impl ClassPathModule {
    pub fn new(name: impl Into<String>, base_path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            base_path: base_path.into().canonicalize().expect("must be directory"),
        }
    }
}

impl ModuleLoader for ClassPathModule {
    fn packages(&self) -> Vec<Arc<str>> {
        // TODO: unwrap
        let mut packages = HashSet::new();
        fn traverse(path: &Path, packages: &mut HashSet<String>, base_path: &Path) {
            if !path.is_dir() {
                return;
            }
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    traverse(&path, packages, base_path);
                } else if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "class" {
                            let dir_name = path
                                .parent()
                                .and_then(Path::to_str)
                                .unwrap_or("")
                                .to_string();
                            let package_name = dir_name
                                .strip_prefix(base_path.to_str().expect("must be utf-8"))
                                .expect("must have base path as prefix")
                                .to_string();

                            packages.insert(
                                package_name
                                    .strip_prefix('/')
                                    .unwrap_or(&package_name)
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }
        traverse(&self.base_path, &mut packages, &self.base_path);

        packages.into_iter().map(Into::into).collect()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn get_class_file(&self, class_name: &str) -> OwnedOrRef<'_, class::Class> {
        // TODO: unwrap
        let class_file = fs::read(self.base_path.join(class_name)).unwrap();
        let class_file = parser::class_file(&class_file).unwrap();
        class_file.into()
    }
}

pub enum OwnedOrRef<'a, T> {
    Owned(T),
    Ref(&'a T),
}

impl<T> Deref for OwnedOrRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            OwnedOrRef::Owned(o) => o,
            OwnedOrRef::Ref(r) => r,
        }
    }
}

impl<T> From<T> for OwnedOrRef<'_, T> {
    fn from(o: T) -> Self {
        OwnedOrRef::Owned(o)
    }
}

impl<'a, T> From<&'a T> for OwnedOrRef<'a, T> {
    fn from(r: &'a T) -> Self {
        OwnedOrRef::Ref(r)
    }
}
