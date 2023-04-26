use crate::workload::Workload;
use crate::{err, Error};
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

pub fn generate(workload: &Workload) -> Result<(), Error> {
    let mut models = Vec::new();
    for (name, schema) in workload.api.components.as_ref().unwrap().schemas.iter() {
        let schema = schema.as_item().unwrap();
        eprintln!("{name}: {schema:#?}");
        models.extend(generate_model(&workload, &name, schema)?);
        if name == "AccountState" {
            break;
        }
    }

    let mut module = Tokens::new();
    quote_in!(module => $(for name in &models => pub mod $(name);));
    module.line();
    quote_in!(module => $(for name in &models => pub use $(name)::$(name.to_upper_camel_case());));
    let mut file = File::create(&format!("{}/models/mod.rs", workload.output))?;
    file.write_all(module.to_string()?.as_bytes())?;
    format_generated(&workload.output)?;
    Ok(())
}

fn generate_model(workload: &Workload, name: &str, schema: &Schema) -> Result<Vec<String>, Error> {
    let path = name.to_snake_case();
    let mut models = vec![path.clone()];
    let mut tokens = Tokens::new();
    if let Some(description) = schema.schema_data.description.as_ref() {
        quote_in!(tokens => #[doc=$(quoted(description))]);
        tokens.push();
    }
    quote_in!(tokens => pub);
    tokens.space();
    let mut implementations = Tokens::new();
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
                        ReferenceOr::Reference {
                            reference,
                            description,
                            ..
                        } => {
                            if let Some(description) = description.as_ref() {
                                quote_in!(properties => #[doc=$(quoted(description))]);
                                tokens.push();
                            }
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
                                    models.extend(generate_model(workload, &ty, item)?);
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
        SchemaKind::AllOf { all_of } => {
            let mut properties = Tokens::new();
            for schema in all_of.iter() {
                match schema {
                    ReferenceOr::Reference { reference, .. } => {
                        // add all properties of the referenced schema
                    }
                    ReferenceOr::Item(item) => match &item.schema_kind {
                        SchemaKind::Type(Type::String(_)) => {
                            if let Some(description) = item.schema_data.description.as_ref() {
                                quote_in!(properties => #[doc=$(quoted(description))]);
                            }
                            quote_in!(properties => String,);
                        }
                        SchemaKind::Type(Type::Object(_)) | SchemaKind::Any(_) => {
                            let model_name = &format!("{name}_salient").to_upper_camel_case();
                            let model_import = &rust::import(
                                format!("super::{}", model_name.to_snake_case()),
                                model_name,
                            );
                            models.extend(generate_model(workload, &model_name, item)?);
                            quote_in!(properties => salient: $model_import,);
                            quote_in!(implementations =>
                                impl Deref for $name {
                                    type Target = $model_import;
                                    fn deref(&self) -> &Self::Target {
                                        &self.salient
                                    }
                                }
                            )
                        }
                        _ => return err!("Unhandled type: {:?} {name:?}", item.schema_kind,),
                    },
                }
            }
            quote_in!(tokens => struct $name { $properties });
        }
        SchemaKind::Any(schema) => {
            let mut properties = Tokens::new();
            for (property_name, property) in schema.properties.iter() {
                let required = schema.required.contains(property_name);
                let property_type = match property {
                    ReferenceOr::Reference {
                        reference,
                        description,
                        ..
                    } => {
                        if let Some(description) = description.as_ref() {
                            quote_in!(properties => #[doc=$(quoted(description))]);
                            tokens.push();
                        }
                        let ty = reference.split("/").last().unwrap().to_string();
                        let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                        quote!($ty)
                    }
                    ReferenceOr::Item(item) => {
                        let ty = format!("{name}_{property_name}").to_upper_camel_case();
                        match &item.schema_kind {
                            SchemaKind::Type(Type::String(_)) => {
                                if let Some(description) = item.schema_data.description.as_ref() {
                                    quote_in!(properties => #[doc=$(quoted(description))]);
                                }
                                quote!(String)
                            }
                            SchemaKind::Type(Type::Object(_)) => {
                                models.extend(generate_model(workload, &ty, item)?);
                                let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
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
        _ => return err!("Unhandled kind {:?}", schema.schema_kind),
    };
    tokens.line();
    tokens.append(implementations);
    let path = format!("{}/models/{path}.rs", workload.output);
    let mut file = File::create(&path)?;
    file.write_all(tokens.to_file_string()?.as_bytes())?;
    Ok(models)
}

// fn generate_type(type_: Type, )

// run Command cargo fmt
fn format_generated(path: &str) -> Result<(), Error> {
    Command::new("bash")
        .args(["-c", &format!("rustfmt {path}/**/*.rs")])
        .status()?;
    Ok(())
}
