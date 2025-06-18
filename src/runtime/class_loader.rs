use crate::runtime::{FieldResolve, Fieldref, MethodInfo, Module, ModuleExport, Variable};
use crate::{
    class,
    descriptor::{
        self, FieldDescriptor, MethodDescriptor, parse_field_descriptor, parse_method_descriptor,
        parse_return_type_descriptor,
    },
    runtime::{
        self, Annotation, Const, CpClassInfo, CpNameAndTypeInfo, ElementValuePair, FieldInfo,
    },
};
use nom::{
    IResult, Parser,
    bytes::complete::take,
    error_position,
    multi::count,
    number::complete::{be_u16, be_u32, u8},
};
use parking_lot::ReentrantMutex;
use std::cell::Cell;
use std::collections::HashMap;
use std::convert::identity;
use std::sync::{Arc, RwLock};

use super::{ElementValue, LocalVariable};

mod bootstrap;
use crate::class::JavaStr;
use crate::consts::{ClassAccessFlag, FieldAccessFlag};
use crate::descriptor::FieldType;
use crate::runtime::structs::ClinitStatus;

pub(super) use bootstrap::BootstrapClassLoader;
pub use bootstrap::{ClassPathModule, JModModule, ModuleLoader};

pub fn parse_class(class_file: &class::Class) -> runtime::Class {
    let constant_pool = parse_constant_pool(&class_file.constant_pool);

    let (mut static_fields, instance_fields): (Vec<_>, Vec<_>) = class_file
        .fields
        .iter()
        .map(|f| parse_field(&constant_pool, f))
        .partition(|f| f.access_flags.contains(FieldAccessFlag::STATIC));
    let methods: Vec<MethodInfo> = class_file
        .methods
        .iter()
        .map(|m| parse_method(&constant_pool, m))
        .collect();
    let attributes = class_file
        .attributes
        .iter()
        .map(convert_attribute(&constant_pool))
        .collect();

    let class_name = Arc::clone(&resolve_cp_class(&constant_pool, class_file.this_class).name);

    let static_fields_var = allocate_static_fields(&mut static_fields);

    runtime::Class {
        access_flags: class_file.access_flags,
        class_name: Arc::clone(&class_name),
        super_class: None,
        interfaces: Vec::with_capacity(class_file.interfaces.len()),
        static_fields_info: static_fields,
        instance_fields_info: instance_fields,
        methods,
        attributes,
        constant_pool,
        array_element_type: None,
        static_fields: static_fields_var,
        clinit_call: ReentrantMutex::new(Cell::new(ClinitStatus::NotInit)),
    }
}

pub fn gen_array_class(class_name: Arc<str>) -> runtime::Class {
    runtime::Class {
        access_flags: ClassAccessFlag::PUBLIC | ClassAccessFlag::FINAL | ClassAccessFlag::SYNTHETIC,
        class_name,
        super_class: None,
        interfaces: Vec::with_capacity(2),
        static_fields_info: vec![],
        instance_fields_info: vec![],
        methods: vec![],
        attributes: vec![],
        constant_pool: vec![],
        static_fields: vec![],
        array_element_type: None,
        // array has no clinit
        clinit_call: ReentrantMutex::new(Cell::new(ClinitStatus::Init)),
    }
}

