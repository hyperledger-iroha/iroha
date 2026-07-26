#![deny(warnings)]
//! Emit a Norito JSON inventory of remaining Clippy warnings per crate.

use std::{
    collections::BTreeMap,
    fs,
    io::{self, BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
};

use norito::json::{self, Value};

const REPORT_PATH: &str = "target/clippy-inventory.norito.json";
const INVENTORY_PREFIX: &str = "clippy inventory:";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClippyWarning {
    code: Option<String>,
    message: String,
    file: Option<String>,
    line: Option<u64>,
}

type DynError = Box<dyn std::error::Error + Send + Sync + 'static>;

fn main() -> Result<(), DynError> {
    run()
}

fn run() -> Result<(), DynError> {
    let mut command = Command::new("cargo");
    command
        .arg("clippy")
        .arg("--workspace")
        .arg("--all-targets")
        .arg("--message-format=json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "missing stdout from cargo"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "missing stderr from cargo"))?;

    let mut stderr_reader = BufReader::new(stderr);
    let stderr_handle = std::thread::spawn(move || -> io::Result<()> {
        let mut buffer = String::new();
        while stderr_reader.read_line(&mut buffer)? != 0 {
            eprint!("{buffer}");
            buffer.clear();
        }
        Ok(())
    });

    let mut warnings: BTreeMap<String, Vec<ClippyWarning>> = BTreeMap::new();
    let mut stdout_reader = BufReader::new(stdout);
    let mut line = String::new();

    while stdout_reader.read_line(&mut line)? != 0 {
        if let Some(warning) = parse_warning_line(line.trim()) {
            warnings.entry(warning.0).or_default().push(warning.1);
        }
        line.clear();
    }

    let status = child.wait()?;
    if let Err(err) = stderr_handle.join().unwrap() {
        eprintln!("{INVENTORY_PREFIX} failed to read cargo stderr: {err}");
    }

    emit_summary(&warnings)?;
    if !status.success() {
        return Err(format!("cargo clippy exited with status {status}").into());
    }
    if !warnings.is_empty() {
        return Err(format!(
            "clippy warnings detected; inspect {REPORT_PATH} for the full inventory"
        )
        .into());
    }
    Ok(())
}

fn parse_warning_line(line: &str) -> Option<(String, ClippyWarning)> {
    if !line.starts_with('{') {
        return None;
    }

    let value: Value = match json::from_str(line) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("{INVENTORY_PREFIX} failed to parse JSON line: {err}");
            return None;
        }
    };

    if !matches!(value.get("reason"), Some(Value::String(s)) if s == "compiler-message") {
        return None;
    }

    let package_id = value.get("package_id").and_then(Value::as_str)?;
    let crate_name = package_short_name(package_id).to_owned();

    let message = value.get("message")?;

    if !matches!(message.get("level"), Some(Value::String(level)) if level == "warning") {
        return None;
    }

    let text = message
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| message.get("rendered").and_then(Value::as_str))
        .unwrap_or("")
        .trim()
        .to_owned();

    let code = message
        .get("code")
        .and_then(|code| code.get("code"))
        .and_then(Value::as_str)
        .map(str::to_owned);

    let (file, line) = extract_primary_span(message.get("spans"));

    Some((
        crate_name,
        ClippyWarning {
            code,
            message: text,
            file,
            line,
        },
    ))
}

fn extract_primary_span(spans: Option<&Value>) -> (Option<String>, Option<u64>) {
    let mut primary: Option<&Value> = None;
    if let Some(Value::Array(list)) = spans {
        for entry in list {
            if matches!(entry.get("is_primary"), Some(Value::Bool(true))) {
                primary = Some(entry);
                break;
            }
        }
        if primary.is_none() {
            primary = list.first();
        }
    }

    if let Some(Value::Object(obj)) = primary {
        let file = obj
            .get("file_name")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let line = obj.get("line_start").and_then(Value::as_u64);
        (file, line)
    } else {
        (None, None)
    }
}

fn package_short_name(package_id: &str) -> &str {
    package_id.split([' ', '#']).next().unwrap_or(package_id)
}

