use std::{collections::HashMap, sync::Mutex};

use crate::{err, Error};
use heck::ToSnakeCase;
use hyper::Method;

use once_cell::sync::OnceCell;
use openapiv3::{PathItem, ReferenceOr};

static OPERATIONS: OnceCell<Mutex<HashMap<String, Operation>>> = OnceCell::new();

#[derive(Clone)]
pub struct Operation {
    name: String,
    path: String,
    method: Method,
}

impl Operation {
    pub fn all() -> Vec<Operation> {
        OPERATIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    fn add(operation: Operation) {
        let mut operations = OPERATIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        if operations.contains_key(&operation.name) {
            panic!("Operation {} already exists", operation.name);
        }
        operations.insert(operation.name.clone(), operation);
    }

    fn get(name: &str) -> Option<Operation> {
        OPERATIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .get(name)
            .cloned()
    }

    pub fn discover_all_from_path(path: &str, schema: &ReferenceOr<PathItem>) -> Result<(), Error> {
        match schema {
            ReferenceOr::Reference { reference, .. } => {
                return err!("References not implemented: {}", reference);
            }
            ReferenceOr::Item(item) => {
                println!("{item:#?}");
                if let Some(op) = &item.get {
                    Self::discover(path, Method::GET, op.clone())?;
                }
                if let Some(op) = &item.put {
                    Self::discover(path, Method::PUT, op.clone())?;
                }
                if let Some(op) = &item.post {
                    Self::discover(path, Method::POST, op.clone())?;
                }
                if let Some(op) = &item.delete {
                    Self::discover(path, Method::DELETE, op.clone())?;
                }
                if let Some(op) = &item.options {
                    Self::discover(path, Method::OPTIONS, op.clone())?;
                }
                if let Some(op) = &item.head {
                    Self::discover(path, Method::HEAD, op.clone())?;
                }
                if let Some(op) = &item.patch {
                    Self::discover(path, Method::PATCH, op.clone())?;
                }
                if let Some(op) = &item.trace {
                    Self::discover(path, Method::TRACE, op.clone())?;
                }
            }
        }
        Ok(())
    }

    pub fn discover(path: &str, method: Method, schema: openapiv3::Operation) -> Result<(), Error> {
        let name = match schema.operation_id {
            Some(name) => name.to_snake_case(),
            None => {
                return err!("Operation is missing operationId: {}", path);
            }
        };
        Operation::add(Operation {
            name,
            path: path.to_string(),
            method,
        });
        Ok(())
    }
}
