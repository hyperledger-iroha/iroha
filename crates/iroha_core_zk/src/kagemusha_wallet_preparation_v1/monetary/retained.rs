//! Exact private monetary restart sources retained before software Advance.
//! Original controls are reauthenticated by their source state; no public clock or verdict API.
use super::*;
use crate::kagemusha_wallet_state_v1::{
    self as custody, BlacklistOriginalReferenceV1, FrozenTransition, ObjectStore,
    publish_blacklist_original, read_blacklist_original,
};
type SourceError = custody::Error;
use SourceError as E;
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize};

const MAX_BYTES: usize = 64 * 1024;
#[derive(NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.wallet.native_monetary_sources.v1")]
struct Payload {
    version: u16,
    predecessor_capsule_digest: [u8; 32],
    manifest_digest: [u8; 32],
    statement_digest: [u8; 32],
    enrollment: Vec<u8>,
    send: Option<Send>,
    receive: Option<Receive>,
}
#[derive(NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.wallet.native_send_sources.v1")]
struct Send {
    observed: Option<([u8; 32], u64)>,
    anchored_original: Option<Vec<u8>>,
    blacklist: Option<Vec<u8>>,
    quota_share_original: Option<Vec<u8>>,
    quota_usage_original: Vec<u8>,
}
#[derive(NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.wallet.native_receive_sources.v1")]
struct Receive {
    recorded_history: Option<Vec<u8>>,
    recorded_gap: Option<Vec<u8>>,
    fold_history: Vec<u8>,
}