fn parse_constant_pool(cp: &Vec<class::ConstantPoolInfo>) -> Vec<runtime::ConstantPoolInfo> {
    let mut constant_pool = Vec::with_capacity(cp.len());
    let mut class_info_map = HashMap::new();
    for (i, cp_info) in cp.iter().enumerate() {
        if let class::ConstantPoolInfo::Class { name_index } = cp_info {
            class_info_map.insert(
                i as u16 + 1,
                CpClassInfo {
                    name: resolve_cp_utf8(cp, *name_index).to_str_arc(),
                    class: Default::default(),
                },
            );
        }
    }

    for cp_info in cp {
        type Cpi = runtime::ConstantPoolInfo;
        let constant_pool_info = match cp_info {
            class::ConstantPoolInfo::Utf8(v) => Cpi::Utf8(Arc::clone(v)),
            class::ConstantPoolInfo::Integer(v) => Cpi::Integer(*v),
            class::ConstantPoolInfo::Float(v) => Cpi::Float(*v),
            class::ConstantPoolInfo::Long(v) => Cpi::Long(*v),
            class::ConstantPoolInfo::Double(v) => Cpi::Double(*v),
            class::ConstantPoolInfo::Class { name_index } => Cpi::Class(CpClassInfo {
                name: resolve_cp_utf8(cp, *name_index).to_str_arc(),
                class: Default::default(),
            }),
            class::ConstantPoolInfo::String { string_index } => {
                Cpi::String(resolve_cp_utf8(cp, *string_index))
            }
            class::ConstantPoolInfo::Fieldref {
                class_index,
                name_and_type_index,
            } => Cpi::Fieldref(Fieldref {
                class_name: Arc::clone(&class_info_map[class_index].name),
                name_and_type: resolve_cp_name_and_type_field(cp, *name_and_type_index),
                resolve: Default::default(),
            }),
            class::ConstantPoolInfo::Methodref {
                class_index,
                name_and_type_index,
            } => Cpi::Methodref {
                class: class_info_map[class_index].clone(),
                name_and_type: resolve_cp_name_and_type_method(cp, *name_and_type_index),
            },
            class::ConstantPoolInfo::InterfaceMethodref {
                class_index,
                name_and_type_index,
            } => Cpi::InterfaceMethodref {
                class: class_info_map[class_index].clone(),
                name_and_type: resolve_cp_name_and_type_method(cp, *name_and_type_index),
            },
            class::ConstantPoolInfo::NameAndType {
                name_index,
                descriptor_index,
            } => Cpi::NameAndType(resolve_cp_name_and_type(cp, *name_index, *descriptor_index)),
            // TODO: fill
            class::ConstantPoolInfo::MethodHandle { .. } => Cpi::MethodHandle,
            class::ConstantPoolInfo::MethodType { .. } => Cpi::MethodType,
            class::ConstantPoolInfo::Dynamic { .. } => Cpi::Dynamic,
            class::ConstantPoolInfo::InvokeDynamic { .. } => Cpi::InvokeDynamic,
            class::ConstantPoolInfo::Module { name_index } => {
                Cpi::Module(resolve_cp_utf8(cp, *name_index))
            }
            class::ConstantPoolInfo::Package { name_index } => {
                Cpi::Package(resolve_cp_utf8(cp, *name_index))
            }
            class::ConstantPoolInfo::Empty => Cpi::Empty,
        };
        constant_pool.push(constant_pool_info);
    }

    constant_pool
}

fn parse_field(cp: &[runtime::ConstantPoolInfo], field: &class::FieldInfo) -> runtime::FieldInfo {
    let descriptor = resolve_runtime_cp_utf8(cp, field.descriptor_index);
    let (_, descriptor) = parse_field_descriptor(&descriptor.to_str()).unwrap();
    runtime::FieldInfo {
        access_flags: field.access_flags,
        name: resolve_runtime_cp_utf8(cp, field.name_index),
        descriptor,
        attributes: field.attributes.iter().map(convert_attribute(cp)).collect(),
        index: 0,
    }
}

fn parse_method(
    cp: &[runtime::ConstantPoolInfo],
    method: &class::MethodInfo,
) -> runtime::MethodInfo {
    let descriptor = resolve_runtime_cp_utf8(cp, method.descriptor_index);
    let (_, descriptor) = parse_method_descriptor(&descriptor.to_str()).unwrap();
    runtime::MethodInfo {
        access_flags: method.access_flags,
        name: resolve_runtime_cp_utf8(cp, method.name_index),
        descriptor,
        attributes: method
            .attributes
            .iter()
            .map(convert_attribute(cp))
            .collect(),
    }
}

fn convert_attribute(
    constant_pool: &[runtime::ConstantPoolInfo],
) -> impl FnMut(&class::AttributeInfo) -> runtime::AttributeInfo + '_ {
    move |a| {
        // TODO: unwrap
        parse_attribute(a.attribute_name_index, &a.info, constant_pool)
            .unwrap()
            .1
    }
}

fn resolve_cp_utf8(constant_pool: &[class::ConstantPoolInfo], index: u16) -> Arc<JavaStr> {
    let class::ConstantPoolInfo::Utf8(string) = &constant_pool[index as usize - 1] else {
        panic!("cannot find string {}", index);
    };
    Arc::clone(string)
}

fn resolve_runtime_cp_utf8(
    constant_pool: &[runtime::ConstantPoolInfo],
    index: u16,
) -> Arc<JavaStr> {
    let runtime::ConstantPoolInfo::Utf8(string) = &constant_pool[index as usize - 1] else {
        panic!("cannot find string {}", index);
    };
    Arc::clone(string)
}

