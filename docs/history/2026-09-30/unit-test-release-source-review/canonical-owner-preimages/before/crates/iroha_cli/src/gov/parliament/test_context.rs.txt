//! Capturing [`RunContext`] for Parliament command tests.
//!
//! Commands run against it exactly as against the real context, except that
//! submitted transactions and printed output are recorded instead of being
//! sent or written, so a test can assert the exact instructions a command
//! would sign.

use eyre::{Result, bail};
use iroha::{
    config::Config,
    data_model::{account::AccountId, isi::InstructionBox, transaction::Executable},
};
use iroha_crypto::KeyPair;
use iroha_data_model::{NetworkId, isi::governance::SubmitParliamentLifecycleTransitionV1};
use iroha_i18n::{Bundle, Language, Localizer};
use iroha_model_base::metadata::Metadata;
use norito::json::JsonSerialize;

use crate::{CliOutputFormat, RunContext};

/// Run context that records submissions and output.
pub(super) struct CaptureContext {
    config: Config,
    format: CliOutputFormat,
    i18n: Localizer,
    /// JSON documents printed with `print_data`, in order.
    pub printed: Vec<norito::json::Value>,
    /// Text lines printed with `println`, in order.
    pub lines: Vec<String>,
    /// Instruction lists of every submitted transaction, in order.
    pub submitted: Vec<Vec<InstructionBox>>,
}

impl CaptureContext {
    /// JSON-output context signing with `key_pair` (as its account) on `network_id`.
    pub(super) fn new(key_pair: KeyPair, network_id: NetworkId) -> Self {
        let mut config = crate::fallback_config();
        config.account = AccountId::new(key_pair.public_key().clone());
        config.key_pair = key_pair;
        config.network_id = network_id;
        Self {
            config,
            format: CliOutputFormat::Json,
            i18n: Localizer::new(Bundle::Cli, Language::English),
            printed: Vec::new(),
            lines: Vec::new(),
            submitted: Vec::new(),
        }
    }

    /// The same context with text output.
    pub(super) fn text(mut self) -> Self {
        self.format = CliOutputFormat::Text;
        self
    }

    /// Point the configured Torii at `url`.
    pub(super) fn with_torii(mut self, url: url::Url) -> Self {
        self.config.torii_api_url = url;
        self
    }

    /// The only transaction submitted so far, as Parliament transitions.
    pub(super) fn single_transition(&self) -> &SubmitParliamentLifecycleTransitionV1 {
        let [instructions] = self.submitted.as_slice() else {
            panic!(
                "expected exactly one submitted transaction, got {}",
                self.submitted.len()
            )
        };
        let [instruction] = instructions.as_slice() else {
            panic!(
                "expected exactly one instruction, got {}",
                instructions.len()
            )
        };
        instruction
            .as_any()
            .downcast_ref::<SubmitParliamentLifecycleTransitionV1>()
            .expect("Parliament lifecycle transition")
    }
}

impl RunContext for CaptureContext {
    fn config(&self) -> &Config {
        &self.config
    }

    fn transaction_metadata(&self) -> Option<&Metadata> {
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
        self.format
    }

    fn print_data<T>(&mut self, data: &T) -> Result<()>
    where
        T: JsonSerialize + ?Sized,
    {
        self.printed.push(norito::json::to_value(data)?);
        Ok(())
    }

    fn println(&mut self, data: impl std::fmt::Display) -> Result<()> {
        self.lines.push(data.to_string());
        Ok(())
    }

    fn submit_with_metadata(
        &mut self,
        instructions: impl Into<Executable>,
        _metadata: Metadata,
        _wait_for_confirmation: bool,
    ) -> Result<()> {
        match instructions.into() {
            Executable::Instructions(instructions) => {
                self.submitted.push(instructions.into_vec());
                Ok(())
            }
            _ => bail!("Parliament commands submit plain instructions only"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::data_model::governance::types::GovernanceAttemptId;
    use iroha_crypto::Algorithm;
    use iroha_data_model::isi::governance::ParliamentLifecycleTransitionV1;

    #[test]
    fn capture_context_records_submissions_and_output() {
        let key_pair =
            KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).expect("account key");
        let account = AccountId::new(key_pair.public_key().clone());
        let network_id = crate::fallback_config().network_id;
        let mut context = CaptureContext::new(key_pair.clone(), network_id).text();
        assert_eq!(context.config().account, account);
        assert_eq!(context.config().key_pair, key_pair);
        assert!(matches!(context.output_format(), CliOutputFormat::Text));
        context.println("line").expect("println");
        context
            .print_data(&norito::json!({ "key": 1 }))
            .expect("print");
        let instruction = InstructionBox::from(SubmitParliamentLifecycleTransitionV1 {
            governance_attempt_id: GovernanceAttemptId::new([1; 32]),
            transition: ParliamentLifecycleTransitionV1::CompleteQualification,
        });
        context.finish(vec![instruction]).expect("finish");
        assert_eq!(context.lines, vec!["line".to_owned()]);
        assert_eq!(context.printed.len(), 1);
        assert_eq!(
            context.single_transition().governance_attempt_id,
            GovernanceAttemptId::new([1; 32])
        );
        let url = url::Url::parse("http://127.0.0.1:9/").expect("url");
        assert_eq!(context.with_torii(url.clone()).config.torii_api_url, url);
    }
}
