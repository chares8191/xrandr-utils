use std::collections::HashSet;
use std::env;
use std::io::{self, IsTerminal, Read, Write};
use std::process::{Command, Stdio};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args();
    let _binary = args.next();
    let command = match args.next() {
        Some(value) => value,
        None => {
            print_usage();
            return Ok(());
        }
    };

    if matches!(command.as_str(), "--help" | "-h" | "help") {
        print_usage();
        return Ok(());
    }

    let verbose = get_verbose_text()?;
    let sections = parse_sections(&verbose);

    match command.as_str() {
        "display_connected" => {
            let display = expect_arg(&mut args, "display")?;
            let section = find_section(&sections, &display)
                .ok_or_else(|| format!("display not found: {display}"))?;
            println!("{}", section.state.as_str());
        }
        "single_display_output" => {
            let keep = expect_arg(&mut args, "display")?;
            run_single_display_output(&keep, &sections)?;
        }
        "dual_display_output" => {
            let left = expect_arg(&mut args, "left display")?;
            let right = expect_arg(&mut args, "right display")?;
            run_dual_display_output(&left, &right, &sections)?;
        }
        "display_connected_map" => {
            let flags = parse_map_flags(&mut args, false)?;
            for section in &sections {
                let value = section.state.as_str();
                if should_skip_map_value(value, &flags) {
                    continue;
                }
                println!("{}={}", section.name, value);
            }
        }
        "display_section" => {
            let display = expect_arg(&mut args, "display")?;
            let section = find_section(&sections, &display)
                .ok_or_else(|| format!("display not found: {display}"))?;
            let text = section.lines.join("\n");
            if text.is_empty() {
                return Err("section is empty".to_string());
            }
            println!("{text}");
        }
        "display_section_map" => {
            let flags = parse_map_flags(&mut args, false)?;
            for section in &sections {
                let text = section.lines.join("\n");
                let escaped = escape_multiline(&text);
                if should_skip_map_value(&escaped, &flags) {
                    continue;
                }
                println!("{}={}", section.name, escaped);
            }
        }
        "display_edid" => {
            let display = expect_arg(&mut args, "display")?;
            let section = find_section(&sections, &display)
                .ok_or_else(|| format!("display not found: {display}"))?;
            let edid = extract_edid_hex(section).ok_or_else(|| {
                format!("edid data not available for display: {display}")
            })?;
            println!("{edid}");
        }
        "display_edid_decoded" => {
            let display = expect_arg(&mut args, "display")?;
            let section = find_section(&sections, &display)
                .ok_or_else(|| format!("display not found: {display}"))?;
            let edid = extract_edid_hex(section).ok_or_else(|| {
                format!("edid data not available for display: {display}")
            })?;
            let decoded = decode_edid(&edid)?;
            print!("{decoded}");
            if !decoded.ends_with('\n') {
                println!();
            }
        }
        "display_serial" => {
            let display = expect_arg(&mut args, "display")?;
            let section = find_section(&sections, &display)
                .ok_or_else(|| format!("display not found: {display}"))?;
            let edid = extract_edid_hex(section).ok_or_else(|| {
                format!("edid data not available for display: {display}")
            })?;
            let decoded = decode_edid(&edid)?;
            let serial = extract_serial(&decoded)
                .ok_or_else(|| format!("serial not found in edid for: {display}"))?;
            println!("{serial}");
        }
        "display_serial_map" => {
            let flags = parse_map_flags(&mut args, false)?;
            for section in &sections {
                let serial = match extract_edid_hex(section) {
                    Some(edid) => match decode_edid(&edid) {
                        Ok(decoded) => extract_serial(&decoded).unwrap_or_default(),
                        Err(_) => String::new(),
                    },
                    None => String::new(),
                };
                if should_skip_map_value(serial.as_str(), &flags) {
                    continue;
                }
                println!("{}={}", section.name, serial);
            }
        }
        "display_names" => {
            let connected_only = parse_display_names_flags(&mut args)?;
            for section in &sections {
                if connected_only && section.state != DisplayState::Connected {
                    continue;
                }
                println!("{}", section.name);
            }
        }
        "display_geometry" => {
            let display = expect_arg(&mut args, "display")?;
            let section = find_section(&sections, &display)
                .ok_or_else(|| format!("display not found: {display}"))?;
            if section.state != DisplayState::Connected {
                return Err(format!("display not connected: {display}"));
            }
            let geometry = section.geometry.clone().ok_or_else(|| {
                format!("geometry not available for display: {display}")
            })?;
            println!("{geometry}");
        }
        "display_geometry_map" => {
            let flags = parse_map_flags(&mut args, false)?;
            for section in &sections {
                if section.state != DisplayState::Connected {
                    continue;
                }
                if let Some(geometry) = &section.geometry {
                    let value = if section.primary {
                        format!("primary,{}", geometry)
                    } else {
                        geometry.clone()
                    };
                    if should_skip_map_value(value.as_str(), &flags) {
                        continue;
                    }
                    println!("{}={}", section.name, value);
                }
            }
        }
        "display_connector" => {
            let display = expect_arg(&mut args, "display")?;
            let section = find_section(&sections, &display)
                .ok_or_else(|| format!("display not found: {display}"))?;
            let connector = extract_connector_id(section)
                .ok_or_else(|| format!("connector id not available for: {display}"))?;
            println!("{connector}");
        }
        "display_connector_map" => {
            let flags = parse_map_flags(&mut args, false)?;
            for section in &sections {
                let connector = extract_connector_id(section).unwrap_or_default();
                if should_skip_map_value(connector.as_str(), &flags) {
                    continue;
                }
                println!("{}={}", section.name, connector);
            }
        }
        "display_label_line" => {
            let display = expect_arg(&mut args, "display")?;
            let section = find_section(&sections, &display)
                .ok_or_else(|| format!("display not found: {display}"))?;
            if let Some(line) = section.lines.first() {
                println!("{line}");
            } else {
                return Err(format!("label line missing for display: {display}"));
            }
        }
        _ => return Err(format!("unknown command: {command}")),
    }

    Ok(())
}

