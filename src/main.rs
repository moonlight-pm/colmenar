use colmenar::{Api, Error};

fn main() {
    match run() {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{e}");
        }
    }
}

fn run() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        println!("Usage: {} <schema-file> <output-directory>", args[0]);
        std::process::exit(1);
    }
    Api::new(&args[1], &args[2])?.generate()?;
    Ok(())
}
