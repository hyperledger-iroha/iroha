//! CLI output helpers for text/JSON normalization.

use eyre::Result;
use norito::json::JsonSerialize;

use crate::{CliOutputFormat, RunContext};

/// Emit JSON output or a text summary, depending on the CLI output format.
pub fn print_with_optional_text<C, T>(
    context: &mut C,
    text: Option<String>,
    json: &T,
) -> Result<()>
where
    C: RunContext,
    T: JsonSerialize + ?Sized,
{
    match context.output_format() {
        CliOutputFormat::Json => context.print_data(json),
        CliOutputFormat::Text => {
            if let Some(line) = text {
                context.println(line)
            } else {
                context.print_data(json)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_i18n::{Bundle, Language, Localizer};

    struct TestContext {
        output_format: CliOutputFormat,
        printed: Vec<String>,
        config: iroha::config::Config,
        i18n: Localizer,
    }

    impl TestContext {
        fn new(output_format: CliOutputFormat) -> Self {
            Self {
                output_format,
                printed: Vec::new(),
                config: crate::fallback_config(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &iroha::config::Config {
            &self.config
        }

        fn transaction_metadata(&self) -> Option<&iroha::data_model::metadata::Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            false
        }

        fn i18n(&self) -> &Localizer {
            &self.i18n
        }

        fn output_format(&self) -> CliOutputFormat {
            self.output_format
        }

        fn print_data<T>(&mut self, data: &T) -> eyre::Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            let rendered = norito::json::to_json_pretty(data)?;
            self.printed.push(rendered);
            Ok(())
        }

        fn println(&mut self, data: impl std::fmt::Display) -> eyre::Result<()> {
            self.printed.push(data.to_string());
            Ok(())
        }
    }

    #[test]
    fn print_with_optional_text_prefers_json_in_json_mode() {
        let mut ctx = TestContext::new(CliOutputFormat::Json);
        let payload = norito::json::json!({"ok": true});
        print_with_optional_text(&mut ctx, Some("hello".into()), &payload)
            .expect("emit");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"ok\":true"));
    }

    #[test]
    fn print_with_optional_text_uses_text_in_text_mode() {
        let mut ctx = TestContext::new(CliOutputFormat::Text);
        let payload = norito::json::json!({"ok": true});
        print_with_optional_text(&mut ctx, Some("hello".into()), &payload)
            .expect("emit");
        assert_eq!(ctx.printed, vec!["hello"]);
    }

    #[test]
    fn print_with_optional_text_falls_back_to_json_when_missing_text() {
        let mut ctx = TestContext::new(CliOutputFormat::Text);
        let payload = norito::json::json!({"ok": true});
        print_with_optional_text(&mut ctx, None, &payload).expect("emit");
        assert_eq!(ctx.printed.len(), 1);
        assert!(ctx.printed[0].contains("\"ok\":true"));
    }
}
