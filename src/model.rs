use crate::prelude::*;
use once_cell::sync::OnceCell;
use openapiv3::{ReferenceOr, Schema, SchemaKind, Type};
use std::{collections::BTreeMap, sync::Mutex};

static MODELS: OnceCell<Mutex<BTreeMap<String, Model>>> = OnceCell::new();

#[derive(Clone)]
pub struct Model {
    pub path: String,
    pub name: String,
    pub ty: Option<Tokens>,
    pub description: Option<String>,
    pub properties: Vec<Property>,
    pub enumeration: Option<Enumeration>,
}

impl Model {
    pub fn all() -> Vec<Model> {
        MODELS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    fn add(model: Model) {
        let mut models = MODELS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .unwrap();
        if models.contains_key(&model.name) {
            panic!("Model {} already exists", model.name);
        }
        models.insert(model.name.clone(), model);
    }

    fn get(name: &str) -> Option<Model> {
        MODELS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
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
            enumeration: None,
        };
        model.description = schema.schema_data.description.clone();
        match &schema.schema_kind {
            SchemaKind::Type(Type::String(_)) => {
                model.enumeration = Enumeration::discover(schema);
                if model.enumeration.is_none() {
                    if model.name == "DateTime" {
                        model.ty = Some(quote!(chrono::DateTime<chrono::Utc>));
                    } else {
                        model.ty = Some(quote!(String));
                    }
                }
            }
            SchemaKind::Type(Type::Integer(_)) => {
                model.enumeration = Enumeration::discover(schema);
                if model.enumeration.is_none() {
                    model.ty = Some(quote!(i64));
                }
            }
            SchemaKind::Type(Type::Array(_)) => {
                model.ty = Some(Array::discover(
                    &format!("{name}_Item").to_upper_camel_case(),
                    schema,
                )?);
            }
            SchemaKind::Type(Type::Object(object)) => {
                Property::discover(&mut model, &object.required, &object.properties)?;
            }
            SchemaKind::Any(schema) => {
                Property::discover(&mut model, &schema.required, &schema.properties)?;
            }
            SchemaKind::AllOf { all_of } => {
                for schema in all_of.iter() {
                    match schema {
                        ReferenceOr::Reference { reference, .. } => {
                            let reference = reference.split('/').last().unwrap();
                            let reference = Model::get(&reference).unwrap();
                            model.properties.extend(reference.properties.clone());
                        }
                        ReferenceOr::Item(item) => match &item.schema_kind {
                            SchemaKind::Type(Type::Object(object)) => {
                                Property::discover(
                                    &mut model,
                                    &object.required,
                                    &object.properties,
                                )?;
                            }
                            SchemaKind::Any(schema) => {
                                Property::discover(
                                    &mut model,
                                    &schema.required,
                                    &schema.properties,
                                )?;
                            }
                            SchemaKind::OneOf { one_of } => {
                                let mut types = Vec::new();
                                let mut g = 0;
                                for schema in one_of.iter() {
                                    match schema {
                                        ReferenceOr::Reference { reference, .. } => {
                                            types.push(
                                                reference.split('/').last().unwrap().to_string(),
                                            );
                                        }
                                        ReferenceOr::Item(item) => {
                                            let ty = format!("{name}_{}", GREEK[g])
                                                .to_upper_camel_case();
                                            Model::discover(&ty, item)?;
                                            types.push(ty);
                                            g += 1;
                                        }
                                    }
                                }
                                model.enumeration = Some(Enumeration::Object(types));
                            }
                            _ => {
                                return err!("Unhandled type for '{name}': {:?}", item.schema_kind,)
                            }
                        },
                    }
                }
            }
            SchemaKind::OneOf { one_of } => {
                let mut types = Vec::new();
                let mut g = 0;
                for schema in one_of.iter() {
                    match schema {
                        ReferenceOr::Reference { reference, .. } => {
                            types.push(reference.split('/').last().unwrap().to_string());
                        }
                        ReferenceOr::Item(item) => {
                            let ty = format!("{name}_{}", GREEK[g]).to_upper_camel_case();
                            Model::discover(&ty, item)?;
                            types.push(ty);
                            g += 1;
                        }
                    }
                }
                model.enumeration = Some(Enumeration::Object(types));
            }
            _ => return err!("Unhandled kind for '{name}': {:?}", schema.schema_kind),
        };
        Model::add(model);
        Ok(())
    }

