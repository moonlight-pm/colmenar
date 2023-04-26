use crate::generate::*;

#[derive(Clone)]
pub struct Property {
    pub name: String,
    pub ty: Tokens,
    pub description: Option<String>,
    pub required: bool,
}

impl Property {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ty: quote!(String),
            description: None,
            required: false,
        }
    }
}
