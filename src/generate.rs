use crate::Error;
pub use genco::{
    prelude::rust::{self, Tokens},
    quote,
    tokens::quoted,
};

pub fn write_tokens(path: &str, tokens: Tokens) -> Result<(), Error> {
    std::fs::write(path, tokens.to_file_string()?)?;
    Ok(())
}