fn resolve_cp_package(constant_pool: &[runtime::ConstantPoolInfo], index: u16) -> Arc<JavaStr> {
    let runtime::ConstantPoolInfo::Package(name) = &constant_pool[index as usize - 1] else {
        panic!("cannot find package {}", index);
    };
    Arc::clone(name)
}

fn resolve_cp_module(constant_pool: &[runtime::ConstantPoolInfo], index: u16) -> Arc<JavaStr> {
    let runtime::ConstantPoolInfo::Module(name) = &constant_pool[index as usize - 1] else {
        panic!("cannot find module {}", index);
    };
    Arc::clone(name)
}

fn resolve_cp_class(constant_pool: &[runtime::ConstantPoolInfo], class_index: u16) -> &CpClassInfo {
    let runtime::ConstantPoolInfo::Class(cp_class_info) = &constant_pool[class_index as usize - 1]
    else {
        panic!("cannot find class {}", class_index);
    };
    cp_class_info
}

fn resolve_cp_name_and_type_field(
    constant_pool: &[class::ConstantPoolInfo],
    index: u16,
) -> CpNameAndTypeInfo<FieldDescriptor> {
    let class::ConstantPoolInfo::NameAndType {
        name_index,
        descriptor_index,
    } = &constant_pool[index as usize - 1]
    else {
        panic!("cannot find name_and_type {}", index);
    };

    let name = resolve_cp_utf8(constant_pool, *name_index);
    let descriptor = resolve_cp_utf8(constant_pool, *descriptor_index);

    // TODO: unwrap
    let (_, descriptor) =
        descriptor::parse_field_descriptor(&descriptor.to_str()).expect("invalid descriptor");

    CpNameAndTypeInfo::<FieldDescriptor> {
        name: Arc::clone(&name),
        descriptor,
    }
}

fn resolve_cp_name_and_type_method(
    constant_pool: &[class::ConstantPoolInfo],
    index: u16,
) -> CpNameAndTypeInfo<MethodDescriptor> {
    let class::ConstantPoolInfo::NameAndType {
        name_index,
        descriptor_index,
    } = &constant_pool[index as usize - 1]
    else {
        panic!("cannot find name_and_type {}", index);
    };

    let name = resolve_cp_utf8(constant_pool, *name_index);
    let descriptor = resolve_cp_utf8(constant_pool, *descriptor_index);

    // TODO: unwrap
    let (_, descriptor) =
        descriptor::parse_method_descriptor(&descriptor.to_str()).expect("invalid descriptor");

    CpNameAndTypeInfo::<MethodDescriptor> {
        name: Arc::clone(&name),
        descriptor,
    }
}

fn resolve_cp_name_and_type(
    constant_pool: &[class::ConstantPoolInfo],
    name_index: u16,
    descriptor_index: u16,
) -> CpNameAndTypeInfo<Arc<JavaStr>> {
    let name = resolve_cp_utf8(constant_pool, name_index);
    let descriptor = resolve_cp_utf8(constant_pool, descriptor_index);
    CpNameAndTypeInfo::<Arc<JavaStr>> { name, descriptor }
}

fn resolve_constant_value(constant_pool: &[runtime::ConstantPoolInfo], index: u16) -> Const {
    match &constant_pool[index as usize - 1] {
        runtime::ConstantPoolInfo::Integer(i) => Const::Int(*i),
        runtime::ConstantPoolInfo::Float(f) => Const::Float(*f),
        runtime::ConstantPoolInfo::Long(l) => Const::Long(*l),
        runtime::ConstantPoolInfo::Double(d) => Const::Double(*d),
        runtime::ConstantPoolInfo::String(str) => Const::String(Arc::clone(str)),
        runtime::ConstantPoolInfo::Utf8(string) => Const::String(Arc::clone(string)),
        _ => {
            panic!("cannot find constant_value {}", index);
        }
    }
}