fn run_single_display_output(keep: &str, sections: &[DisplaySection]) -> Result<(), String> {
    if find_section(sections, keep).is_none() {
        return Err(format!("display not found: {keep}"));
    }

    let mut exclude = HashSet::new();
    exclude.insert(keep.to_string());

    let off_targets = filtered_display_names(sections, &exclude);
    let mut args = vec![
        "--output".to_string(),
        keep.to_string(),
        "--primary".to_string(),
        "--auto".to_string(),
    ];
    args.extend(build_off_args(&off_targets));

    run_xrandr_with_args(args)
}

fn run_dual_display_output(left: &str, right: &str, sections: &[DisplaySection]) -> Result<(), String> {
    if left == right {
        return Err("left and right displays must be different".to_string());
    }

    if find_section(sections, left).is_none() {
        return Err(format!("display not found: {left}"));
    }
    if find_section(sections, right).is_none() {
        return Err(format!("display not found: {right}"));
    }

    let mut exclude = HashSet::new();
    exclude.insert(left.to_string());
    exclude.insert(right.to_string());

    let off_targets = filtered_display_names(sections, &exclude);

    let mut args = vec![
        "--output".to_string(),
        left.to_string(),
        "--primary".to_string(),
        "--auto".to_string(),
        "--output".to_string(),
        right.to_string(),
        "--auto".to_string(),
        "--right-of".to_string(),
        left.to_string(),
    ];
    args.extend(build_off_args(&off_targets));

    run_xrandr_with_args(args)
}

fn filtered_display_names(sections: &[DisplaySection], exclude: &HashSet<String>) -> Vec<String> {
    sections
        .iter()
        .map(|section| section.name.as_str())
        .filter(|name| !exclude.contains(*name))
        .map(|name| name.to_string())
        .collect()
}

fn build_off_args(displays: &[String]) -> Vec<String> {
    let mut args = Vec::new();
    for display in displays {
        args.push("--output".to_string());
        args.push(display.clone());
        args.push("--off".to_string());
    }
    args
}

fn run_xrandr_with_args(args: Vec<String>) -> Result<(), String> {
    let status = Command::new("xrandr")
        .args(&args)
        .status()
        .map_err(|err| format!("failed to run xrandr: {err}"))?;

    if !status.success() {
        return Err(format!("xrandr command failed: {status}"));
    }

    Ok(())
}

fn expect_arg(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing argument: {name}"))
}

