use std::io::{self, Write};

use crate::json::{self, JsonSerialize, Value};

const INDENT: usize = 2;

/// Errors that can occur during YAML serialization.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("YAML IO error: {0}")]
    Io(#[from] io::Error),
    #[error("YAML JSON conversion error: {0}")]
    Json(#[from] crate::Error),
}

/// Serialize a value implementing [`JsonSerialize`] into YAML and write it to `writer`.
pub fn to_writer<T, W>(writer: W, value: &T) -> Result<(), Error>
where
    T: JsonSerialize,
    W: Write,
{
    let json_value = json::to_value(value).map_err(|err| Error::Json(err.into()))?;
    to_writer_from_value(writer, &json_value)
}

/// Serialize a [`Value`] into YAML and write it to `writer`.
pub fn to_writer_from_value<W>(mut writer: W, value: &Value) -> Result<(), Error>
where
    W: Write,
{
    write_value(&mut writer, value, 0)?;
    // Ensure the output is terminated with a newline for POSIX-friendly files.
    if !ends_with_newline(value) {
        writer.write_all(b"\n")?;
    }
    Ok(())
}

/// Serialize a value implementing [`JsonSerialize`] into a YAML string.
pub fn to_string<T>(value: &T) -> Result<String, Error>
where
    T: JsonSerialize,
{
    let json_value = json::to_value(value).map_err(|err| Error::Json(err.into()))?;
    to_string_from_value(&json_value)
}

/// Serialize a [`Value`] into a YAML string.
pub fn to_string_from_value(value: &Value) -> Result<String, Error> {
    let mut buf = Vec::new();
    to_writer_from_value(&mut buf, value)?;
    // Output is UTF-8 by construction because we only write ASCII plus the contents of
    // JSON strings, which are guaranteed to be UTF-8.
    Ok(String::from_utf8(buf).expect("norito::yaml emitted invalid UTF-8"))
}

fn write_value<W: Write>(writer: &mut W, value: &Value, indent: usize) -> Result<(), Error> {
    match value {
        Value::Object(map) => write_object(writer, map, indent),
        Value::Array(seq) => write_array(writer, seq, indent),
        _ => {
            write_indent(writer, indent)?;
            write_scalar(writer, value)?;
            writer.write_all(b"\n")?;
            Ok(())
        }
    }
}

fn write_object<W: Write>(writer: &mut W, map: &json::Map, indent: usize) -> Result<(), Error> {
    for (key, value) in map.iter() {
        write_indent(writer, indent)?;
        write_key(writer, key)?;
        match value {
            Value::Object(nested) if nested.is_empty() => {
                writer.write_all(b": {}\n")?;
            }
            Value::Object(nested) => {
                writer.write_all(b":\n")?;
                write_object(writer, nested, indent + INDENT)?;
            }
            Value::Array(seq) if seq.is_empty() => {
                writer.write_all(b": []\n")?;
            }
            Value::Array(seq) => {
                writer.write_all(b":\n")?;
                write_array(writer, seq, indent + INDENT)?;
            }
            Value::String(s) if s.contains('\n') => {
                writer.write_all(b": |-\n")?;
                write_block_string(writer, s, indent + INDENT)?;
            }
            _ => {
                writer.write_all(b": ")?;
                write_scalar(writer, value)?;
                writer.write_all(b"\n")?;
            }
        }
    }
    Ok(())
}

fn write_array<W: Write>(writer: &mut W, seq: &[Value], indent: usize) -> Result<(), Error> {
    if seq.is_empty() {
        write_indent(writer, indent)?;
        writer.write_all(b"[]\n")?;
        return Ok(());
    }

    for value in seq {
        write_indent(writer, indent)?;
        writer.write_all(b"- ")?;
        match value {
            Value::Object(map) if map.is_empty() => {
                writer.write_all(b"{}\n")?;
            }
            Value::Object(map) => {
                writer.write_all(b"\n")?;
                write_object(writer, map, indent + INDENT)?;
            }
            Value::Array(nested) if nested.is_empty() => {
                writer.write_all(b"[]\n")?;
            }
            Value::Array(nested) => {
                writer.write_all(b"\n")?;
                write_array(writer, nested, indent + INDENT)?;
            }
            Value::String(s) if s.contains('\n') => {
                writer.write_all(b"|-\n")?;
                write_block_string(writer, s, indent + INDENT)?;
            }
            _ => {
                write_scalar(writer, value)?;
                writer.write_all(b"\n")?;
            }
        }
    }
    Ok(())
}

fn write_scalar<W: Write>(writer: &mut W, value: &Value) -> Result<(), Error> {
    match value {
        Value::Null => writer.write_all(b"null")?,
        Value::Bool(true) => writer.write_all(b"true")?,
        Value::Bool(false) => writer.write_all(b"false")?,
        Value::Number(n) => writer.write_all(number_to_string(n).as_bytes())?,
        Value::String(s) => write_string_scalar(writer, s)?,
        Value::Array(_) => {
            // Inline representation for nested arrays when called as scalar (edge case).
            writer.write_all(b"[")?;
            write_inline_sequence(writer, value)?;
            writer.write_all(b"]")?;
        }
        Value::Object(_) => {
            writer.write_all(b"{")?;
            write_inline_map(writer, value)?;
            writer.write_all(b"}")?;
        }
    }
    Ok(())
}

fn write_inline_sequence<W: Write>(writer: &mut W, value: &Value) -> Result<(), Error> {
    if let Value::Array(seq) = value {
        let mut first = true;
        for item in seq {
            if !first {
                writer.write_all(b", ")?;
            }
            write_scalar(writer, item)?;
            first = false;
        }
    }
    Ok(())
}

fn write_inline_map<W: Write>(writer: &mut W, value: &Value) -> Result<(), Error> {
    if let Value::Object(map) = value {
        let mut first = true;
        for (k, v) in map.iter() {
            if !first {
                writer.write_all(b", ")?;
            }
            write_string_scalar(writer, k)?;
            writer.write_all(b": ")?;
            write_scalar(writer, v)?;
            first = false;
        }
    }
    Ok(())
}

fn write_block_string<W: Write>(writer: &mut W, input: &str, indent: usize) -> Result<(), Error> {
    let trimmed = input.trim_end_matches('\n');
    if trimmed.is_empty() {
        writer.write_all(b"\n")?;
        return Ok(());
    }

    for line in trimmed.split('\n') {
        write_indent(writer, indent)?;
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

fn write_string_scalar<W: Write>(writer: &mut W, input: &str) -> Result<(), Error> {
    if input.is_empty() {
        writer.write_all(b"''")?;
        return Ok(());
    }
    if is_plain_string(input) {
        writer.write_all(input.as_bytes())?;
        return Ok(());
    }
    // Fallback to single-quoted string with doubled quotes.
    writer.write_all(b"'")?;
    for ch in input.chars() {
        if ch == '\'' {
            writer.write_all(b"''")?;
        } else {
            let mut buf = [0u8; 4];
            writer.write_all(ch.encode_utf8(&mut buf).as_bytes())?;
        }
    }
    writer.write_all(b"'")?;
    Ok(())
}

fn write_key<W: Write>(writer: &mut W, key: &str) -> Result<(), Error> {
    write_string_scalar(writer, key)
}

fn write_indent<W: Write>(writer: &mut W, depth: usize) -> Result<(), Error> {
    for _ in 0..depth {
        writer.write_all(b" ")?;
    }
    Ok(())
}

fn ends_with_newline(value: &Value) -> bool {
    matches!(value, Value::Object(_) | Value::Array(_))
}

fn is_plain_string(input: &str) -> bool {
    if input.is_empty() {
        return false;
    }
    if input.starts_with([
        '-', '?', ':', ',', '[', ']', '{', '}', '&', '*', '#', '!', '|', '>', '\'', '"', '%', '@',
        '`',
    ]) {
        return false;
    }
    if input.ends_with(' ') || input.starts_with(' ') {
        return false;
    }
    input.bytes().all(|b| {
        matches!(b,
            b'a'..=b'z'
            | b'A'..=b'Z'
            | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'/' | b':' | b'@' | b'+'
        )
    })
}

fn number_to_string(number: &json::Number) -> String {
    match number {
        json::Number::I64(v) => v.to_string(),
        json::Number::U64(v) => v.to_string(),
        json::Number::F64(v) => {
            // Use `ryu` formatting via `to_string` for f64 preserving finite values.
            let mut s = v.to_string();
            if !s.contains(['.', 'e', 'E']) {
                s.push_str(".0");
            }
            s
        }
    }
}
