//! Original-pool readers used before governed parameter publication.
//!
//! Original decoder/refusal ownership is classified by the calling attempt.
//! TODO(S6): enclosing instruction/source and diagnostic storage retain their
//! separate owners; this leaf does not retroactively fund those graphs.

use crate::pipeline_gas_policy::ParameterIngressError;
use iroha_allocation::{AllocationBudget, ChargedBuffer};
use iroha_data_model::parameter::system::{
    AdmittedConsensusHandshakeMetadata, AdmittedSumeragiNposParameters,
};
use iroha_primitives::json::Json;
use norito::json;

struct KeyBuffer(ChargedBuffer<u8>);
impl AsMut<[u8]> for KeyBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }
}
impl AsRef<str> for KeyBuffer {
    fn as_ref(&self) -> &str {
        std::str::from_utf8(self.0.as_slice()).expect("shared JSON key kernel validates UTF-8")
    }
}

// Preserve the former Value preflight's four exact field diagnostics without a
// Value tree, an ordinary String key, or a serialization pass. The production
// caller supplies canonical Json, which has already rejected duplicate keys and
// nonrepresentable numbers; this helper is not a semantic validator for arbitrary
// unchecked text. Syntax is checked
// for the whole source first. The typed destination still rejects every unknown,
// duplicate, absent or mistyped field before a successor can be published.
fn npos_zero_field(
    source: &str,
    pool: &AllocationBudget,
) -> Result<Option<&'static str>, ParameterIngressError> {
    const FIELDS: [&str; 4] = [
        "finality_margin_blocks",
        "evidence_horizon_blocks",
        "activation_lag_blocks",
        "slashing_delay_blocks",
    ];
    let mut zeroes = [false; 4];
    let mut parser = json::Parser::new(source);
    parser.preflight_document()?;
    parser.skip_ws();
    let mut object = json::MapVisitor::new(&mut parser)?;
    while let Some(key) = object.next_key_with_buffer(|length| {
        let mut bytes = ChargedBuffer::new(length, pool)?;
        for _ in 0..length {
            bytes.push_reserved(0);
        }
        Ok::<_, ParameterIngressError>(KeyBuffer(bytes))
    })? {
        let name = match &key {
            json::KeyRef::Borrowed(name) => *name,
            json::KeyRef::Owned(name) => name.as_ref(),
        };
        let raw = object.parse_value_with_parser(json::Parser::raw_value_slice)?;
        if let Some(index) = FIELDS.iter().position(|field| *field == name) {
            // Value::as_u64 accepts signed integer -0 but not floating zeroes.
            zeroes[index] = matches!(raw, "0" | "-0");
        }
    }
    drop(object);
    parser.finish_document()?;
    Ok(FIELDS
        .into_iter()
        .zip(zeroes)
        .find_map(|(field, zero)| zero.then_some(field)))
}

/// Decode exact original `NPoS` bytes with funded record, keys, quantities and text.
pub(super) fn npos(
    source: &Json,
    pool: &AllocationBudget,
) -> Result<AdmittedSumeragiNposParameters, ParameterIngressError> {
    if let Some(field) = npos_zero_field(source.get(), pool)? {
        return Err(ParameterIngressError::NposZeroField(field));
    }
    let mut parser = json::Parser::new(source.get());
    parser.preflight_document()?;
    parser.skip_ws();
    let value = AdmittedSumeragiNposParameters::try_decode_json(&mut parser, pool)?;
    parser.finish_document()?;
    Ok(value)
}

/// Decode and validate the signed scalar metadata with original funded scratch.
pub(super) fn handshake(
    source: &Json,
    pool: &AllocationBudget,
) -> Result<AdmittedConsensusHandshakeMetadata, ParameterIngressError> {
    let mut parser = json::Parser::new(source.get());
    parser.preflight_document()?;
    parser.skip_ws();
    let value = AdmittedConsensusHandshakeMetadata::try_decode_json(&mut parser, pool)?;
    parser.finish_document()?;
    value.get().validate()?;
    Ok(value)
}

/// Original instruction phase used only to decorate a completed rejection.
#[derive(Clone, Copy)]
pub(super) enum PolicyInput {
    /// Signed handshake payload and its separate intrinsic validation.
    Handshake,
    /// Incoming signed `NPoS` bytes and the four explicit zero-bound diagnostics.
    IncomingNpos,
    /// Previously installed signed `NPoS` bytes, whose original error was unprefixed.
    InstalledNpos,
}

