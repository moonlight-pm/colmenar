use crate::{err, generate::*, Error, Property};
use heck::{ToSnakeCase, ToUpperCamelCase};
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use openapiv3::{ReferenceOr, Schema, SchemaKind, Type};
use std::{collections::HashMap, sync::Mutex};

static MODELS: OnceCell<Mutex<HashMap<String, Model>>> = OnceCell::new();

#[derive(Clone)]
pub struct Model {
    pub path: String,
    pub name: String,
    pub ty: Option<Tokens>,
    pub description: Option<String>,
    pub properties: Vec<Property>,
    pub enumeration: Vec<String>,
}

impl Model {
    pub fn all() -> Vec<Model> {
        MODELS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    fn add(model: Model) {
        MODELS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .insert(model.name.clone(), model);
    }

    fn get(name: &str) -> Option<Model> {
        MODELS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .get(name)
            .cloned()
    }

    pub fn discover(name: &str, schema: &Schema) -> Result<(), Error> {
        let path = name.to_snake_case();
        let mut model = Self {
            path: path.clone(),
            name: name.to_string(),
            ty: None,
            description: None,
            properties: Vec::new(),
            enumeration: Vec::new(),
        };

        model.description = schema.schema_data.description.clone();
        match &schema.schema_kind {
            SchemaKind::Type(Type::String(string)) => {
                if string.enumeration.is_empty() {
                    model.ty = Some(quote!(String));
                } else {
                    model.enumeration = string
                        .enumeration
                        .iter()
                        .filter_map(|s| s.as_ref().map(|s| s.to_upper_camel_case()))
                        .collect();
                }
            }
            SchemaKind::Type(Type::Object(object)) => {
                model.discover_properties(&object.required, &object.properties)?;
            }
            SchemaKind::AllOf { all_of } => {
                for schema in all_of.iter() {
                    match schema {
                        ReferenceOr::Reference { reference, .. } => {
                            let reference = reference
                                .as_str()
                                .split('/')
                                .last()
                                .unwrap()
                                .to_upper_camel_case();
                            let reference = Model::get(&reference).unwrap();
                            model.properties.extend(reference.properties.clone());
                        }
                        ReferenceOr::Item(item) => match &item.schema_kind {
                            SchemaKind::Type(Type::Object(object)) => {
                                model.discover_properties(&object.required, &object.properties)?;
                            }
                            SchemaKind::Any(schema) => {
                                model.discover_properties(&schema.required, &schema.properties)?;
                            }
                            _ => return err!("Unhandled type: {:?} {name:?}", item.schema_kind,),
                        },
                    }
                }
            }
            _ => return err!("Unhandled kind {:?}", schema.schema_kind),
        };
        Model::add(model);
        Ok(())
    }

    pub fn discover_properties(
        &mut self,
        required: &Vec<String>,
        indexmap: &IndexMap<String, ReferenceOr<Box<Schema>>>,
    ) -> Result<(), Error> {
        for (name, schema) in indexmap.iter() {
            let mut property = Property::new(name);
            property.required = required.contains(name);
            match schema {
                ReferenceOr::Reference {
                    reference,
                    description,
                    ..
                } => {
                    property.description = description.clone();
                    let ty = reference.split("/").last().unwrap().to_string();
                    let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                    property.ty = quote!($ty);
                }
                ReferenceOr::Item(item) => {
                    let ty = format!("{}_{name}", self.name).to_upper_camel_case();
                    match &item.schema_kind {
                        SchemaKind::Type(Type::String(string)) => {
                            property.description = item.schema_data.description.clone();
                            if string.enumeration.is_empty() {
                                property.ty = quote!(String)
                            } else {
                                Model::discover(&ty, item)?;
                                let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                                property.ty = quote!($ty)
                            }
                        }
                        SchemaKind::Type(Type::Object(_)) => {
                            Model::discover(&ty, item)?;
                            let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                            property.ty = quote!($ty)
                        }
                        _ => return err!("Unhandled type: {:?} {name:?}", item.schema_kind,),
                    }
                }
            };
            self.properties.push(property);
        }
        Ok(())
    }

    pub fn add_property(&mut self, property: Property) {
        self.properties.push(property);
    }

    pub fn write(&self, dir: &str) -> Result<(), Error> {
        let path = format!("{dir}/models/{}.rs", self.path);
        let mut tokens = Tokens::new();
        if let Some(description) = &self.description {
            tokens.append(quote!(
                #[doc=$(quoted(description))]
            ));
        }
        tokens.append(match &self.ty {
            Some(ty) => quote!(
                pub type $(&self.name) = $ty;
            ),
            None =>  {
                if self.enumeration.is_empty() {
                    quote!(
                        pub struct $(&self.name) {
                            $(for property in &self.properties =>
                                $(property.description.as_ref().map(|description| quote!(#[doc=$(quoted(description))])))
                                pub $(&property.name): $(&property.ty),
                            )
                        }
                    )
                } else {
                    quote!(
                        pub enum $(&self.name) {
                            $(for value in &self.enumeration =>
                                $value,
                            )
                        }
                    )
                }
            }
        });
        write_tokens(&path, tokens)?;
        Ok(())
    }
}
