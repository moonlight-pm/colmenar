use crate::prelude::*;
use once_cell::sync::OnceCell;
use openapiv3::{ParameterData, ParameterSchemaOrContent, ReferenceOr, SchemaKind, Type};
use std::{collections::BTreeMap, sync::Mutex};

static PARAMETERS: OnceCell<Mutex<BTreeMap<String, Parameter>>> = OnceCell::new();

#[derive(Debug, Clone)]
pub struct Parameter {
    pub type_name: String,
    pub original_name: String,
    pub name: String,
    pub safe_name: String,
    pub ty: Tokens,
    pub description: Option<String>,
    pub required: bool,
}

impl Parameter {
    pub fn all() -> Vec<Parameter> {
        PARAMETERS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    fn add(parameter: Parameter) {
        let mut parameters = PARAMETERS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .unwrap();
        if parameters.contains_key(&parameter.original_name) {
            panic!("Parameter {} already exists", parameter.original_name);
        }
        parameters.insert(parameter.type_name.clone(), parameter);
    }

    pub fn get(name: &str) -> Option<Parameter> {
        PARAMETERS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .unwrap()
            .get(name)
            .cloned()
    }

    pub fn discover(type_name: &str, data: ParameterData) -> Result<Self, Error> {
        let original_name = data.name.clone();
        let name = data.name.to_snake_case();
        let safe_name = if KEYWORDS.contains(&name.as_str()) {
            format!("r#{name}")
        } else {
            name.clone()
        };
        let ty = match data.format {
            ParameterSchemaOrContent::Schema(schema) => match schema {
                ReferenceOr::Reference { .. } => {
                    unimplemented!()
                }
                ReferenceOr::Item(schema) => match schema.schema_kind {
                    SchemaKind::Type(Type::String(_)) => {
                        quote!(String)
                    }
                    SchemaKind::Type(Type::Array(_)) => {
                        let model = Array::discover(&type_name, &schema)?;
                        if model.to_string().unwrap().contains(&type_name) {
                            let model = import("super", type_name);
                            quote!($model)
                        } else {
                            quote!($model)
                        }
                    }
                    _ => {
                        Model::discover(&type_name, &schema)?;
                        let model = import("super", type_name);
                        quote!($model)
                    }
                },
            },
            ParameterSchemaOrContent::Content(_) => {
                // return err!("{}", content);
                unimplemented!()
            }
        };
        let parameter = Self {
            type_name: type_name.to_string(),
            original_name,
            name: name,
            safe_name,
            ty,
            description: data.description,
            required: data.required,
        };
        Parameter::add(parameter.clone());
        Ok(parameter)
    }
}
