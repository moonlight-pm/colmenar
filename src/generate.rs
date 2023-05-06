use crate::Error;
pub use genco::{
    prelude::rust::{self, import, Tokens},
    quote,
    tokens::quoted,
};

pub fn write_tokens(path: &str, tokens: Tokens) -> Result<(), Error> {
    let path = std::path::Path::new(path);
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir)?;
    std::fs::write(path, tokens.to_file_string()?)?;
    Ok(())
}
