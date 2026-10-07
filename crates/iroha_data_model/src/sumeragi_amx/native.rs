//! Canonical native participant state and dedicated AMX monetary custody.

use super::{AmxError, AmxOutcomeV1, AmxParticipantStateV1};
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    account::AccountId,
    asset::{AssetBalanceScope, AssetId},
};
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Domain-separated commitment of the complete canonical native monetary leg.
/// # Errors
/// The canonical encoding cannot complete under the executing codec limits.
pub fn native_transfer_effects_hash(leg: &AmxTransferLegV1) -> Result<[u8; 32], AmxError> {
    // DM7 restores only the original encoded-frame scratch, with the same bytes and errors.
    #[cfg(all(test, sumeragi_model_mutation = "DM7"))]
    let result = norito::encode_canonical(leg).map(|bytes| {
        iroha_crypto::Hash::new_from_chunks(&[b"iroha:native-amx-transfer:v1", &[0], &bytes]).into()
    });
    #[cfg(not(all(test, sumeragi_model_mutation = "DM7")))]
    let result = streamed_native_transfer_effects_hash(leg);
    result.map_err(|error| {
        error
            .decode_resource_error()
            .map_or_else(|| AmxError::Proof(error.to_string()), AmxError::Resource)
    })
}

#[cfg(not(all(test, sumeragi_model_mutation = "DM7")))]
fn streamed_native_transfer_effects_hash(
    leg: &AmxTransferLegV1,
) -> Result<[u8; 32], norito::Error> {
    // This closed schema borrows AssetId's UUID, scope and account controller. Single compact
    // keys and MultisigPolicy member/key sequences stream borrowed fields under COMPACT_LEN;
    // Quantity borrows Numeric/BigInt digits and writes its bounded two's-complement stack array.
    // None of these concrete serializers needs an offset table or heap scratch. The canonical
    // writer's two frame passes and Hash writer have fixed stack state. This audit does not
    // extend to ordinary owning leg decode, its alignment copies, or witness publication.
    let mut codec = None;
    let hash = iroha_crypto::Hash::new_from_writer(|writer| {
        writer.write_all(b"iroha:native-amx-transfer:v1")?;
        writer.write_all(&[0])?;
        norito::core::write_canonical_to_writer(leg, writer).map_err(|cause| {
            codec = Some(cause);
            // Preserve the exact codec/refusal cause, without formatting or boxing a bridge.
            std::io::Error::from(std::io::ErrorKind::Other)
        })
    });
    if let Some(cause) = codec {
        return Err(cause);
    }
    Ok(hash.map_err(norito::Error::Io)?.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_model_base::{domain::DomainId, topology::DataSpaceId};
    #[test]
    fn native_monetary_leg_roundtrips_and_binds_every_effect() {
        let id = |seed| {
            AccountId::new(
                KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            )
        };
        let source = AssetId::with_scope(
            crate::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("native", "amx").unwrap(),
                "currency".parse().unwrap(),
            ),
            id(1),
            AssetBalanceScope::Dataspace(DataSpaceId::new(u64::MAX)),
        );
        let leg = AmxTransferLegV1 {
            source,
            destination: id(2),
            amount: 17_u32.into(),
        };
        assert_eq!(
            norito::decode_canonical::<AmxTransferLegV1>(&norito::encode_canonical(&leg).unwrap())
                .unwrap(),
            leg
        );
        assert_eq!(
            norito::json::from_str::<AmxTransferLegV1>(&norito::json::to_json(&leg).unwrap())
                .unwrap(),
            leg
        );
        let effects = native_transfer_effects_hash(&leg).unwrap();
        for changed in [
            AmxTransferLegV1 {
                amount: 18_u32.into(),
                ..leg.clone()
            },
            AmxTransferLegV1 {
                destination: id(3),
                ..leg.clone()
            },
        ] {
            assert_ne!(native_transfer_effects_hash(&changed).unwrap(), effects);
        }
        let record = AmxTransferEscrowV1 {
            tx: [4; 32],
            effects_hash: effects,
            leg,
            custody: id(5),
            settled: Some(AmxOutcomeV1::Abort),
        };
        assert_eq!(
            norito::decode_canonical::<AmxTransferEscrowV1>(
                &norito::encode_canonical(&record).unwrap()
            )
            .unwrap(),
            record
        );
        assert_eq!(
            norito::json::from_str::<AmxTransferEscrowV1>(&norito::json::to_json(&record).unwrap())
                .unwrap(),
            record
        );
    }
}

