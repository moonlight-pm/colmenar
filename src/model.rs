use crate::{err, generate::*, Error, Property};
use heck::{ToSnakeCase, ToUpperCamelCase};
use indexmap::IndexMap;
use openapiv3::{ReferenceOr, Schema, SchemaKind, Type};

#[derive(Clone)]
pub struct Model {
    pub path: String,
    pub name: String,
    pub ty: Option<Tokens>,
    pub description: Option<String>,
    pub properties: Vec<Property>,
}

impl Model {
    pub fn discover(name: &str, schema: &Schema) -> Result<Vec<Model>, Error> {
        let mut models = Vec::new();
        let path = name.to_snake_case();
        let mut model = Self {
            path: path.clone(),
            name: name.to_string(),
            ty: None,
            description: None,
            properties: Vec::new(),
        };

        model.description = schema.schema_data.description.clone();
        match &schema.schema_kind {
            SchemaKind::Type(type_) => match type_ {
                // String(StringType),
                // Number(NumberType),
                // Integer(IntegerType),
                // Object(ObjectType),
                // Array(ArrayType),
                // Boolean {},
                Type::String(_) => {
                    model.ty = Some(quote!(String));
                }
                Type::Object(object) => {
                    models.extend(model.discover_properties(&object.required, &object.properties)?);
                }
                _ => return err!("Unhandled type: {type_:?}"),
            },
            SchemaKind::AllOf { all_of } => {
                for schema in all_of.iter() {
                    match schema {
                        ReferenceOr::Reference { reference, .. } => {
                            // add all properties of the referenced schema
                        }
                        ReferenceOr::Item(item) => {
                            match &item.schema_kind {
                                SchemaKind::Type(Type::Object(object)) => {
                                    models.extend(model.discover_properties(
                                        &object.required,
                                        &object.properties,
                                    )?);
                                }
                                SchemaKind::Any(schema) => {
                                    models.extend(model.discover_properties(
                                        &schema.required,
                                        &schema.properties,
                                    )?);
                                }
                                _ => {
                                    return err!("Unhandled type: {:?} {name:?}", item.schema_kind,)
                                }
                            }
                        }
                    }
                }
            }
            _ => return err!("Unhandled kind {:?}", schema.schema_kind),
        };
        models.push(model);
        Ok(models)
    }

    pub fn discover_properties(
        &mut self,
        required: &Vec<String>,
        indexmap: &IndexMap<String, ReferenceOr<Box<Schema>>>,
    ) -> Result<Vec<Model>, Error> {
        let mut models = Vec::new();
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
                        SchemaKind::Type(Type::String(_)) => {
                            property.description = item.schema_data.description.clone();
                            property.ty = quote!(String)
                        }
                        SchemaKind::Type(Type::Object(_)) => {
                            models.extend(Model::discover(&ty, item)?);
                            let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                            property.ty = quote!($ty)
                        }
                        _ => return err!("Unhandled type: {:?} {name:?}", item.schema_kind,),
                    }
                }
            };
            self.properties.push(property);
        }
        Ok(models)
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
            None => quote!(
                pub struct $(&self.name) {
                    $(for property in &self.properties =>
                        $(property.description.as_ref().map(|description| quote!(#[doc=$(quoted(description))])))
                        pub $(&property.name): $(&property.ty),
                    )
                }
            ),
        });
        write_tokens(&path, tokens)?;
        Ok(())
    }
}
