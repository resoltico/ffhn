use std::ffi::{OsStr, OsString};
use std::io::{self, Write};

use crate::args::build_cli_command;
use crate::metadata::version_banner;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TopLevelRequest {
    Help,
    Version,
}

pub(crate) fn try_handle_top_level_request(
    raw_args: &[OsString],
    stdout: &mut impl Write,
) -> io::Result<bool> {
    let Some(request) = detect_top_level_request(raw_args) else {
        return Ok(false);
    };

    match request {
        TopLevelRequest::Help => write_root_help(stdout)?,
        TopLevelRequest::Version => writeln!(stdout, "{}", version_banner())?,
    }

    Ok(true)
}

fn detect_top_level_request(raw_args: &[OsString]) -> Option<TopLevelRequest> {
    if raw_args.len() <= 1 {
        return Some(TopLevelRequest::Help);
    }

    let args = &raw_args[1..];
    if let Some(rest) = args
        .first()
        .and_then(|arg| (*arg == OsStr::new("help")).then_some(&args[1..]))
        && help_subcommand_requests_root_help(rest)
    {
        return Some(TopLevelRequest::Help);
    }

    let mut saw_help = false;
    let mut saw_version = false;
    for arg in args {
        if arg == "--" {
            return None;
        }
        if parse_known_top_level_flags(arg, &mut saw_help, &mut saw_version) {
            continue;
        }
        return None;
    }

    if saw_help {
        return Some(TopLevelRequest::Help);
    }
    saw_version.then_some(TopLevelRequest::Version)
}

fn help_subcommand_requests_root_help(rest: &[OsString]) -> bool {
    if rest.is_empty() {
        return true;
    }

    let mut saw_help = false;
    let mut saw_version = false;
    for arg in rest {
        if !parse_known_top_level_flags(arg, &mut saw_help, &mut saw_version) {
            return false;
        }
    }
    saw_help || saw_version
}

fn parse_known_top_level_flags(arg: &OsStr, saw_help: &mut bool, saw_version: &mut bool) -> bool {
    let Some(arg) = arg.to_str() else {
        return false;
    };

    match arg {
        "--help" => {
            *saw_help = true;
            true
        }
        "--version" => {
            *saw_version = true;
            true
        }
        _ => {
            let Some(short_group) = arg.strip_prefix('-') else {
                return false;
            };
            if short_group.is_empty() || arg.starts_with("--") {
                return false;
            }

            for short_flag in short_group.chars() {
                match short_flag {
                    'h' => *saw_help = true,
                    'V' => *saw_version = true,
                    _ => return false,
                }
            }
            true
        }
    }
}

fn write_root_help(stdout: &mut impl Write) -> io::Result<()> {
    writeln!(stdout, "{}", version_banner())?;
    writeln!(stdout)?;

    let mut command = build_cli_command().about(None::<&str>);
    command.write_long_help(stdout)?;
    writeln!(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<OsString> {
        parts.iter().map(OsString::from).collect()
    }

    #[test]
    fn detect_top_level_request_classifies_supported_root_paths() {
        assert_eq!(
            detect_top_level_request(&args(&["ffhn"])),
            Some(TopLevelRequest::Help)
        );
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "--help"])),
            Some(TopLevelRequest::Help)
        );
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "--version"])),
            Some(TopLevelRequest::Version)
        );
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "--version", "--help"])),
            Some(TopLevelRequest::Help)
        );
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "--help", "--version"])),
            Some(TopLevelRequest::Help)
        );
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "-Vh"])),
            Some(TopLevelRequest::Help)
        );
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "help"])),
            Some(TopLevelRequest::Help)
        );
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "help", "--version"])),
            Some(TopLevelRequest::Help)
        );
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "help", "-h"])),
            Some(TopLevelRequest::Help)
        );
    }

    #[test]
    fn detect_top_level_request_leaves_subcommands_and_unknown_flags_to_clap() {
        assert_eq!(detect_top_level_request(&args(&["ffhn", "run"])), None);
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "run", "--help"])),
            None
        );
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "status", "--version"])),
            None
        );
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "bogus", "--version"])),
            None
        );
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "help", "run"])),
            None
        );
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "help", "-x"])),
            None
        );
        assert_eq!(detect_top_level_request(&args(&["ffhn", "-x"])), None);
        assert_eq!(detect_top_level_request(&args(&["ffhn", "--bogus"])), None);
        assert_eq!(
            detect_top_level_request(&args(&["ffhn", "--", "--help"])),
            None
        );
    }

    #[test]
    fn parse_known_top_level_flags_covers_short_groups_and_empty_groups() {
        let mut saw_help = false;
        let mut saw_version = false;
        assert!(parse_known_top_level_flags(
            OsStr::new("-hV"),
            &mut saw_help,
            &mut saw_version,
        ));
        assert!(saw_help);
        assert!(saw_version);

        let mut saw_help = false;
        let mut saw_version = false;
        assert!(!parse_known_top_level_flags(
            OsStr::new("-"),
            &mut saw_help,
            &mut saw_version,
        ));
    }

    #[cfg(unix)]
    #[test]
    fn parse_known_top_level_flags_rejects_non_utf8_arguments() {
        use std::os::unix::ffi::OsStringExt;

        let mut saw_help = false;
        let mut saw_version = false;
        assert!(!parse_known_top_level_flags(
            &OsString::from_vec(vec![b'-', 0xFF]),
            &mut saw_help,
            &mut saw_version,
        ));
        assert!(!saw_help);
        assert!(!saw_version);
    }
}
