use crate::prelude::*;
use once_cell::sync::OnceCell;
use std::{collections::HashMap, sync::Mutex};

static RESOURCES: OnceCell<Mutex<HashMap<String, Resource>>> = OnceCell::new();

#[derive(Clone)]
pub struct Resource {
    pub path: String,
    pub operations: Vec<String>,
}

impl Resource {
    pub fn all() -> Vec<Resource> {
        RESOURCES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    pub fn add(resource: Resource) {
        let mut resources = RESOURCES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        if resources.contains_key(&resource.path) {
            panic!("Resource {} already exists", resource.path);
        }
        resources.insert(resource.path.clone(), resource);
    }

    pub fn add_operation(path: &str, operation: &str) -> Result<(), Error> {
        let mut resources = RESOURCES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        if let Some(resource) = resources.get_mut(path) {
            if resource.operations.contains(&operation.to_string()) {
                return err!(
                    "Operation {} already exists for resource {}",
                    operation,
                    path
                );
            }
            resource.operations.push(operation.to_string());
        } else {
            let resource = Resource {
                path: path.to_string(),
                operations: vec![operation.to_string()],
            };
            resources.insert(path.to_string(), resource);
        }
        Ok(())
    }

    // fn get(path: &str) -> Option<Resource> {
    //     RESOURCES
    //         .get_or_init(|| Mutex::new(HashMap::new()))
    //         .lock()
    //         .unwrap()
    //         .get(path)
    //         .cloned()
    // }

    pub fn unversioned_path(&self) -> String {
        self.path.replace("/v1/", "")
    }

    pub fn tokens(&self) -> Result<Tokens, Error> {
        let mut tokens = Tokens::new();
        for op in &self.operations {
            let operation = Operation::get(op).unwrap();
            let name = operation.name.to_snake_case();
            let method = operation.method.as_str();
            let path = operation.path.clone();
            let description = operation.description.clone();
            quote_in! { tokens =>
                #[doc = $(quoted(description))]
                pub async fn $name(&self) -> Result<$(&operation.response), Error> {
                    $(match method {
                        "GET" => Ok(serde_json::from_value(self.get($(quoted(path))).await?)?),
                        _ => unimplemented!(),
                    })
                }
            }
            tokens.line();
        }
        Ok(tokens)
    }
}