fn parse_attribute<'a>(
    attribute_name_index: u16,
    mut input: &'a [u8],
    constant_pool: &[runtime::ConstantPoolInfo],
) -> IResult<&'a [u8], runtime::AttributeInfo> {
    // TODO: move this to parser
    let attribute_name = resolve_runtime_cp_utf8(constant_pool, attribute_name_index);

    let attribute_info = match attribute_name.to_str().as_ref() {
        "Code" => {
            let attribute_info;
            (input, attribute_info) = parse_code_attribute(input, constant_pool)?;
            attribute_info
        }
        "ConstantValue" => {
            let constantvalue_index;
            (input, constantvalue_index) = be_u16(input)?;
            runtime::AttributeInfo::ConstantValue(resolve_constant_value(
                constant_pool,
                constantvalue_index,
            ))
        }
        "RuntimeVisibleAnnotations" => {
            let (num_annotations, annotations);
            (input, num_annotations) = be_u16(input)?;
            (input, annotations) =
                count(parse_annotation(constant_pool), num_annotations as _).parse(input)?;

            runtime::AttributeInfo::RuntimeVisibleAnnotations(annotations)
        }
        "LocalVariableTable" => {
            let (local_variable_table_length, local_variable_table);
            (input, local_variable_table_length) = be_u16(input)?;
            (input, local_variable_table) = count(
                parse_local_variable(constant_pool),
                local_variable_table_length as _,
            )
            .parse(input)?;

            runtime::AttributeInfo::LocalVariableTable(local_variable_table)
        }
        "Signature" => {
            let signature_index;
            (input, signature_index) = be_u16(input)?;
            runtime::AttributeInfo::Signature(resolve_runtime_cp_utf8(
                constant_pool,
                signature_index,
            ))
        }
        "Deprecated" => runtime::AttributeInfo::Deprecated,
        // TODO: only used for verification
        "StackMapTable" => runtime::AttributeInfo::StackMapTable(vec![]),
        // TODO: checked exception only
        "Exceptions" => runtime::AttributeInfo::Exceptions,
        "SourceFile" => {
            let sourcefile_index;
            (input, sourcefile_index) = be_u16(input)?;
            runtime::AttributeInfo::SourceFile(resolve_runtime_cp_utf8(
                constant_pool,
                sourcefile_index,
            ))
        }
        "LineNumberTable" => {
            let (line_number_table_length, line_number_table);
            (input, line_number_table_length) = be_u16(input)?;
            (input, line_number_table) = count(
                |input| {
                    let (input, start_pc) = be_u16(input)?;
                    let (input, line_number) = be_u16(input)?;
                    Ok((
                        input,
                        runtime::LineNumberTableItem {
                            start_pc,
                            line_number,
                        },
                    ))
                },
                line_number_table_length as _,
            )
            .parse(input)?;
            runtime::AttributeInfo::LineNumberTable(line_number_table)
        }
        "Module" => {
            let (module_name_index, module_flags, module_version_index);
            (input, module_name_index) = be_u16(input)?;
            (input, module_flags) = be_u16(input)?;
            (input, module_version_index) = be_u16(input)?;

            let (requires_count, requires);
            (input, requires_count) = be_u16(input)?;
            (input, requires) = count(
                |input| {
                    let (input, requires_index) = be_u16(input)?;
                    let (input, requires_flags) = be_u16(input)?;
                    let (input, requires_version_index) = be_u16(input)?;
                    Ok((input, ()))
                },
                requires_count as _,
            )
            .parse(input)?;

            let (exports_count, exports);
            (input, exports_count) = be_u16(input)?;
            (input, exports) = count(
                |input| {
                    let (input, exports_index) = be_u16(input)?;
                    let (input, exports_flags) = be_u16(input)?;
                    let (input, exports_to_count) = be_u16(input)?;
                    let (input, exports_to_index) =
                        count(be_u16, exports_to_count as _).parse(input)?;

                    Ok((
                        input,
                        ModuleExport {
                            exports: resolve_cp_package(constant_pool, exports_index),
                            exports_flags,
                            exports_to: exports_to_index
                                .iter()
                                .map(|index| resolve_cp_module(constant_pool, *index))
                                .collect(),
                        },
                    ))
                },
                exports_count as _,
            )
            .parse(input)?;

            let (opens_count, opens);
            (input, opens_count) = be_u16(input)?;
            (input, opens) = count(
                |input| {
                    let (input, opens_index) = be_u16(input)?;
                    let (input, opens_flags) = be_u16(input)?;
                    let (input, opens_to_count) = be_u16(input)?;
                    let (input, opens_to_index) =
                        count(be_u16, opens_to_count as _).parse(input)?;
                    Ok((input, ()))
                },
                opens_count as _,
            )
            .parse(input)?;

            let (uses_count, uses_index);
            (input, uses_count) = be_u16(input)?;
            (input, uses_index) = count(be_u16, uses_count as _).parse(input)?;

            let (provides_count, provides);
            (input, provides_count) = be_u16(input)?;
            (input, provides) = count(
                |input| {
                    let (input, provides_index) = be_u16(input)?;
                    let (input, provides_with_count) = be_u16(input)?;
                    let (input, provides_with_index) =
                        count(be_u16, provides_with_count as _).parse(input)?;
                    Ok((input, ()))
                },
                provides_count as _,
            )
            .parse(input)?;
            runtime::AttributeInfo::Module(Module {
                // TODO:
                exports,
            })
        }
        "ModulePackages" => {
            let (package_count, package_index);
            (input, package_count) = be_u16(input)?;
            (input, package_index) = count(be_u16, package_count as _).parse(input)?;

            let packages = package_index
                .iter()
                .map(|package_index| resolve_cp_package(constant_pool, *package_index))
                .collect();

            runtime::AttributeInfo::ModulePackages(packages)
        }

        // ModuleHashes
        //  ModuleHashes_attribute {
        //    // index to CONSTANT_utf8_info structure in constant pool representing
        //    // the string "ModuleHashes"
        //    u2 attribute_name_index;
        //    u4 attribute_length;
        //
        //    // index to CONSTANT_utf8_info structure with algorithm name
        //    u2 algorithm_index;
        //
        //    // the number of entries in the hashes table
        //    u2 hashes_count;
        //    {   u2 module_name_index (index to CONSTANT_Module_info structure)
        //        u2 hash_length;
        //        u1 hash[hash_length];
        //    } hashes[hashes_count];
        //
        //  }
        // TODO:
        "ModuleHashes" => runtime::AttributeInfo::ModuleHashes,
        "ModuleTarget" => {
            let target_platform_index;
            (input, target_platform_index) = be_u16(input)?;
            runtime::AttributeInfo::ModuleTarget(resolve_runtime_cp_utf8(
                constant_pool,
                target_platform_index,
            ))
        }
        // TODO:
        "InnerClasses" => runtime::AttributeInfo::InnerClasses,
        _ => {
            // TODO:
            eprintln!("Unknown attribute {:?}", attribute_name);
            // return Err(nom::Err::Error(error_position!(
            //     input,
            //     nom::error::ErrorKind::Tag
            // )));
            runtime::AttributeInfo::Unknown(attribute_name)
        }
    };

    Ok((input, attribute_info))
}

