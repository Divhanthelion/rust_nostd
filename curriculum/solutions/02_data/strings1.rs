//! # strings1: A zero-copy console parser
//!
//! Many ECUs and dev boards expose a text console over a UART. You'll write
//! its parser, plus a checksum validator for NMEA sentences (the text format
//! GPS receivers speak).
//!
//! **Zero-copy:** `Command` borrows from the input line (`&'a str`). Nothing is
//! copied, and nothing needs a heap.
//!
//! Grammar (verbs are case-insensitive, words separated by ASCII whitespace,
//! surrounding whitespace is ignored):
//!
//! ```text
//! set <key> <value>     value is an i32         → Command::Set
//! get <key>                                       → Command::Get
//! reset                                           → Command::Reset
//! echo <anything...>    text after the verb, trimmed, may be empty → Command::Echo
//! ```
//!
//! Errors: empty line → `Empty`; unknown verb → `UnknownCommand(verb)`;
//! missing key/value → `MissingArgument`; value not an i32 → `BadNumber`;
//! extra words after `set k v`, `get k` or `reset` → `TooManyArguments`.
#![no_std]

#[derive(Debug, PartialEq, Eq)]
pub enum Command<'a> {
    Set { key: &'a str, value: i32 },
    Get { key: &'a str },
    Reset,
    Echo(&'a str),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError<'a> {
    Empty,
    UnknownCommand(&'a str),
    MissingArgument,
    BadNumber,
    TooManyArguments,
}

pub fn parse_command(line: &str) -> Result<Command<'_>, ParseError<'_>> {
    let line = line.trim();
    let (verb, rest) = match line.split_once(|c: char| c.is_ascii_whitespace()) {
        Some((v, r)) => (v, r.trim()),
        None => (line, ""),
    };
    if verb.is_empty() {
        return Err(ParseError::Empty);
    }
    let mut args = rest.split_ascii_whitespace();
    let cmd = if verb.eq_ignore_ascii_case("set") {
        let key = args.next().ok_or(ParseError::MissingArgument)?;
        let value = args.next().ok_or(ParseError::MissingArgument)?;
        let value = value.parse().map_err(|_| ParseError::BadNumber)?;
        Command::Set { key, value }
    } else if verb.eq_ignore_ascii_case("get") {
        Command::Get { key: args.next().ok_or(ParseError::MissingArgument)? }
    } else if verb.eq_ignore_ascii_case("reset") {
        Command::Reset
    } else if verb.eq_ignore_ascii_case("echo") {
        return Ok(Command::Echo(rest));
    } else {
        return Err(ParseError::UnknownCommand(verb));
    };
    if args.next().is_some() {
        return Err(ParseError::TooManyArguments);
    }
    Ok(cmd)
}

/// The checksum of an NMEA sentence such as
/// `"$GPGGA,123519,4807.038,N*47"`: the XOR of every byte strictly between
/// the leading `'$'` and the `'*'`. `None` if the sentence doesn't start with
/// `'$'` or has no `'*'`.
pub fn nmea_checksum(sentence: &str) -> Option<u8> {
    let body = sentence.strip_prefix('$')?;
    let (payload, _) = body.split_once('*')?;
    Some(payload.bytes().fold(0, |acc, b| acc ^ b))
}

/// True if the two hex digits after `'*'` (either case, nothing else after
/// them except optional trailing `"\r\n"`) match the computed checksum.
pub fn nmea_valid(sentence: &str) -> bool {
    let Some(sum) = nmea_checksum(sentence) else { return false };
    let Some((_, hex)) = sentence.split_once('*') else { return false };
    let hex = hex.trim_end_matches("\r\n");
    hex.len() == 2 && u8::from_str_radix(hex, 16).is_ok_and(|v| v == sum)
}

/// Split `"key=value"` into trimmed parts. `None` without an `'='` or with an
/// empty key.
pub fn key_value(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.split_once('=')?;
    let k = k.trim();
    if k.is_empty() {
        return None;
    }
    Some((k, v.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_set() {
        assert_eq!(parse_command("set speed 120"), Ok(Command::Set { key: "speed", value: 120 }));
        assert_eq!(parse_command("  SET  temp   -40 \r\n"), Ok(Command::Set { key: "temp", value: -40 }));
        assert_eq!(parse_command("set speed"), Err(ParseError::MissingArgument));
        assert_eq!(parse_command("set"), Err(ParseError::MissingArgument));
        assert_eq!(parse_command("set speed fast"), Err(ParseError::BadNumber));
        assert_eq!(parse_command("set speed 99999999999"), Err(ParseError::BadNumber));
        assert_eq!(parse_command("set a 1 2"), Err(ParseError::TooManyArguments));
    }

    #[test]
    fn parses_get_reset_echo() {
        assert_eq!(parse_command("get mode"), Ok(Command::Get { key: "mode" }));
        assert_eq!(parse_command("Get"), Err(ParseError::MissingArgument));
        assert_eq!(parse_command("reset"), Ok(Command::Reset));
        assert_eq!(parse_command("RESET now"), Err(ParseError::TooManyArguments));
        assert_eq!(parse_command("echo hello   world "), Ok(Command::Echo("hello   world")));
        assert_eq!(parse_command("echo"), Ok(Command::Echo("")));
    }

    #[test]
    fn parse_errors() {
        assert_eq!(parse_command(""), Err(ParseError::Empty));
        assert_eq!(parse_command("   \t "), Err(ParseError::Empty));
        assert_eq!(parse_command("reboot now"), Err(ParseError::UnknownCommand("reboot")));
    }

    #[test]
    fn borrows_from_input() {
        let line = "get coolant_temp";
        if let Ok(Command::Get { key }) = parse_command(line) {
            // the key points into `line`: zero-copy
            assert!(line.as_bytes().as_ptr_range().contains(&key.as_ptr()));
        } else {
            panic!("expected Get");
        }
    }

    #[test]
    fn nmea() {
        let s = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47";
        assert_eq!(nmea_checksum(s), Some(0x47));
        assert!(nmea_valid(s));
        assert!(nmea_valid("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47\r\n"));
        assert!(!nmea_valid("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*48"));
        assert!(nmea_valid("$AB*03"));
        assert!(nmea_valid("$AB*03"));
        assert_eq!(nmea_checksum("GPGGA*47"), None);
        assert_eq!(nmea_checksum("$GPGGA"), None);
        assert!(!nmea_valid("$AB*3"));
        assert!(!nmea_valid("$AB*0x03"));
    }

    #[test]
    fn key_values() {
        assert_eq!(key_value(" mode = eco "), Some(("mode", "eco")));
        assert_eq!(key_value("a="), Some(("a", "")));
        assert_eq!(key_value("=x"), None);
        assert_eq!(key_value("novalue"), None);
        assert_eq!(key_value("url=a=b"), Some(("url", "a=b")));
    }
}
