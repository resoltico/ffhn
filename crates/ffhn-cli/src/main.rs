use std::io;

fn main() {
    let code = ffhn_cli::run(
        std::env::args(),
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    );
    std::process::exit(code);
}