fn parse_attribute_raw(
    constant_pool: &[runtime::ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], runtime::AttributeInfo> + '_ {
    move |input| {
        let (input, attribute_name_index) = be_u16(input)?;
        let (input, attribute_length) = be_u32(input)?;
        let (_, attribute) = parse_attribute(
            attribute_name_index,
            &input[..attribute_length as _],
            constant_pool,
        )?;
        Ok((&input[(attribute_length as _)..], attribute))
    }
}

fn parse_attributes<'a>(
    input: &'a [u8],
    constant_pool: &[runtime::ConstantPoolInfo],
) -> IResult<&'a [u8], Vec<runtime::AttributeInfo>> {
    let (input, attributes_count) = be_u16(input)?;

    let (input, attributes) =
        count(parse_attribute_raw(constant_pool), attributes_count as _).parse(input)?;

    Ok((input, attributes))
}

fn parse_code_attribute<'a>(
    input: &'a [u8],
    constant_pool: &[runtime::ConstantPoolInfo],
) -> IResult<&'a [u8], runtime::AttributeInfo> {
    let (input, max_stack) = be_u16(input)?;
    let (input, max_locals) = be_u16(input)?;

    let (input, code_length) = be_u32(input)?;
    let (input, code) = take(code_length)(input)?;

    let (input, exception_table_length) = be_u16(input)?;

    let (input, exception_table) = count(
        parse_exception_table(constant_pool),
        exception_table_length as _,
    )
    .parse(input)?;

    let (input, attributes) = parse_attributes(input, constant_pool)?;

    Ok((
        input,
        runtime::AttributeInfo::Code(runtime::CodeAttribute {
            max_stack,
            max_locals,
            code: code.into(),
            exception_table,
            attributes,
        }),
    ))
}

