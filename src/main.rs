fn main() {
    match run() {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{e}");
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let workload = colmenar::init()?;
    colmenar::generate(workload)?;
    Ok(())
}