fn get_verbose_text() -> Result<String, String> {
    let mut stdin = io::stdin();
    if !stdin.is_terminal() {
        let mut buf = String::new();
        stdin
            .read_to_string(&mut buf)
            .map_err(|err| format!("failed to read stdin: {err}"))?;
        if buf.trim().is_empty() {
            return Err("stdin supplied but empty".to_string());
        }
        Ok(buf)
    } else {
        let output = Command::new("xrandr")
            .arg("--verbose")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .map_err(|err| format!("failed to run xrandr --verbose: {err}"))?;
        if !output.status.success() {
            return Err("xrandr --verbose exited with failure".to_string());
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[derive(Clone, Copy, PartialEq)]
enum DisplayState {
    Connected,
    Disconnected,
}

impl DisplayState {
    fn as_str(self) -> &'static str {
        match self {
            DisplayState::Connected => "connected",
            DisplayState::Disconnected => "disconnected",
        }
    }
}

struct DisplaySection {
    name: String,
    state: DisplayState,
    primary: bool,
    geometry: Option<String>,
    lines: Vec<String>,
}

fn parse_sections(verbose: &str) -> Vec<DisplaySection> {
    let mut sections = Vec::new();
    let mut current: Option<DisplaySection> = None;

    for line in verbose.lines() {
        if let Some(header) = parse_header(line) {
            if let Some(section) = current.take() {
                sections.push(section);
            }
            current = Some(DisplaySection {
                name: header.name,
                state: header.state,
                primary: header.primary,
                geometry: header.geometry,
                lines: vec![line.to_string()],
            });
        } else if let Some(section) = current.as_mut() {
            section.lines.push(line.to_string());
        }
    }

    if let Some(section) = current {
        sections.push(section);
    }

    sections
}

#[derive(Default)]
struct MapFlags {
    filtered: bool,
}

fn parse_display_names_flags(
    args: &mut impl Iterator<Item = String>,
) -> Result<bool, String> {
    let mut connected_only = false;
    for arg in args {
        match arg.as_str() {
            "--connected" => connected_only = true,
            _ => return Err(format!("unknown option: {arg}")),
        }
    }
    Ok(connected_only)
}

fn parse_map_flags(
    args: &mut impl Iterator<Item = String>,
    _allow_transposed: bool,
) -> Result<MapFlags, String> {
    let mut flags = MapFlags::default();
    for arg in args {
        match arg.as_str() {
            "--filtered" => flags.filtered = true,
            _ => return Err(format!("unknown option: {arg}")),
        }
    }
    Ok(flags)
}

fn should_skip_map_value(value: &str, flags: &MapFlags) -> bool {
    if !flags.filtered {
        return false;
    }
    value.trim().is_empty()
}

struct HeaderInfo {
    name: String,
    state: DisplayState,
    primary: bool,
    geometry: Option<String>,
}

fn parse_header(line: &str) -> Option<HeaderInfo> {
    let mut parts = line.split_whitespace();
    let name = parts.next()?;
    let state_word = parts.next()?;

    let state = match state_word {
        "connected" => DisplayState::Connected,
        "disconnected" => DisplayState::Disconnected,
        _ => return None,
    };

    let mut primary = false;
    let mut geometry = None;

    for token in parts {
        if token == "primary" {
            primary = true;
        } else if geometry.is_none() && is_geometry_token(token) {
            geometry = Some(token.to_string());
        }
    }

    Some(HeaderInfo {
        name: name.to_string(),
        state,
        primary,
        geometry,
    })
}

fn is_geometry_token(token: &str) -> bool {
    let bytes = token.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return false;
    }

    let mut index = 0;
    match consume_digits(bytes, index) {
        Some(next) => index = next,
        None => return false,
    }
    if index >= len || bytes[index] != b'x' {
        return false;
    }
    index += 1;
    match consume_digits(bytes, index) {
        Some(next) => index = next,
        None => return false,
    }

    match consume_signed_number(bytes, index) {
        Some(next) => index = next,
        None => return false,
    }
    match consume_signed_number(bytes, index) {
        Some(next) => index = next,
        None => return false,
    }

    index == len
}

fn consume_digits(bytes: &[u8], mut index: usize) -> Option<usize> {
    if index >= bytes.len() || !bytes[index].is_ascii_digit() {
        return None;
    }
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    Some(index)
}

fn consume_signed_number(bytes: &[u8], mut index: usize) -> Option<usize> {
    if index >= bytes.len() {
        return None;
    }

    let sign = bytes[index];
    if sign != b'+' && sign != b'-' {
        return None;
    }
    index += 1;

    if index >= bytes.len() || !bytes[index].is_ascii_digit() {
        return None;
    }

    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }

    Some(index)
}

