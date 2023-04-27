use openapiv3::{Schema, SchemaKind, Type};

#[derive(Clone)]
pub enum Enumeration {
    String(Vec<String>),
    Integer(Vec<i64>),
    Object(Vec<String>),
}

impl Enumeration {
    pub fn discover(schema: &Schema) -> Option<Self> {
        return match &schema.schema_kind {
            SchemaKind::Type(Type::String(string)) => {
                if string.enumeration.is_empty() {
                    return None;
                }
                Some(Enumeration::String(
                    string
                        .enumeration
                        .iter()
                        .filter_map(|s| s.as_ref().map(|s| s.clone()))
                        .collect(),
                ))
            }
            SchemaKind::Type(Type::Integer(integer)) => {
                if integer.enumeration.is_empty() {
                    return None;
                }
                Some(Enumeration::Integer(
                    integer
                        .enumeration
                        .iter()
                        .filter_map(|i| i.map(|i| i))
                        .collect(),
                ))
            }
            _ => {
                return None;
            }
        };
    }
}
