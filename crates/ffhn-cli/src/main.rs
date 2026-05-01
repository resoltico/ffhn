#![forbid(unsafe_code)]

use std::io;

fn main() {
    let code = ffhn_cli::run(
        std::env::args_os(),
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    );
    std::process::exit(code);
}