fn parse_exception_table(
    constant_pool: &[runtime::ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], runtime::ExceptionTableItem> + '_ {
    move |input| {
        let (input, start_pc) = be_u16(input)?;
        let (input, end_pc) = be_u16(input)?;
        let (input, handler_pc) = be_u16(input)?;
        let (input, catch_type) = be_u16(input)?;

        let catch_type_info = if catch_type == 0 {
            None
        } else {
            Some(resolve_cp_class(constant_pool, catch_type))
        };

        Ok((
            input,
            runtime::ExceptionTableItem {
                start_pc,
                end_pc,
                handler_pc,
                catch_type: catch_type_info.cloned(),
            },
        ))
    }
}

fn parse_annotation(
    constant_pool: &[runtime::ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], Annotation> + '_ {
    move |input| {
        let (input, type_index) = be_u16(input)?;
        let type_str = resolve_runtime_cp_utf8(constant_pool, type_index);
        let (input, num_element_value_pairs) = be_u16(input)?;
        let (input, element_value_pairs) = count(
            parse_element_value_pair(constant_pool),
            num_element_value_pairs as _,
        )
        .parse(input)?;

        Ok((
            input,
            Annotation {
                // TODO: unwrap
                type_descriptor: parse_field_descriptor(&type_str.to_str()).unwrap().1,
                element_value_pairs,
            },
        ))
    }
}

fn parse_element_value_pair(
    constant_pool: &[runtime::ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], ElementValuePair> + '_ {
    move |input| {
        let (input, element_name_index) = be_u16(input)?;
        let element_name = resolve_runtime_cp_utf8(constant_pool, element_name_index);
        let (input, value) = parse_element_value(constant_pool)(input)?;
        Ok((
            input,
            ElementValuePair {
                element_name,
                value,
            },
        ))
    }
}

fn parse_element_value(
    constant_pool: &[runtime::ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], ElementValue> + '_ {
    move |input| {
        let (input, tag) = u8(input)?;
        let mut input = input;
        let value = match tag {
            b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b's' => {
                let converter = match tag {
                    b'B' => Const::into_byte,
                    b'C' => Const::into_char,
                    b'S' => Const::into_short,
                    b'Z' => Const::into_boolean,
                    b'D' | b'F' | b'I' | b'J' | b's' => identity,
                    _ => unreachable!("all case covered"),
                };

                let c;
                (input, c) = parse_constant_value(input, constant_pool, converter)?;
                ElementValue::Const(c)
            }
            b'e' => {
                let (type_name_index, const_name_index);
                (input, type_name_index) = be_u16(input)?;
                (input, const_name_index) = be_u16(input)?;
                ElementValue::Enum {
                    type_name: resolve_runtime_cp_utf8(constant_pool, type_name_index),
                    const_name: resolve_runtime_cp_utf8(constant_pool, const_name_index),
                }
            }
            b'c' => {
                let class_info_index;
                (input, class_info_index) = be_u16(input)?;
                let class_info = resolve_runtime_cp_utf8(constant_pool, class_info_index);
                // TODO: unwrap
                let class = parse_return_type_descriptor(&class_info.to_str())
                    .unwrap()
                    .1;

                ElementValue::Class(class)
            }
            b'@' => {
                let annotation;
                (input, annotation) = parse_annotation(constant_pool)(input)?;
                ElementValue::Annotation(annotation)
            }
            b'[' => {
                let (num_values, values);
                (input, num_values) = be_u16(input)?;
                (input, values) =
                    count(parse_element_value(constant_pool), num_values as _).parse(input)?;
                ElementValue::Array(values)
            }
            _ => {
                eprintln!("unkonwn element value tag {}", tag);
                return Err(nom::Err::Error(error_position!(
                    input,
                    nom::error::ErrorKind::Tag
                )));
            }
        };
        Ok((input, value))
    }
}

fn parse_constant_value<'a>(
    input: &'a [u8],
    constant_pool: &[runtime::ConstantPoolInfo],
    converter: impl FnOnce(runtime::Const) -> runtime::Const,
) -> IResult<&'a [u8], runtime::Const> {
    let (input, const_value_index) = be_u16(input)?;
    let value = resolve_constant_value(constant_pool, const_value_index);
    Ok((input, converter(value)))
}

