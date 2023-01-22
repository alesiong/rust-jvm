use nom::{
    branch::alt,
    bytes::complete::take_until,
    character::complete::{char, one_of},
    combinator::{eof, map},
    multi::many0,
    sequence::delimited,
    IResult,
};

#[derive(Debug)]
pub struct FieldDescriptor(pub(crate) FieldType);

#[derive(Debug)]
pub struct MethodDescriptor {
    pub(crate) parameters: Vec<FieldType>,
    pub(crate) return_type: Option<FieldType>,
}

#[derive(Debug)]
pub enum FieldType {
    Byte,
    Char,
    Double,
    Float,
    Int,
    Long,
    Object(String),
    Short,
    Boolean,
    Array(Box<FieldType>),
}

pub fn parse_field_descriptor(input: &str) -> IResult<&str, FieldDescriptor> {
    let (input, field_type) = parse_field_type(input)?;
    eof(input)?;
    Ok((input, FieldDescriptor(field_type)))
}

pub fn parse_method_descriptor(input: &str) -> IResult<&str, MethodDescriptor> {
    let (input, parameters) = delimited(char('('), many0(parse_field_type), char(')'))(input)?;

    let (input, return_type) = alt((map(parse_field_type, Some), parse_void_type))(input)?;

    eof(input)?;
    Ok((
        input,
        MethodDescriptor {
            parameters,
            return_type,
        },
    ))
}

fn parse_field_type(input: &str) -> IResult<&str, FieldType> {
    alt((parse_base_type, parse_object_type, parse_array_type))(input)
}

fn parse_base_type(input: &str) -> IResult<&str, FieldType> {
    let (input, ch) = one_of("BCDFIJSZ")(input)?;
    let field_type = match ch {
        'B' => FieldType::Byte,
        'C' => FieldType::Char,
        'D' => FieldType::Double,
        'F' => FieldType::Float,
        'I' => FieldType::Int,
        'J' => FieldType::Long,
        'S' => FieldType::Short,
        'Z' => FieldType::Boolean,
        _ => {
            todo!("error")
        }
    };
    Ok((input, field_type))
}

fn parse_object_type(input: &str) -> IResult<&str, FieldType> {
    let (input, _) = char('L')(input)?;

    let (input, class_name) = take_until(";")(input)?;

    let (input, _ ) = char(';')(input)?;

    Ok((input, FieldType::Object(class_name.to_string())))
}

fn parse_array_type(input: &str) -> IResult<&str, FieldType> {
    let (input, _) = char('[')(input)?;

    let (input, field_type) = parse_field_type(input)?;

    Ok((input, FieldType::Array(Box::new(field_type))))
}

fn parse_void_type(input: &str) -> IResult<&str, Option<FieldType>> {
    let (input, _) = char('V')(input)?;
    Ok((input, None))
}