    pub fn tokens(&self) -> Result<Tokens, Error> {
        let import_serialize = rust::import("serde", "Serialize");
        let import_deserialize = rust::import("serde", "Deserialize");
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
                match &self.enumeration {
                    Some(enumeration) => {
                        match enumeration {
                            Enumeration::String(values) => {
                                let range = 0..values.len();
                                let variants = values.into_iter().map(|s| {
                                    if s.chars().next().unwrap().is_digit(10) {
                                        format!("{}{}", &self.name, s).to_upper_camel_case()
                                    } else {
                                        s.to_upper_camel_case()
                                    }
                                }).collect::<Vec<_>>();
                                quote!(
                                    #[derive(Debug, Clone, PartialEq, $import_serialize, $import_deserialize)]
                                    pub enum $(&self.name) {
                                        $(for v in range =>
                                            #[serde(rename = $(quoted(&values[v])))]
                                            $(&variants[v]),
                                        )
                                    }
                                    $['\n']
                                    impl Default for $(&self.name) {
                                        fn default() -> Self { Self::$(&variants[0]) }
                                    }
                                )
                            }
                            Enumeration::Integer(values) => {
                                quote!(
                                    #[derive(Debug, Clone, PartialEq, $import_serialize, $import_deserialize)]
                                    pub enum $(&self.name) {
                                        $(for value in values =>
                                            $(&self.name)$(*value) = $(*value),
                                        )
                                    }
                                )
                            }
                            Enumeration::Object(types) => {
                                quote!(
                                    #[derive(Debug, Clone, PartialEq, $import_serialize, $import_deserialize)]
                                    pub enum $(&self.name) {
                                        $(for t in types =>
                                            $t($t),
                                        )
                                    }
                                )
                            }
                        }
                    }
                    None => {
                        let strtok = quote!(String);
                        let has_string = self.properties.iter().any(|p| {
                            p.ty == strtok
                        });
                        quote!(
                            #[derive(Debug, Clone, PartialEq, Default, $import_serialize, $import_deserialize)]
                            pub struct $(&self.name) {
                                $(for property in &self.properties =>
                                    $(property.description.as_ref().map(|description| quote!(#[doc=$(quoted(description))])))
                                    $(if !property.required { #[serde(skip_serializing_if = "Option::is_none")] })
                                    $(if property.name != property.safe_name {
                                        #[serde(rename = $(quoted(&property.name)))]
                                    })
                                    pub $(&property.safe_name):
                                    $(if property.required && !property.nullable { $(&property.ty) } else { Option<$(&property.ty)> }),
                                )
                            }
                            $['\n']
                            impl $(&self.name) {
                                pub fn new
                                $(if has_string {<S: AsRef<str>>})
                                (
                                    $(for property in &self.properties =>
                                        $(if property.required {
                                            $(&property.safe_name):
                                            $(if property.nullable {
                                                $(if property.ty == strtok {
                                                    Option<S>
                                                } else {
                                                    Option<$(&property.ty)>
                                                })
                                            } else {
                                                $(if property.ty == strtok {
                                                    S
                                                } else {
                                                    $(&property.ty)
                                                })
                                            }),
                                        })
                                    )
                                ) -> Self {
                                    $(for property in &self.properties =>
                                        $(if property.required && property.ty == strtok {
                                            $(if property.nullable {
                                                let $(&property.safe_name) = $(&property.safe_name).map(|s| s.as_ref().to_string());
                                            } else {
                                                let $(&property.safe_name) = $(&property.safe_name).as_ref().to_string();
                                            })
                                        })
                                    )
                                    Self {
                                        $(for property in &self.properties =>
                                            $(if property.required {
                                                $(&property.safe_name),
                                            })
                                        )
                                        ..Default::default()
                                    }
                                }
                            }
                        )
                    }
                }
            }
        });
        tokens.line();
        Ok(tokens)
    }
}