fn parse_local_variable(
    constant_pool: &[runtime::ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], LocalVariable> + '_ {
    move |input| {
        let (input, start_pc) = be_u16(input)?;
        let (input, length) = be_u16(input)?;
        let (input, name_index) = be_u16(input)?;
        let (input, descriptor_index) = be_u16(input)?;
        let descriptor = resolve_runtime_cp_utf8(constant_pool, descriptor_index);
        // TODO: unwrap
        let (_, descriptor) = parse_field_descriptor(&descriptor.to_str()).unwrap();
        let (input, index) = be_u16(input)?;

        Ok((
            input,
            LocalVariable {
                start_pc,
                length,
                name: resolve_runtime_cp_utf8(constant_pool, name_index),
                descriptor,
                index,
            },
        ))
    }
}

fn allocate_static_fields(static_fields_info: &mut [FieldInfo]) -> Vec<RwLock<Variable>> {
    let mut static_fields = Vec::with_capacity(static_fields_info.len());
    for field in static_fields_info {
        let const_value = field.attributes.iter().find_map(|attr| {
            if let runtime::AttributeInfo::ConstantValue(value) = attr {
                Some(value)
            } else {
                None
            }
        });
        field.index = static_fields.len() as _;
        match field.descriptor.0 {
            FieldType::Byte
            | FieldType::Char
            | FieldType::Short
            | FieldType::Int
            | FieldType::Boolean => {
                let value = const_value
                    .map(|value| {
                        use Const::*;
                        let (Byte(a) | Char(a) | Int(a) | Short(a) | Boolean(a)) = value else {
                            panic!("unexpected const value");
                        };
                        *a
                    })
                    .unwrap_or(0);
                static_fields.push(RwLock::new(Variable { int: value }));
            }
            FieldType::Double => {
                let value = const_value
                    .map(|value| {
                        use Const::*;
                        let Double(a) = value else {
                            panic!("unexpected const value");
                        };
                        *a
                    })
                    .unwrap_or(0.0);
                let (a, b) = Variable::put_double(value);
                static_fields.push(RwLock::new(a));
                static_fields.push(RwLock::new(b));
            }
            FieldType::Float => {
                let value = const_value
                    .map(|value| {
                        use Const::*;
                        let Float(a) = value else {
                            panic!("unexpected const value");
                        };
                        *a
                    })
                    .unwrap_or(0.0);
                static_fields.push(RwLock::new(Variable { float: value }));
            }
            FieldType::Long => {
                let value = const_value
                    .map(|value| {
                        use Const::*;
                        let Long(a) = value else {
                            panic!("unexpected const value");
                        };
                        *a
                    })
                    .unwrap_or(0);
                let (a, b) = Variable::put_long(value);
                static_fields.push(RwLock::new(a));
                static_fields.push(RwLock::new(b));
            }
            FieldType::Object(_) | FieldType::Array(_) => {
                // TODO: String const
                static_fields.push(RwLock::new(Variable { reference: 0 }))
            }
        }
    }
    static_fields
}

fn resolve_static_field(
    class: &Arc<runtime::Class>,
    field_ref: &runtime::Fieldref,
    skip_this: bool,
) -> Option<FieldResolve> {
    if !skip_this {
        let name_and_type = &field_ref.name_and_type;
        for field in &class.static_fields_info {
            if !(field.name == name_and_type.name && field.descriptor == name_and_type.descriptor) {
                continue;
            }
            println!(
                "loaded field from other class: {:?} from {}.{}",
                field_ref.name_and_type.name, class.class_name, field.index
            );
            return Some(FieldResolve::OtherClass {
                class: Arc::clone(class),
                index: field.index,
            });
        }
    }

    // not found, go further
    for interface in &class.interfaces {
        if let Some(resolve) = resolve_static_field(interface, field_ref, false) {
            return Some(resolve);
        }
    }
    if let Some(ref super_class) = class.super_class {
        return resolve_static_field(super_class, field_ref, false);
    }
    None
}

pub(in crate::runtime) fn resolve_field(
    class: &Arc<runtime::Class>,
    field_ref: &runtime::Fieldref,
    is_static: bool,
) -> Option<FieldResolve> {
    if is_static {
        return resolve_static_field(class, field_ref, false);
    }
    let index = class
        .instance_fields_info
        .iter()
        .find(|f| {
            f.name == field_ref.name_and_type.name
                && f.descriptor == field_ref.name_and_type.descriptor
        })?
        .index;

    Some(FieldResolve::OtherClass {
        class: Arc::clone(class),
        index,
    })
}
