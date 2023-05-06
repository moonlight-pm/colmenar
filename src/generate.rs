use crate::{err, Error};
pub use genco::{
    prelude::rust::{self, import, Tokens},
    quote, quote_in,
    tokens::quoted,
};
use std::process::Command;

pub fn write_tokens(path: &str, tokens: Tokens) -> Result<(), Error> {
    let path = std::path::Path::new(path);
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir)?;
    std::fs::write(path, tokens.to_file_string()?)?;
    Ok(())
}

pub fn format(dir: &str) -> Result<(), Error> {
    println!("Formatting...");
    let output = Command::new("bash")
        .args([
            "-c",
            &format!("find {dir} -type f | xargs rustfmt --edition 2021",),
        ])
        .output()?;
    if !output.status.success() {
        return err!(
            "Error: could not format generated code: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    println!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}