/// Opaque DATA produced by Native's source owner; the capsule digest binds these exact bytes.
/// Construction is crate-private and consumes the actual authenticated preparation inputs.
pub(crate) struct MonetaryRetentionV1 {
    payload: Payload,
    original: Vec<u8>,
}
impl MonetaryRetentionV1 {
    pub(super) fn bind(&self, prepared: &MonetaryStepV1) -> Result<(), super::super::Error> {
        if self.payload.manifest_digest != prepared.manifest_digest
            || self.payload.predecessor_capsule_digest != prepared.source_capsule_digest
            || self.payload.statement_digest != authority(prepared.statement.statement_digest())?
            || !matches!(
                (
                    prepared.statement.effect.kind(),
                    &self.payload.send,
                    &self.payload.receive
                ),
                (KagemushaWalletOperationKindV1::Send, Some(_), None)
                    | (KagemushaWalletOperationKindV1::Receive, None, Some(_))
            )
        {
            return Err(super::super::Error::Authority);
        }
        Ok(())
    }
    /// Exact MonetaryWitness original, ready for the native capsule builder before durable Advance.
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    pub(crate) fn send(
        owner: &AuthenticatedCredentialV1,
        previous: &FoldedStateV1,
        prepared: &MonetaryStepV1,
        controls: &SendControlsV1<'_>,
        blacklist_original: Option<&[u8]>,
        objects: &mut dyn ObjectStore,
    ) -> Result<Self, E> {
        let quota_usage_original = norito::encode_canonical(&controls.quota_usage.slots().to_vec())
            .map_err(|_| E::Invalid("quota64 original"))?;
        if controls.quota_usage.root() != previous.source_state().core.quota_usage_root {
            return Err(E::Invalid("source quota64 root"));
        }
        let checked = prepared
            .request
            .check_send(&KagemushaWalletSendInputsV1 {
                payer_credential: owner.credential(),
                payer_state: previous.source_state(),
                omega: &previous.lineage().public,
                anchored: controls.anchored,
                now: controls.now,
                blacklist: controls.blacklist,
                quota_share: controls.quota_share,
                quota_usage: controls.quota_usage,
            })
            .map_err(|_| E::Invalid("Send actual retained controls"))?;
        if prepared.send_check.as_ref() != Some(&checked)
            || prepared.manifest_digest != owner.manifest_digest
            || prepared.source_capsule_digest != previous.source_capsule_digest()
            || previous.manifest_digest != prepared.manifest_digest
            || previous.credential != owner.credential
            || prepared.statement.effect.kind() != KagemushaWalletOperationKindV1::Send
        {
            return Err(E::Invalid("Send exact prepared source"));
        }
        let blacklist = match (controls.blacklist, blacklist_original) {
            (None, None) => None,
            (Some(value), Some(original)) => {
                let exact =
                    KagemushaWalletBlacklistV1::decode_canonical(original, &value.body.scheme_id)
                        .map_err(|_| E::Invalid("full blacklist original"))?;
                if &exact != value {
                    return Err(E::Invalid("full blacklist original equality"));
                }
                Some(
                    publish_blacklist_original(objects, &value.body.scheme_id, original)?
                        .to_canonical_bytes()?,
                )
            }
            _ => return Err(E::Invalid("full blacklist original ownership")),
        };
        let anchored_original = controls
            .anchored
            .map(norito::encode_canonical)
            .transpose()
            .map_err(|_| E::Invalid("retained native anchor"))?;
        let quota_share_original = controls
            .quota_share
            .map(|value| value.to_canonical_bytes())
            .transpose()
            .map_err(|_| E::Invalid("retained quota share"))?;
        Self::encode(Payload {
            version: 1,
            predecessor_capsule_digest: previous.source_capsule_digest(),
            manifest_digest: prepared.manifest_digest,
            statement_digest: prepared
                .statement
                .statement_digest()
                .map_err(|_| E::Invalid("retained statement"))?,
            enrollment: owner.originals().1.to_vec(),
            send: Some(Send {
                observed: controls.now.map(|v| (v.boot_id, v.monotonic_ms)),
                anchored_original,
                blacklist,
                quota_share_original,
                quota_usage_original,
            }),
            receive: None,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn receive(
        owner: &AuthenticatedCredentialV1,
        preparation: &PreparationV1<'_>,
        previous: &ReleasedStep,
        prepared: &MonetaryStepV1,
        recorded: Option<&KagemushaWalletRecordedBlacklistProofV1>,
        history_leaf: KagemushaWalletIndexedLeafV1,
        history_opening: KagemushaWalletIndexedOpeningV1,
        budget: MemoryBudget,
    ) -> Result<Self, E> {
        preparation
            .receipt_tape(owner, previous, budget)
            .map_err(|_| E::Invalid("Receive actual source proof"))?;
        if prepared.statement.effect.kind() != KagemushaWalletOperationKindV1::Receive
            || prepared.source_capsule_digest
                != previous
                    .frozen
                    .capsule
                    .capsule_digest()
                    .map_err(|_| E::Invalid("Receive source capsule"))?
            || history_opening
                .leaf_root(&history_leaf)
                .map_err(|_| E::Invalid("Receive history source path"))?
                != previous
                    .frozen
                    .capsule
                    .successor_state
                    .rest
                    .blacklist_history_root
        {
            return Err(E::Invalid("Receive exact current history source"));
        }
        verify_history(
            &previous
                .frozen
                .capsule
                .successor_state
                .rest
                .blacklist_history_root,
            &prepared.request.body,
            &history_leaf,
            &history_opening,
        )?;
        let rerun = preparation
            .prepare_receive(
                owner,
                previous,
                &prepared.request_original,
                prepared
                    .payment_original
                    .as_deref()
                    .ok_or(E::Invalid("Receive Payment"))?,
                &prepared
                    .payer
                    .as_ref()
                    .ok_or(E::Invalid("Receive payer"))?
                    .credential_original,
                prepared
                    .payer_certificate_set_original
                    .as_deref()
                    .ok_or(E::Invalid("Receive certificates"))?,
                recorded,
                ReceiveMapsV1 {
                    consumed: model_insertion(
                        prepared.maps.first().ok_or(E::Invalid("Receive map"))?,
                    ),
                },
                prepared.state.core.state_nonce,
                budget,
            )
            .map_err(|_| E::Invalid("Receive hard source reconstruction"))?;
        if rerun.state != prepared.state || rerun.statement != prepared.statement {
            return Err(E::Invalid("Receive retained source parity"));
        }
        Self::encode(Payload {
            version: 1,
            predecessor_capsule_digest: prepared.source_capsule_digest,
            manifest_digest: prepared.manifest_digest,
            statement_digest: prepared
                .statement
                .statement_digest()
                .map_err(|_| E::Invalid("retained statement"))?,
            enrollment: owner.originals().1.to_vec(),
            send: None,
            receive: Some(Receive {
                recorded_history: recorded
                    .map(|v| v.history_opening.leaf_transcript(&v.history_leaf)),
                recorded_gap: recorded.map(|v| v.gap.transcript()),
                fold_history: history_opening.leaf_transcript(&history_leaf),
            }),
        })
    }
    fn encode(payload: Payload) -> Result<Self, E> {
        let original = norito::encode_canonical(&payload)
            .map_err(|_| E::Invalid("native retained sources"))?;
        if original.is_empty() || original.len() > MAX_BYTES {
            return Err(E::Invalid("native retained sources bound"));
        }
        Ok(Self { payload, original })
    }
}

pub(crate) struct ReceiveHistory {
    pub history_leaf: KagemushaWalletIndexedLeafV1,
    pub history_opening: KagemushaWalletIndexedOpeningV1,
}
pub(crate) struct MonetaryRestoreV1 {
    pub(crate) prepared: MonetaryStepV1,
    pub(crate) fold_history: Option<ReceiveHistory>,
}
fn payload(frozen: &FrozenTransition) -> Result<Payload, E> {
    let original = exact_retained(
        &frozen.capsule,
        KagemushaWalletRetainedInputRoleV1::MonetaryWitness,
    )?;
    if original.is_empty() || original.len() > MAX_BYTES {
        return Err(E::WitnessLost("native monetary source bound"));
    }
    let payload: Payload =
        norito::decode_canonical_with_limits(original, norito::canonical_decode_limits(MAX_BYTES))
            .map_err(|_| E::WitnessLost("native monetary source canonical original"))?;
    if payload.version != 1
        || !matches!(
            (frozen.capsule.kind, &payload.send, &payload.receive),
            (KagemushaWalletOperationKindV1::Send, Some(_), None)
                | (KagemushaWalletOperationKindV1::Receive, None, Some(_))
        )
        || payload.statement_digest
            != frozen
                .capsule
                .statement
                .statement_digest()
                .map_err(|_| E::WitnessLost("monetary original statement"))?
        || payload.predecessor_capsule_digest != frozen.capsule.predecessor_capsule_digest
    {
        return Err(E::WitnessLost("native monetary predecessor source"));
    }
    Ok(payload)
}
pub(crate) fn enrollment(frozen: &FrozenTransition) -> Result<Vec<u8>, E> {
    Ok(payload(frozen)?.enrollment)
}
fn opening(
    original: &[u8],
) -> Result<
    (
        KagemushaWalletIndexedLeafV1,
        KagemushaWalletIndexedOpeningV1,
    ),
    E,
> {
    let (leaf, path) = KagemushaWalletIndexedOpeningV1::from_transcript(original)
        .map_err(|_| E::WitnessLost("retained indexed route"))?;
    Ok((leaf.ok_or(E::WitnessLost("retained indexed leaf"))?, path))
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn restore(
    preparation: &PreparationV1<'_>,
    owner: &AuthenticatedCredentialV1,
    previous: Option<&FoldedStateV1>,
    previous_source: &ReleasedStep,
    frozen: &FrozenTransition,
    objects: &mut dyn ObjectStore,
    budget: MemoryBudget,
) -> Result<MonetaryRestoreV1, E> {
    let payload = payload(frozen)?;
    if payload.enrollment != owner.originals().1 {
        return Err(E::WitnessLost("retained own Enrollment original"));
    }
    let c = &frozen.capsule;
    if payload.manifest_digest != preparation.installed.verifier().manifest_digest()
        || c.predecessor_capsule_digest
            != previous_source
                .frozen
                .capsule
                .capsule_digest()
                .map_err(|_| E::WitnessLost("actual monetary predecessor"))?
    {
        return Err(E::WitnessLost("actual monetary installation/source"));
    }
    let nonce = c.successor_state.core.state_nonce;
    let request = exact_retained(c, KagemushaWalletRetainedInputRoleV1::Request)?;
    let result = match (c.kind, payload.send, payload.receive) {
        (KagemushaWalletOperationKindV1::Send, Some(source), None) => {
            let previous = previous.ok_or(E::FoldRequired)?;
            if previous.source_capsule_digest() != c.predecessor_capsule_digest {
                return Err(E::WitnessLost("Send actual predecessor fold"));
            }
            let anchored = source
                .anchored_original
                .as_ref()
                .map(|original| {
                    norito::decode_canonical_with_limits::<KagemushaWalletAnchoredTimeV1>(
                        original,
                        norito::canonical_decode_limits(original.len()),
                    )
                    .map_err(|_| E::WitnessLost("retained anchor original"))
                })
                .transpose()?;
            let blacklist = source
                .blacklist
                .as_ref()
                .map(|reference| {
                    let reference =
                        BlacklistOriginalReferenceV1::decode_canonical(reference, &c.scheme_id)?;
                    let original = read_blacklist_original(objects, &reference, &c.scheme_id)?;
                    KagemushaWalletBlacklistV1::decode_canonical(&original, &c.scheme_id)
                        .map_err(|_| E::WitnessLost("full blacklist canonical original"))
                })
                .transpose()?;
            let quota = source
                .quota_share_original
                .as_ref()
                .map(|original| {
                    KagemushaWalletQuotaShareV1::decode_canonical(original, &c.scheme_id)
                        .map_err(|_| E::WitnessLost("retained quota share original"))
                })
                .transpose()?;
            let slots: Vec<Option<KagemushaWalletQuotaUsageLeafV1>> =
                norito::decode_canonical_with_limits(
                    &source.quota_usage_original,
                    norito::canonical_decode_limits(MAX_BYTES),
                )
                .map_err(|_| E::WitnessLost("source quota64 canonical slots"))?;
            let slots = slots
                .try_into()
                .map_err(|_| E::WitnessLost("source full64 quota slots"))?;
            let usage = KagemushaWalletQuotaUsageArrayV1::from_slots(slots)
                .map_err(|_| E::WitnessLost("source quota64 slots"))?;
            if usage.root() != previous.source_state().core.quota_usage_root {
                return Err(E::WitnessLost("source quota64 root"));
            }
            let now =
                source.observed.map(
                    |(boot_id, monotonic_ms)| KagemushaWalletMonotonicReadingV1 {
                        boot_id,
                        monotonic_ms,
                    },
                );
            let maps = &c.map_openings;
            let request_body: KagemushaWalletRequestV1 = norito::decode_canonical_with_limits(
                request,
                norito::canonical_decode_limits(request.len()),
            )
            .map_err(|_| E::WitnessLost("retained Request original"))?;
            let fee = request_body.body.fee != 0;
            if maps.len() != if fee { 4 } else { 2 } {
                return Err(E::WitnessLost("Send original map count"));
            }
            let pending = decode_indexed_insertion_v1(&maps[..2])
                .map_err(|_| E::WitnessLost("Send pending originals"))?;
            let fee = if fee {
                Some(
                    decode_indexed_insertion_v1(&maps[2..])
                        .map_err(|_| E::WitnessLost("Send fee originals"))?,
                )
            } else {
                None
            };
            MonetaryRestoreV1 {
                prepared: preparation
                    .prepare_send(
                        owner,
                        previous,
                        request,
                        SendControlsV1 {
                            anchored: anchored.as_ref(),
                            now: now.as_ref(),
                            blacklist: blacklist.as_ref(),
                            quota_share: quota.as_ref(),
                            quota_usage: &usage,
                        },
                        SendMapsV1 { pending, fee },
                        nonce,
                    )
                    .map_err(|_| E::Invalid("native Send original reconstruction"))?,
                fold_history: None,
            }
        }
        (KagemushaWalletOperationKindV1::Receive, None, Some(source)) => {
            let recorded = match (source.recorded_history, source.recorded_gap) {
                (None, None) => None,
                (Some(history), Some(gap)) => {
                    let (history_leaf, history_opening) = opening(&history)?;
                    Some(KagemushaWalletRecordedBlacklistProofV1 {
                        history_leaf,
                        history_opening,
                        gap: KagemushaWalletBlacklistGapOpeningV1::from_transcript(&gap)
                            .map_err(|_| E::WitnessLost("recorded blacklist gap"))?,
                    })
                }
                _ => return Err(E::WitnessLost("recorded blacklist originals")),
            };
            let (history_leaf, history_opening) = opening(&source.fold_history)?;
            if history_opening
                .leaf_root(&history_leaf)
                .map_err(|_| E::WitnessLost("Receive history source"))?
                != previous_source
                    .frozen
                    .capsule
                    .successor_state
                    .rest
                    .blacklist_history_root
            {
                return Err(E::WitnessLost("Receive actual history root"));
            }
            let request_value = super::request(request, preparation.installed.verifier().scheme())
                .map_err(|_| E::WitnessLost("Receive original Request"))?;
            verify_history(
                &previous_source
                    .frozen
                    .capsule
                    .successor_state
                    .rest
                    .blacklist_history_root,
                &request_value.body,
                &history_leaf,
                &history_opening,
            )?;
            let consumed = decode_indexed_insertion_v1(&c.map_openings)
                .map_err(|_| E::WitnessLost("Receive consumed originals"))?;
            MonetaryRestoreV1 {
                prepared: preparation
                    .prepare_receive(
                        owner,
                        previous_source,
                        request,
                        exact_retained(c, KagemushaWalletRetainedInputRoleV1::Payment)?,
                        exact_retained(c, KagemushaWalletRetainedInputRoleV1::Credential)?,
                        exact_retained(c, KagemushaWalletRetainedInputRoleV1::CertificateSet)?,
                        recorded.as_ref(),
                        ReceiveMapsV1 { consumed },
                        nonce,
                        budget,
                    )
                    .map_err(|_| E::Invalid("native Receive original reconstruction"))?,
                fold_history: Some(ReceiveHistory {
                    history_leaf,
                    history_opening,
                }),
            }
        }
        _ => return Err(E::WitnessLost("native monetary kind/source union")),
    };
    if result.prepared.state() != &c.successor_state
        || result.prepared.statement() != &c.statement
        || result.prepared.source_capsule_digest() != c.predecessor_capsule_digest
    {
        return Err(E::Invalid("native monetary frozen parity"));
    }
    Ok(result)
}

fn exact_retained(
    c: &KagemushaWalletRecoveryCapsuleV1,
    role: KagemushaWalletRetainedInputRoleV1,
) -> Result<&[u8], E> {
    retained_original(&c.retained_inputs, role)
        .map_err(|_| E::WitnessLost("unique monetary retained original"))
}
fn decode_indexed_insertion_v1(
    originals: &[Vec<u8>],
) -> Result<KagemushaWalletIndexedInsertV1, super::super::Error> {
    let [low, empty] = originals else {
        return Err(super::super::Error::Authority);
    };
    let (low, low_opening) = authority(KagemushaWalletIndexedOpeningV1::from_transcript(low))?;
    let (slot, slot_opening) = authority(KagemushaWalletIndexedOpeningV1::from_transcript(empty))?;
    if slot.is_some() {
        return Err(super::super::Error::Authority);
    }
    Ok(KagemushaWalletIndexedInsertV1 {
        low: low.ok_or(super::super::Error::Authority)?,
        low_opening,
        slot_opening,
    })
}
fn model_insertion(native: &IndexedInsert<Fp>) -> KagemushaWalletIndexedInsertV1 {
    KagemushaWalletIndexedInsertV1 {
        low: KagemushaWalletIndexedLeafV1 {
            key: native.leaf.key.to_repr(),
            value: native.leaf.value.to_repr(),
            next_key: native.leaf.next_key.to_repr(),
        },
        low_opening: KagemushaWalletIndexedOpeningV1 {
            slot: native.leaf_slot,
            siblings: native.leaf_siblings.map(|v| v.to_repr()),
        },
        slot_opening: KagemushaWalletIndexedOpeningV1 {
            slot: native.slot,
            siblings: native.slot_siblings.map(|v| v.to_repr()),
        },
    }
}

impl PreparationV1<'_> {
    /// Restore the exact signed capsule's monetary sources through actual coordinator custody.
    pub(crate) fn restore_monetary(
        &self,
        owner: &AuthenticatedCredentialV1,
        previous: Option<&FoldedStateV1>,
        source: &ReleasedStep,
        frozen: &FrozenTransition,
        objects: &mut dyn ObjectStore,
        budget: MemoryBudget,
    ) -> Result<MonetaryRestoreV1, E> {
        restore(self, owner, previous, source, frozen, objects, budget)
    }
    /// Read the exact Enrollment certificate retained by the monetary source owner.
    pub(crate) fn monetary_enrollment(&self, frozen: &FrozenTransition) -> Result<Vec<u8>, E> {
        enrollment(frozen)
    }
}

#[cfg(test)]
#[path = "retained/tests.rs"]
mod tests;

fn verify_history(
    root: &[u8; 32],
    request: &KagemushaWalletRequestBodyV1,
    leaf: &KagemushaWalletIndexedLeafV1,
    opening: &KagemushaWalletIndexedOpeningV1,
) -> Result<(), E> {
    let version = if request.receiver_blacklist_version == 0 {
        1
    } else {
        request.receiver_blacklist_version
    };
    let key = Fp::from(version).to_repr();
    if leaf.key == key {
        kagemusha_wallet_indexed_verify_membership_v1(root, leaf, opening)
            .map_err(|_| E::WitnessLost("Receive exact history membership route"))?;
        if request.receiver_blacklist_version != 0 {
            KagemushaWalletBlacklistHistoryLeafV1 {
                list_version: version,
                entries_root: request.receiver_blacklist_root,
            }
            .verify_membership(root, leaf, opening)
            .map_err(|_| E::WitnessLost("Receive recorded history pair"))?;
        }
    } else {
        kagemusha_wallet_indexed_verify_non_membership_v1(root, &key, leaf, opening)
            .map_err(|_| E::WitnessLost("Receive exact history absence route"))?;
        if request.receiver_blacklist_version != 0 {
            return Err(E::WitnessLost("Receive recorded history pair absent"));
        }
    }
    Ok(())
}
