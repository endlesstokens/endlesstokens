// SPDX-License-Identifier: MIT

use std::{env, process};

fn main() {
    let mut args = env::args().skip(1);

    match args.next().as_deref() {
        Some("--version") | Some("-V") | Some("version") => {
            println!("{}", version_text(env!("CARGO_PKG_VERSION")));
        }
        Some("--help") | Some("-h") | None => print_help(),
        Some(arg) => {
            eprintln!("unknown argument: {arg}");
            eprintln!("try 'eltk --help'");
            process::exit(2);
        }
    }
}

fn print_help() {
    println!("{}", help_text(env!("CARGO_PKG_VERSION")));
}

fn version_text(version: &str) -> String {
    format!("{} {version}", eltk_core::CLI_NAME)
}

fn help_text(version: &str) -> String {
    format!(
        "{program} {version}\n\n{product} token usage tracker.\n\nUsage: {program} [OPTIONS] [COMMAND]\n\nCommands:\n  version       Print version\n\nOptions:\n  -V, --version Print version\n  -h, --help    Print help",
        program = eltk_core::CLI_NAME,
        product = eltk_core::PRODUCT_NAME
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_version_text() {
        assert_eq!(version_text("0.0.0"), "eltk 0.0.0");
    }

    #[test]
    fn help_lists_supported_flags() {
        let help = help_text("0.0.0");
        assert!(help.starts_with("eltk 0.0.0"));
        assert!(help.contains("version"));
        assert!(help.contains("-V, --version"));
        assert!(help.contains("-h, --help"));
    }
}
