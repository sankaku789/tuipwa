mod category;
mod cli;
mod icons;
mod meta;
mod model;
mod platform;
mod registry;
#[cfg(test)]
mod tests;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn cli(args: &[String]) -> Result<i32> {
    // The internal session helper is not part of the public interface and is
    // therefore not listed in the help text.
    if args.first().map(String::as_str) == Some("__session") && args.len() == 3 {
        platform::linux::runtime::session(&args[1], &args[2])?;
        return Ok(0);
    }
    cli::run(args)
}

fn main() {
    let code = match cli(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("TuiPWA Native: {e}");
            1
        }
    };
    std::process::exit(code);
}
