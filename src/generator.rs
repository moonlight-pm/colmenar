use crate::error::{err, Error};
use crate::workload::Workload;
use genco::{
    lang::rust::{self, Tokens},
    prelude::quoted,
    quote, quote_in,
};
use heck::{ToSnakeCase, ToUpperCamelCase};
use openapiv3::{ReferenceOr, Schema, SchemaKind, Type};
use std::fs::File;
use std::io::Write;
use std::process::Command;

pub fn generate(workload: Workload) -> Result<(), Error> {
    for (name, schema) in workload.api.schemas() {
        let schema = schema.as_item().unwrap();
        println!("{name}: {schema:#?}");
        generate_model(&workload, name, schema)?;
        if name == "AccountState" {
            break;
        }
    }
    format_generated(&workload.output)?;
    Ok(())
}

fn generate_model(workload: &Workload, name: &str, schema: &Schema) -> Result<(), Error> {
    let path = name.to_snake_case();
    let mut tokens = Tokens::new();
    if let Some(description) = schema.schema_data.description.as_ref() {
        quote_in!(tokens => #[doc=$(quoted(description))]);
        tokens.push();
    }
    quote_in!(tokens => pub);
    tokens.space();
    match &schema.schema_kind {
        SchemaKind::Type(type_) => match type_ {
            // String(StringType),
            // Number(NumberType),
            // Integer(IntegerType),
            // Object(ObjectType),
            // Array(ArrayType),
            // Boolean {},
            Type::String(_) => {
                quote_in!(tokens => type $name = String;);
            }
            Type::Object(object) => {
                let mut properties = Tokens::new();
                for (property_name, property) in object.properties.iter() {
                    let required = object.required.contains(property_name);
                    let property_type = match property {
                        ReferenceOr::Reference { reference } => {
                            // XXX: The openapiv3 lib does not provide a description field for a reference, if it is present.
                            let ty = reference.split("/").last().unwrap().to_string();
                            let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                            quote!($ty)
                        }
                        ReferenceOr::Item(item) => {
                            let ty = format!("{name}_{property_name}").to_upper_camel_case();
                            match &item.schema_kind {
                                SchemaKind::Type(Type::String(_)) => {
                                    if let Some(description) = item.schema_data.description.as_ref()
                                    {
                                        quote_in!(properties => #[doc=$(quoted(description))]);
                                    }
                                    quote!(String)
                                }
                                SchemaKind::Type(Type::Object(_)) => {
                                    generate_model(workload, &ty, item)?;
                                    let ty =
                                        rust::import(format!("super::{}", ty.to_snake_case()), ty);
                                    quote!($ty)
                                }
                                _ => {
                                    return err!(
                                        "Unhandled type: {:?} {property_name:?}",
                                        item.schema_kind,
                                    )
                                }
                            }
                        }
                    };
                    let property_type = match required {
                        true => quote!($property_type),
                        false => quote!(Option<$property_type>),
                    };
                    quote_in!(properties => $property_name: $property_type,);
                }
                quote_in!(tokens => struct $name { $properties });
            }
            _ => return err!("Unhandled type: {type_:?}"),
        },
        _ => return err!("Unhandled kind {:?}", schema.schema_kind),
    };
    let path = format!("{}/models/{path}.rs", workload.output);
    let mut file = File::create(&path)?;
    file.write_all(tokens.to_file_string()?.as_bytes())?;
    Ok(())
}

// fn generate_type(type_: Type, )

// run Command cargo fmt
fn format_generated(path: &str) -> Result<(), Error> {
    Command::new("bash")
        .args(["-c", &format!("rustfmt {path}/**/*.rs")])
        .status()?;
    Ok(())
}