/// Retain original refusal ownership before restoring instruction diagnostic context.
pub(super) fn instruction_error(
    state: &mut crate::state::StateTransaction<'_, '_>,
    original: crate::pipeline_gas_policy::ParameterIngressAttemptError,
    input: PolicyInput,
) -> iroha_data_model::isi::error::InstructionExecutionError {
    use iroha_data_model::isi::error::{InstructionExecutionError, InvalidParameterError};

    let attempt = crate::state::parameter_ingress_attempt_error(original, |original| {
        // This closure is called only for a completed rejection. Local decoder,
        // finite-pool and native allocator refusals retain their original owners
        // and never enter string decoration.
        let message = match (input, original) {
            (PolicyInput::Handshake, ParameterIngressError::Handshake(error)) => {
                format!("invalid signed consensus handshake metadata: {error}")
            }
            (PolicyInput::Handshake, error) => {
                format!("invalid signed consensus handshake metadata payload: {error}")
            }
            (PolicyInput::IncomingNpos, error @ ParameterIngressError::NposZeroField(_)) => {
                error.to_string()
            }
            // The former Value/from_value and installed custom-parameter paths
            // exposed json::Error directly, without the core Error::Json wrapper.
            (
                PolicyInput::IncomingNpos,
                ParameterIngressError::Codec(norito::Error::Json(error)),
            ) => {
                format!("invalid signed NPoS parameter payload: {error}")
            }
            (PolicyInput::IncomingNpos, error) => {
                format!("invalid signed NPoS parameter payload: {error}")
            }
            (
                PolicyInput::InstalledNpos,
                ParameterIngressError::Codec(norito::Error::Json(error)),
            ) => error.to_string(),
            (PolicyInput::InstalledNpos, error) => error.to_string(),
        };
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message))
    });
    state.attempt_error_to_instruction_error(attempt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_gas_policy::ParameterIngressAttemptError;
    use iroha_data_model::parameter::system::SumeragiNposParameters;

    #[test]
    fn npos_preflight_preserves_exact_zero_order_and_complete_syntax() {
        let pool = AllocationBudget::new(1 << 20);
        for field in [
            "finality_margin_blocks",
            "evidence_horizon_blocks",
            "activation_lag_blocks",
            "slashing_delay_blocks",
        ] {
            let source = format!("{{\"{field}\":0}}");
            assert_eq!(npos_zero_field(&source, &pool).unwrap(), Some(field));
        }
        assert_eq!(
            npos_zero_field(
                r#"{"slashing_delay_blocks":0,"finality_margin_blocks":-0}"#,
                &pool
            )
            .unwrap(),
            Some("finality_margin_blocks")
        );
        assert_eq!(
            npos_zero_field(r#"{"finality_margin_blocks":0.0}"#, &pool).unwrap(),
            None
        );
        assert!(npos_zero_field(r#"{"finality_margin_blocks":0,"broken":[}"#, &pool).is_err());
        assert_eq!(pool.reserved_bytes(), 0);
    }

    #[test]
    fn npos_original_key_and_record_refusal_refund_and_retry() {
        let pool = AllocationBudget::new(1 << 20);
        let source = SumeragiNposParameters::default().into_custom_parameter();
        let escaped = source.payload().get().replacen(
            "finality_margin_blocks",
            "finality_\\u006dargin_blocks",
            1,
        );
        let cap = pool.limit_bytes();
        pool.set_limit_bytes(0);
        let error =
            ParameterIngressAttemptError::observe(|| npos_zero_field(&escaped, &pool)).unwrap_err();
        assert!(matches!(
            error,
            ParameterIngressAttemptError::Original(ParameterIngressError::Allocation(_))
        ));
        assert_eq!(pool.reserved_bytes(), 0);
        let error =
            ParameterIngressAttemptError::observe(|| npos(source.payload(), &pool)).unwrap_err();
        assert!(matches!(
            error,
            ParameterIngressAttemptError::Original(ParameterIngressError::Allocation(_))
        ));
        assert_eq!(pool.reserved_bytes(), 0);
        pool.set_limit_bytes(cap);
        let owner =
            ParameterIngressAttemptError::observe(|| npos(source.payload(), &pool)).unwrap();
        assert_eq!(owner.get(), &SumeragiNposParameters::default());
        assert!(owner.belongs_to(&pool));
        drop(owner);
        assert_eq!(pool.reserved_bytes(), 0);
    }

    #[test]
    fn npos_payload_canonical_boundary_rejects_duplicate_zero_spellings() {
        for source in [
            r#"{"finality_margin_blocks":1,"finality_margin_blocks":0}"#,
            r#"{"finality_margin_blocks":0,"finality_margin_blocks":1}"#,
            r#"{"finality_margin_blocks":1,"finality_\u006dargin_blocks":0}"#,
        ] {
            assert!(norito::json::from_str::<norito::json::Value>(source).is_err());
            assert!(Json::from_str_norito(source).is_err());
        }
    }
}