/// One exact same-dataspace numeric transfer, encoded canonically in an AMX leg payload.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, decode_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_amx::AmxTransferLegV1")]
pub struct AmxTransferLegV1 {
    /// Exact local account, asset definition and restricted balance partition to debit.
    pub source: AssetId,
    /// Registered receiving account in the same restricted balance partition.
    pub destination: AccountId,
    /// Positive quantity retained by this transfer.
    pub amount: Quantity,
}

/// Immutable native transfer custody retained even after its authenticated closure.
/// Generic escrow cancellation, local expiry and account removal cannot consume this record.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_amx::AmxTransferEscrowV1")]
pub struct AmxTransferEscrowV1 {
    /// Transaction identity binding the global deadline and all participant legs.
    pub tx: [u8; 32],
    /// Hash of the exact canonical local transfer effects.
    pub effects_hash: [u8; 32],
    /// Complete authorized local transfer retained at preparation.
    pub leg: AmxTransferLegV1,
    /// Deterministic protocol-only custody account for this transaction on this root.
    pub custody: AccountId,
    /// Global decision that consumed the original escrow, absent while locked.
    #[norito(required)]
    pub settled: Option<AmxOutcomeV1>,
}

/// Native participant authority and permanently protected original monetary records.
/// Runtime installation separately admits every allocation in the original State pool.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_amx::NativeAmxParticipantStateV1")]
pub struct NativeAmxParticipantStateV1 {
    /// Complete original canonical global signed genesis, including its result-only certificate.
    pub global_genesis: Vec<u8>,
    /// Complete canonical H2 carrier whose exact quorum authenticates the genesis result.
    pub global_successor: Vec<u8>,
    /// Canonical ASCII global chain label authenticated by the private signed genesis.
    pub global_chain_label: Vec<u8>,
    /// Component protocol state tracking the genesis-authenticated global instance.
    pub participant: AmxParticipantStateV1,
    /// Genesis-installed protocol-only account retaining local numeric escrow balances.
    pub custody: AccountId,
    /// Original Yes escrows in strict transaction order; closed custody remains protected.
    pub escrows: Vec<AmxTransferEscrowV1>,
}
impl NativeAmxParticipantStateV1 {
    /// Check local scope, canonical order and consistency with every unsettled Yes vote.
    /// # Errors
    /// Rejects any malformed participant or detached/mismatched monetary record.
    pub fn validate(&self) -> Result<(), AmxError> {
        self.participant.validate()?;
        if self.global_genesis.is_empty()
            || self.global_successor.is_empty()
            || std::str::from_utf8(&self.global_chain_label)
                .ok()
                .is_none_or(|label| iroha_primitives::chain_id::validate_chain_id(label).is_err())
        {
            return Err(AmxError::State(
                "native AMX lost its complete original global source",
            ));
        }
        let ds = self.participant.dataspace;
        if self.escrows.windows(2).any(|pair| pair[0].tx >= pair[1].tx) {
            return Err(AmxError::State(
                "native AMX escrows are not strictly ascending",
            ));
        }
        for record in &self.escrows {
            if record.leg.amount.is_zero()
                || record.leg.source.scope() != &AssetBalanceScope::Dataspace(ds)
                || record.leg.source.account() == &record.leg.destination
                || record.custody != self.custody
                || record.custody == *record.leg.source.account()
                || record.custody == record.leg.destination
            {
                return Err(AmxError::State(
                    "native AMX escrow has invalid local monetary scope",
                ));
            }
            if record.effects_hash != native_transfer_effects_hash(&record.leg)? {
                return Err(AmxError::State(
                    "native AMX escrow effects do not bind its complete monetary leg",
                ));
            }
            if record.settled.is_none()
                && !self.participant.entry(&record.tx).is_some_and(|entry| {
                    entry.vote == super::AmxVoteV1::Yes(record.effects_hash)
                        && entry.settled.is_none()
                })
            {
                return Err(AmxError::State(
                    "native AMX escrow lost its original unsettled Yes vote",
                ));
            }
        }
        for entry in &self.participant.prepared {
            if let super::AmxVoteV1::Yes(effects) = entry.vote {
                let record = self
                    .escrows
                    .binary_search_by_key(&entry.tx, |record| record.tx)
                    .ok()
                    .map(|index| &self.escrows[index])
                    .ok_or(AmxError::State(
                        "native AMX Yes vote lost its original escrow",
                    ))?;
                if record.effects_hash != effects || record.settled != entry.settled {
                    return Err(AmxError::State(
                        "native AMX escrow and prepared vote disagree",
                    ));
                }
            }
        }
        Ok(())
    }
}

mod leg_decode;
pub use leg_decode::{
    AllocatedAmxTransferLegV1, AmxLegDecodeErrorV1, PendingAmxTransferLegDecodeV1,
};
