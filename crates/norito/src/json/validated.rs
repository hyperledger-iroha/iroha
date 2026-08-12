//! Direct checked output for already-validated JSON documents.

use super::{BoundedJsonError, JsonWriteSink};

/// Stream one already-validated JSON document into a checked sink.
///
/// Unlike a single [`JsonWriteSink::push_str`] call, this preserves structural
/// depth accounting for embedded arrays and objects. The caller must guarantee
/// that `value` is one complete valid JSON document; this helper deliberately
/// does not reparse or allocate a semantic value.
#[doc(hidden)]
pub fn write_validated_json_to(
    value: &str,
    output: &mut dyn JsonWriteSink,
) -> Result<(), BoundedJsonError> {
    let bytes = value.as_bytes();
    let mut chunk_start = 0;
    let mut in_string = false;
    let mut escaped = false;

    for (index, byte) in bytes.iter().copied().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else {
                match byte {
                    b'\\' => escaped = true,
                    b'"' => in_string = false,
                    _ => {}
                }
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                if chunk_start != index {
                    output.push_str(&value[chunk_start..index])?;
                }
                output.begin_container()?;
                output.push(char::from(byte))?;
                chunk_start = index + 1;
            }
            b'}' | b']' => {
                if chunk_start != index {
                    output.push_str(&value[chunk_start..index])?;
                }
                output.push(char::from(byte))?;
                output.end_container();
                chunk_start = index + 1;
            }
            _ => {}
        }
    }

    if chunk_start != bytes.len() {
        output.push_str(&value[chunk_start..])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingSink {
        output: String,
        depth: usize,
        max_depth: usize,
    }

    impl JsonWriteSink for RecordingSink {
        fn push(&mut self, value: char) -> Result<(), BoundedJsonError> {
            self.output.push(value);
            Ok(())
        }

        fn push_str(&mut self, value: &str) -> Result<(), BoundedJsonError> {
            self.output.push_str(value);
            Ok(())
        }

        fn begin_container(&mut self) -> Result<(), BoundedJsonError> {
            self.depth += 1;
            self.max_depth = self.max_depth.max(self.depth);
            Ok(())
        }

        fn end_container(&mut self) {
            self.depth -= 1;
        }
    }

    #[test]
    fn validated_writer_preserves_bytes_and_tracks_only_structural_delimiters() {
        let value = r#"{"literal":"[{}] and \"quoted\"","nested":[{},[1]]}"#;
        let mut sink = RecordingSink::default();

        write_validated_json_to(value, &mut sink).expect("write validated JSON");

        assert_eq!(sink.output, value);
        assert_eq!(sink.depth, 0);
        assert_eq!(sink.max_depth, 3);

        struct Validated<'a>(&'a str);
        impl super::super::JsonSerialize for Validated<'_> {
            fn json_serialize(&self, output: &mut String) {
                output.push_str(self.0);
            }

            fn json_serialize_to(
                &self,
                output: &mut dyn JsonWriteSink,
            ) -> Result<(), BoundedJsonError> {
                write_validated_json_to(self.0, output)
            }
        }

        assert_eq!(
            super::super::to_json_bounded(&Validated(value), value.len())
                .expect("write at exact bound"),
            value
        );
        assert_eq!(
            super::super::to_json_bounded(&Validated(value), value.len() - 1),
            Err(BoundedJsonError::BodyTooLarge)
        );
    }
}
