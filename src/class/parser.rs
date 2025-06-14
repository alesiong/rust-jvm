use crate::{
    class::{AttributeInfo, Class, ConstantPoolInfo, FieldInfo, MethodInfo, structs::JavaStr},
    consts::{ClassAccessFlag, FieldAccessFlag, MethodAccessFlag},
};
use nom::{
    IResult, Parser,
    bytes::complete::{tag, take},
    combinator::eof,
    error_position,
    multi::count,
    number::complete::{be_f32, be_f64, be_i32, be_i64, be_u16, be_u32, u8},
};

pub fn class_file(input: &[u8]) -> Result<Class, nom::Err<nom::error::Error<&[u8]>>> {
    let (input, (minor, major)) = parse_header(input)?;
    let (input, constant_pool) = parse_constant_pool(input)?;

    let (input, access_flags) = be_u16(input)?;
    let (input, this_class) = be_u16(input)?;
    let (input, super_class) = be_u16(input)?;
    let (input, interfaces) = parse_interfaces(input)?;
    let (input, fields) = parse_fields(input)?;
    let (input, methods) = parse_methods(input)?;
    let (input, attributes) = parse_attributes(input)?;

    eof(input)?;

    Ok(Class {
        major_version: major,
        minor_version: minor,
        access_flags: ClassAccessFlag::from_bits_retain(access_flags),
        this_class,
        super_class,
        constant_pool,
        interfaces,
        fields,
        methods,
        attributes,
    })
}

fn parse_header(input: &[u8]) -> IResult<&[u8], (u16, u16)> {
    let (input, _) = tag(&[0xcau8, 0xfe, 0xba, 0xbe] as &[u8])(input)?;
    let (input, minor) = be_u16(input)?;
    let (input, major) = be_u16(input)?;
    Ok((input, (minor, major)))
}

fn parse_constant_pool(input: &[u8]) -> IResult<&[u8], Vec<ConstantPoolInfo>> {
    let (input, constant_pool_count) = be_u16(input)?;

    let mut constant_pool = Vec::with_capacity(constant_pool_count as usize - 1);

    let mut input = input;

    while constant_pool.len() < constant_pool_count as usize - 1 {
        let constant;
        (input, constant) = parse_constant(input)?;
        let need_empty = matches!(
            constant,
            ConstantPoolInfo::Long(_) | ConstantPoolInfo::Double(_)
        );
        constant_pool.push(constant);
        if need_empty {
            constant_pool.push(ConstantPoolInfo::Empty);
        }
    }

    Ok((input, constant_pool))
}