fn emit_summary(warnings: &BTreeMap<String, Vec<ClippyWarning>>) -> Result<(), DynError> {
    if warnings.is_empty() {
        println!("{INVENTORY_PREFIX} no warnings detected");
    } else {
        println!(
            "{INVENTORY_PREFIX} {} crate(s) emit warnings; see {REPORT_PATH}",
            warnings.len()
        );
        for (crate_name, entries) in warnings {
            println!("  - {crate_name}: {} warning(s)", entries.len());
            for warning in entries {
                let location = match (&warning.file, warning.line) {
                    (Some(file), Some(line)) => format!("{file}:{line}"),
                    (Some(file), None) => file.clone(),
                    _ => String::from("<unknown location>"),
                };
                match &warning.code {
                    Some(code) => println!("    * [{code}] {location} — {}", warning.message),
                    None => println!("    * {location} — {}", warning.message),
                }
            }
        }
    }

    let mut crates_report = Vec::new();
    for (crate_name, entries) in warnings {
        let mut warning_values = Vec::with_capacity(entries.len());
        for entry in entries {
            let fields = [
                ("message", json::to_value(&entry.message)?),
                (
                    "code",
                    match &entry.code {
                        Some(code) => json::to_value(code)?,
                        None => Value::Null,
                    },
                ),
                (
                    "file",
                    match &entry.file {
                        Some(file) => json::to_value(file)?,
                        None => Value::Null,
                    },
                ),
                (
                    "line",
                    match entry.line {
                        Some(line) => json::to_value(&line)?,
                        None => Value::Null,
                    },
                ),
            ];
            warning_values.push(json::object(fields)?);
        }
        crates_report.push(json::object([
            ("crate", json::to_value(crate_name)?),
            ("warnings", json::array(warning_values.into_iter())?),
        ])?);
    }

    let report = json::object([("crates", json::array(crates_report.into_iter())?)])?;

    let output = json::to_json_pretty(&report)?;
    let path = PathBuf::from(REPORT_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_package_name() {
        assert_eq!(package_short_name("crate_a 0.1.0"), "crate_a");
        assert_eq!(
            package_short_name("crate_b 0.2.0 (path+file://foo)"),
            "crate_b"
        );
        assert_eq!(package_short_name("crate_c#meta"), "crate_c");
    }

    #[test]
    fn parses_warning_line() {
        let sample = r#"{
            "reason": "compiler-message",
            "package_id": "demo 0.1.0 (path+file:///demo)",
            "message": {
                "message": "unused variable: `x`",
                "code": { "code": "clippy::unused_variable" },
                "level": "warning",
                "spans": [
                    {
                        "file_name": "src/lib.rs",
                        "line_start": 42,
                        "is_primary": true
                    }
                ]
            }
        }"#;
        let parsed = parse_warning_line(sample).unwrap();
        assert_eq!(parsed.0, "demo");
        assert_eq!(parsed.1.code.as_deref(), Some("clippy::unused_variable"));
        assert_eq!(parsed.1.message, "unused variable: `x`");
        assert_eq!(parsed.1.file.as_deref(), Some("src/lib.rs"));
        assert_eq!(parsed.1.line, Some(42));
    }

    #[test]
    fn primary_span_fallbacks() {
        let spans = json::array([
            json::object([
                ("file_name", json::to_value("second.rs").unwrap()),
                ("line_start", json::to_value(&2_u64).unwrap()),
                ("is_primary", json::to_value(&false).unwrap()),
            ])
            .unwrap(),
            json::object([
                ("file_name", json::to_value("first.rs").unwrap()),
                ("line_start", json::to_value(&1_u64).unwrap()),
                ("is_primary", json::to_value(&true).unwrap()),
            ])
            .unwrap(),
        ])
        .unwrap();

        let (file, line) = extract_primary_span(Some(&spans));
        assert_eq!(file.as_deref(), Some("first.rs"));
        assert_eq!(line, Some(1));

        let spans_no_primary = json::array([json::object([
            ("file_name", json::to_value("fallback.rs").unwrap()),
            ("line_start", json::to_value(&9_u64).unwrap()),
        ])
        .unwrap()])
        .unwrap();
        let (fallback_file, fallback_line) = extract_primary_span(Some(&spans_no_primary));
        assert_eq!(fallback_file.as_deref(), Some("fallback.rs"));
        assert_eq!(fallback_line, Some(9));
    }
}