fn find_section<'a>(sections: &'a [DisplaySection], name: &str) -> Option<&'a DisplaySection> {
    sections.iter().find(|section| section.name == name)
}

fn escape_multiline(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\n', "\\n")
}

fn extract_edid_hex(section: &DisplaySection) -> Option<String> {
    let mut capture = false;
    let mut hex = String::new();

    for line in &section.lines {
        let trimmed = line.trim();
        if trimmed.starts_with("EDID:") {
            capture = true;
            continue;
        }
        if capture {
            if trimmed.is_empty() {
                break;
            }
            if trimmed
                .chars()
                .all(|ch| ch.is_ascii_hexdigit() || ch.is_ascii_whitespace())
            {
                for ch in trimmed.chars() {
                    if ch.is_ascii_hexdigit() {
                        hex.push(ch);
                    }
                }
            } else {
                break;
            }
        }
    }

    if hex.is_empty() {
        None
    } else {
        Some(hex)
    }
}

fn extract_connector_id(section: &DisplaySection) -> Option<String> {
    for line in &section.lines {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("CONNECTOR_ID:") {
            let value = rest.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn decode_edid(hex: &str) -> Result<String, String> {
    let bytes = hex_to_bytes(hex)?;
    let mut child = Command::new("edid-decode")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("failed to run edid-decode: {err}"))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(&bytes)
            .map_err(|err| format!("failed to write edid to decoder: {err}"))?;
    } else {
        return Err("failed to open edid-decode stdin".to_string());
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed to read edid-decode output: {err}"))?;

    if !output.status.success() {
        return Err("edid-decode exited with failure".to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let chars: Vec<char> = hex.chars().filter(|ch| !ch.is_ascii_whitespace()).collect();

    if chars.len() % 2 != 0 {
        return Err("edid hex length is not even".to_string());
    }

    let mut index = 0;
    while index < chars.len() {
        let hi = chars[index];
        let lo = chars[index + 1];
        let value = hex_pair_to_byte(hi, lo)
            .ok_or_else(|| format!("invalid hex pair: {hi}{lo}"))?;
        bytes.push(value);
        index += 2;
    }

    Ok(bytes)
}

fn hex_pair_to_byte(hi: char, lo: char) -> Option<u8> {
    let high = hi.to_digit(16)? as u8;
    let low = lo.to_digit(16)? as u8;
    Some((high << 4) | low)
}

fn extract_serial(decoded: &str) -> Option<String> {
    for line in decoded.lines() {
        if let Some(value) = extract_between_quotes(line, "Display Product Serial Number:") {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }

    for line in decoded.lines() {
        if let Some(value) = extract_after_colon(line, "Serial Number:") {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }

    for line in decoded.lines() {
        if let Some(value) = extract_between_quotes(line, "Alphanumeric Data String:") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    None
}

fn extract_between_quotes(line: &str, label: &str) -> Option<String> {
    if !line.contains(label) {
        return None;
    }
    let start = line.find('\'')?;
    let end = line[start + 1..].find('\'')?;
    Some(line[start + 1..start + 1 + end].trim().to_string())
}

fn extract_after_colon(line: &str, label: &str) -> Option<String> {
    if !line.contains(label) {
        return None;
    }
    let idx = line.find(':')?;
    Some(line[idx + 1..].trim().to_string())
}

fn print_usage() {
    println!(
        "Usage: xrandr-utils <command> [args]\n\n\
Commands:\n  \
display_connected <display>\n  \
display_connected_map [--filtered]\n  \
display_section <display>\n  \
display_section_map [--filtered]\n  \
display_edid <display>\n  \
display_edid_decoded <display>\n  \
display_serial <display>\n  \
display_serial_map [--filtered]\n  \
display_connector <display>\n  \
display_connector_map [--filtered]\n  \
display_names [--connected]\n  \
display_geometry <display>\n  \
display_geometry_map [--filtered]\n  \
display_label_line <display>\n  \
single_display_output <display>\n  \
dual_display_output <left> <right>\n"
    );
}