fn parse_constant(mut input: &[u8]) -> IResult<&[u8], ConstantPoolInfo> {
    let tag;
    (input, tag) = u8(input)?;
    let cp_info = match tag {
        1 => {
            let length;
            (input, length) = be_u16(input)?;
            let bytes;
            (input, bytes) = take(length)(input)?;
            ConstantPoolInfo::Utf8(
                // SAFETY: from JVM class file
                unsafe { JavaStr::new(bytes) }.into(),
            )
        }
        3 => {
            let int;
            (input, int) = be_i32(input)?;
            ConstantPoolInfo::Integer(int)
        }
        4 => {
            let float;
            (input, float) = be_f32(input)?;
            ConstantPoolInfo::Float(float)
        }
        5 => {
            let long;
            (input, long) = be_i64(input)?;
            ConstantPoolInfo::Long(long)
        }
        6 => {
            let double;
            (input, double) = be_f64(input)?;
            ConstantPoolInfo::Double(double)
        }
        7 => {
            let name_index;
            (input, name_index) = be_u16(input)?;

            ConstantPoolInfo::Class { name_index }
        }
        8 => {
            let string_index;
            (input, string_index) = be_u16(input)?;

            ConstantPoolInfo::String { string_index }
        }
        9 => {
            let (class_index, name_and_type_index);
            (input, class_index) = be_u16(input)?;
            (input, name_and_type_index) = be_u16(input)?;
            ConstantPoolInfo::Fieldref {
                class_index,
                name_and_type_index,
            }
        }
        10 => {
            let (class_index, name_and_type_index);
            (input, class_index) = be_u16(input)?;
            (input, name_and_type_index) = be_u16(input)?;
            ConstantPoolInfo::Methodref {
                class_index,
                name_and_type_index,
            }
        }
        11 => {
            let (class_index, name_and_type_index);
            (input, class_index) = be_u16(input)?;
            (input, name_and_type_index) = be_u16(input)?;
            ConstantPoolInfo::InterfaceMethodref {
                class_index,
                name_and_type_index,
            }
        }
        12 => {
            let (name_index, descriptor_index);
            (input, name_index) = be_u16(input)?;
            (input, descriptor_index) = be_u16(input)?;
            ConstantPoolInfo::NameAndType {
                name_index,
                descriptor_index,
            }
        }
        15 => {
            let (reference_kind, reference_index);
            (input, reference_kind) = u8(input)?;
            (input, reference_index) = be_u16(input)?;
            ConstantPoolInfo::MethodHandle {
                reference_kind,
                reference_index,
            }
        }
        16 => {
            let descriptor_index;
            (input, descriptor_index) = be_u16(input)?;
            ConstantPoolInfo::MethodType { descriptor_index }
        }
        17 => {
            let (bootstrap_method_attr_index, name_and_type_index);
            (input, bootstrap_method_attr_index) = be_u16(input)?;
            (input, name_and_type_index) = be_u16(input)?;
            ConstantPoolInfo::Dynamic {
                bootstrap_method_attr_index,
                name_and_type_index,
            }
        }
        18 => {
            let (bootstrap_method_attr_index, name_and_type_index);
            (input, bootstrap_method_attr_index) = be_u16(input)?;
            (input, name_and_type_index) = be_u16(input)?;
            ConstantPoolInfo::InvokeDynamic {
                bootstrap_method_attr_index,
                name_and_type_index,
            }
        }
        19 => {
            let name_index;
            (input, name_index) = be_u16(input)?;
            ConstantPoolInfo::Module { name_index }
        }
        20 => {
            let name_index;
            (input, name_index) = be_u16(input)?;
            ConstantPoolInfo::Package { name_index }
        }
        _ => {
            eprintln!("unkonwn constant type {}", tag);
            return Err(nom::Err::Error(error_position!(
                input,
                nom::error::ErrorKind::Tag
            )));
        }
    };
    Ok((input, cp_info))
}

fn parse_interfaces(input: &[u8]) -> IResult<&[u8], Vec<u16>> {
    let (input, interface_count) = be_u16(input)?;

    let (input, interfaces) = count(be_u16, interface_count as _).parse(input)?;

    Ok((input, interfaces))
}

fn parse_fields(input: &[u8]) -> IResult<&[u8], Vec<FieldInfo>> {
    let (input, field_count) = be_u16(input)?;
    let (input, fields) = count(parse_field, field_count as _).parse(input)?;
    Ok((input, fields))
}

fn parse_field(input: &[u8]) -> IResult<&[u8], FieldInfo> {
    let (input, access_flags) = be_u16(input)?;
    let (input, name_index) = be_u16(input)?;
    let (input, descriptor_index) = be_u16(input)?;

    let (input, attributes) = parse_attributes(input)?;
    Ok((
        input,
        FieldInfo {
            access_flags: FieldAccessFlag::from_bits_retain(access_flags),
            name_index,
            descriptor_index,
            attributes,
        },
    ))
}

fn parse_attributes(input: &[u8]) -> IResult<&[u8], Vec<AttributeInfo>> {
    let (input, attributes_count) = be_u16(input)?;

    let (input, attributes) = count(parse_attribute, attributes_count as _).parse(input)?;

    Ok((input, attributes))
}

fn parse_attribute(input: &[u8]) -> IResult<&[u8], AttributeInfo> {
    let (input, attribute_name_index) = be_u16(input)?;
    let (input, attribute_length) = be_u32(input)?;
    let (input, info) = take(attribute_length)(input)?;

    Ok((
        input,
        AttributeInfo {
            attribute_name_index,
            info: info.to_vec(),
        },
    ))
}

fn parse_methods(input: &[u8]) -> IResult<&[u8], Vec<MethodInfo>> {
    let (input, methods_count) = be_u16(input)?;

    let (input, methods) = count(parse_method, methods_count as _).parse(input)?;

    Ok((input, methods))
}

fn parse_method(input: &[u8]) -> IResult<&[u8], MethodInfo> {
    let (input, access_flags) = be_u16(input)?;
    let (input, name_index) = be_u16(input)?;
    let (input, descriptor_index) = be_u16(input)?;
    let (input, attributes) = parse_attributes(input)?;

    Ok((
        input,
        MethodInfo {
            access_flags: MethodAccessFlag::from_bits_retain(access_flags),
            name_index,
            descriptor_index,
            attributes,
        },
    ))
}
