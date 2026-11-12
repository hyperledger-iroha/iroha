//! Bounded cryptographic turns over pinned exact originals, without custody/store locks.
use super::*;
use crate::kagemusha_wallet_artifacts_v1::producer_inventory::ImportedSigmaV1;
use crate::kagemusha_wallet_preparation_v1::UnloadChargeOriginalsV1;
use crate::kagemusha_wallet_suffix_unload_v1::{
    SuffixAnchorInputV1, SuffixCreditWitnessV1, SuffixErrorV1, SuffixPriorUnloadInputV1,
    SuffixReceiveInputV1, SuffixStepInputV1, SuffixUnloadClaimsV1, SuffixVerifierV1,
};
use iroha_kagemusha_proof::admin_sigma::ConsumingWitness;
use iroha_plonk::ProverRandomness;
use rand::rand_core::TryRng as _;

pub(super) enum Phase {
    Anchor(KagemushaWalletSuffixAnchorV1),
    Chunk {
        cursor: SuffixVerificationCursorV1,
        chunk_index: u128,
        stream_offset: u128,
        steps: Vec<Vec<u8>>,
    },
    Finish {
        tip: SuffixTipV1,
        input: FinishInputs,
    },
    Release {
        tip: SuffixTipV1,
        frozen: SuffixFrozenTransitionV1,
        completion: SuffixCompletionRecordV1,
    },
}
pub(super) enum Verified {
    Anchor(SuffixVerificationCursorV1),
    Chunk {
        cursor: SuffixVerificationCursorV1,
        original: Vec<u8>,
    },
    Finish {
        tip: SuffixTipV1,
        frozen: SuffixFrozenTransitionV1,
        projection: SuffixUnloadReviewProjectionV1,
        successor_custody: preparation_custody::SourceCustodyV1,
    },
    Release {
        operation: [u8; 32],
    },
}

/// A private bounded source-selected job. The bridge runs it after releasing the wallet owner.
/// This job is not a durable capability; dropping its owner cancels all its cryptographic work.
pub struct SuffixPreparationJobV1 {
    pub(super) binding: WorkBinding,
    pub(super) installed: Arc<InstalledVerifierPackV1>,
    pub(super) sources: Arc<QualifiedWalletSourcesV1>,
    pub(super) budget: MemoryBudget,
    pub(super) read: ReadConfig,
    pub(super) scheduler: Scheduler,
    pub(super) job: SpeculationJob<()>,
    pub(super) phase: Phase,
    pub(super) _lifetime: SuffixJobLifetime,
}
/// Result owned only by a completed Native job; no public constructor or codec.
pub struct ProvedSuffixPreparationV1 {
    pub(super) binding: WorkBinding,
    pub(super) verified: Verified,
}
pub(super) fn suffix<T>(result: Result<T, SuffixErrorV1>) -> Result<T, Error> {
    result.map_err(|error| match error {
        SuffixErrorV1::Cancelled => Error::Cancelled,
        SuffixErrorV1::Unavailable(_) => Error::NativeVerification(ProofError::Unavailable),
        SuffixErrorV1::Bound(field) => Error::Invalid(field),
        SuffixErrorV1::Rejected(field) => Error::Proof(field),
    })
}
fn model<T>(
    read: impl FnOnce() -> Result<T, KagemushaWalletValidationErrorV1>,
) -> Result<T, Error> {
    read_model_original(
        read,
        Error::WitnessLost("suffix selected canonical original"),
    )
}
fn input(step: &KagemushaWalletSuffixStepV1) -> Result<SuffixStepInputV1<'_>, Error> {
    model(|| step.validate())?;
    let [low, empty] = step.map_openings.as_slice() else {
        return Err(Error::WitnessLost("suffix step openings"));
    };
    Ok(match &step.kind {
        KagemushaWalletSuffixStepKindV1::Receive { credit } => {
            let original = |role| {
                step.original(role)
                    .ok_or(Error::WitnessLost("suffix Receive retained original"))
            };
            let credit = match credit {
                KagemushaWalletSuffixCreditWitnessV1::Present { membership } => {
                    SuffixCreditWitnessV1::Present { membership }
                }
                KagemushaWalletSuffixCreditWitnessV1::Inserted { low, empty } => {
                    SuffixCreditWitnessV1::Inserted { low, empty }
                }
            };
            SuffixStepInputV1::Receive(SuffixReceiveInputV1 {
                before: &step.before,
                after: &step.after,
                statement: &step.statement,
                sigma: &step.sigma,
                receipt: &step.receipt,
                consumed: [low, empty],
                credit,
                request: original(KagemushaWalletSuffixOriginalRoleV1::Request)?,
                payment: original(KagemushaWalletSuffixOriginalRoleV1::Payment)?,
                payer_credential: original(KagemushaWalletSuffixOriginalRoleV1::PayerCredential)?,
                payer_certificates: original(
                    KagemushaWalletSuffixOriginalRoleV1::PayerCertificates,
                )?,
            })
        }
        KagemushaWalletSuffixStepKindV1::PriorUnload {
            proof_digest,
            charge,
        } => SuffixStepInputV1::PriorUnload(SuffixPriorUnloadInputV1 {
            before: &step.before,
            after: &step.after,
            statement: &step.statement,
            sigma: &step.sigma,
            receipt: &step.receipt,
            proof_digest: *proof_digest,
            recovery: [low, empty],
            charge: charge.as_ref().map(|value| UnloadChargeOriginalsV1 {
                quote: &value.quote,
                certificate_set: &value.certificate_set,
            }),
        }),
    })
}

impl SuffixPreparationJobV1 {
    /// Obtain a stop-only handle for this exact ephemeral proof job.
    #[must_use]
    pub fn cancellation(&self) -> SuffixPreparationCancellationV1 {
        SuffixPreparationCancellationV1 {
            scheduler: self.scheduler.clone(),
            job: self.job.clone(),
        }
    }

    /// Fully verify exactly one anchor, at most eight semantic steps, or the final Unload
    /// sigma. The scheduler bounds proving memory across ordinary and suffix work. No
    /// partial progress is returned on cancellation, unavailable artifacts or invalid proofs.
    pub fn verify(self) -> Result<ProvedSuffixPreparationV1, Error> {
        let run = self
            .scheduler
            .speculate(&self.job)
            .ok_or(Error::Cancelled)?;
        let token = run.token().prover_token();
        let verifier = suffix(SuffixVerifierV1::native(
            &self.installed,
            &self.sources,
            self.budget,
        ))?;
        // Native uses the fully verifying constructor, never ledger's committed_anchor/link.
        let verified = match self.phase {
            Phase::Anchor(anchor) => {
                model(|| anchor.validate())?;
                let anchor = suffix(verifier.verify_anchor(
                    SuffixAnchorInputV1 {
                        credential: &anchor.credential,
                        certificates: &anchor.certificates,
                        lineage: &anchor.lineage,
                        state: &anchor.state,
                    },
                    Some(token),
                ))?;
                if anchor.identity().0 != self.binding.anchor_sequence {
                    return Err(Error::OperationConflict);
                }
                Verified::Anchor(suffix(verifier.cursor_at_anchor(&anchor))?)
            }
            Phase::Chunk {
                mut cursor,
                chunk_index,
                stream_offset,
                steps,
            } => {
                if steps.is_empty()
                    || steps.len() > KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1
                    || chunk_index >= KAGEMUSHA_WALLET_SUFFIX_CHUNKS_MAX_V1 as u128
                {
                    return Err(Error::Invalid("suffix bounded chunk turn"));
                }
                let start = *cursor.point().1;
                for bytes in &steps {
                    let step = model(|| {
                        KagemushaWalletSuffixStepV1::decode_canonical(
                            bytes,
                            &self.installed.verifier().scheme().scheme_id(),
                        )
                    })?;
                    suffix(cursor.verify_step(&verifier, input(&step)?, Some(token)))?;
                }
                suffix(cursor.settle(&verifier, Some(token)))?;
                let chunk = KagemushaWalletSuffixChunkV1 {
                    version: 1,
                    chunk_index,
                    stream_offset,
                    start,
                    data: model(|| KagemushaWalletSuffixChunkV1::stream(&steps))?,
                };
                Verified::Chunk {
                    cursor,
                    original: model(|| chunk.to_canonical_bytes())?,
                }
            }
            Phase::Release {
                tip,
                frozen,
                completion,
            } => Self::release(
                &self.installed,
                &verifier,
                tip,
                frozen,
                completion,
                self.binding,
                token,
            )?,
            Phase::Finish { tip, input } => Self::finish(
                &self.installed,
                &self.sources,
                self.read,
                self.budget,
                &verifier,
                tip,
                input,
                self.binding,
                token,
            )?,
        };
        run.token().check()?;
        run.finish(());
        Ok(ProvedSuffixPreparationV1 {
            binding: self.binding,
            verified,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn finish(
        installed: &InstalledVerifierPackV1,
        sources: &QualifiedWalletSourcesV1,
        read: ReadConfig,
        budget: MemoryBudget,
        verifier: &SuffixVerifierV1<'_>,
        tip: SuffixTipV1,
        input: FinishInputs,
        binding: WorkBinding,
        cancellation: &iroha_pasta::CancellationToken,
    ) -> Result<Verified, Error> {
        input.request.validate()?;
        if input.source_capsule != binding.source_capsule
            || tip.point().0.core.sequence != binding.source_sequence
            || tip.anchor().0 != binding.anchor_sequence
            || input.request.request_id != binding.request_id
            || input.chunk_digests.is_empty()
            || input.chunk_digests.len() > KAGEMUSHA_WALLET_SUFFIX_CHUNKS_MAX_V1
        {
            return Err(Error::OperationConflict);
        }
        let charge = input
            .request
            .charge
            .as_ref()
            .map(|value| UnloadChargeOriginalsV1 {
                quote: &value.quote,
                certificate_set: &value.certificates,
            });
        let derivation = suffix(verifier.derive_unload(
            &tip,
            input.request.amount,
            &input.recovery,
            charge,
            input.nonce,
        ))?;
        let sigma = if let Some(existing) = &input.existing {
            existing.validate()?;
            if existing.credential != input.credential
                || existing.certificates() != archive::encode(&input.certificates)?
                || existing.capsule.body().statement != derivation.statement
                || existing.capsule.body().successor_state != derivation.successor
                || existing.capsule.body().map_openings != derivation.openings
            {
                return Err(Error::WitnessLost("suffix selected derivation changed"));
            }
            existing.capsule.body().step_proof.clone()
        } else {
            let preparation = proof(PreparationV1::new(installed))?.with_cancellation(cancellation);
            let issuer = model(|| {
                input.certificates.certificate(
                    &input.credential.body.issuer_certificate,
                    KagemushaWalletSignerRoleV1::Enrollment,
                )
            })?;
            let owner = proof(preparation.authenticate_credential(
                &model(|| input.credential.to_canonical_bytes())?,
                &model(|| issuer.to_canonical_bytes())?,
            ))?;
            let witness = ConsumingWitness {
                predecessor: proof(preparation.state_fields(
                    &owner,
                    &derivation.predecessor,
                    &derivation.predecessor_adjusted,
                ))?,
                successor: proof(preparation.state_fields(
                    &owner,
                    &derivation.successor,
                    &derivation.successor_adjusted,
                ))?,
                statement: proof(preparation.statement_fields(&owner, &derivation.statement))?,
            };
            // The locked preparation revalidated these same signed originals; this import binds
            // their sealed source outside that lock, preserving the installed selector/key.
            let imported = sources
                .bind_sigma_cancellable(8, read, Some(cancellation))
                .map_err(|error| {
                    if error.is_cancelled() {
                        Error::Cancelled
                    } else if error.is_unavailable() {
                        Error::ArtifactsUnavailable("suffix Unload sigma originals")
                    } else {
                        Error::Proof("suffix Unload sigma source binding")
                    }
                })?;
            let ImportedSigmaV1::Unload(prover) = imported else {
                return Err(Error::Proof("suffix Unload installed selector"));
            };
            proof(preparation.prove_unload_sigma(
                &owner,
                &witness,
                &derivation.statement,
                &prover,
                ProverRandomness::os(),
                budget,
            ))?
        };
        suffix(verifier.verify_unload_sigma(&derivation.statement, &sigma, Some(cancellation)))?;
        let charge = match (&input.request.charge, &input.request.beneficiary) {
            (None, None) => KagemushaWalletUnloadChargeV1::None,
            (Some(charge), Some(beneficiary)) => KagemushaWalletUnloadChargeV1::Quoted {
                quote: model(|| {
                    KagemushaWalletChargeQuoteV1::decode_canonical(
                        &charge.quote,
                        &input.credential.body.scheme_id,
                    )
                })?,
                beneficiary: archive::decode(beneficiary)?,
            },
            _ => return Err(Error::Invalid("suffix charge selection")),
        };
        let header = KagemushaWalletSuffixHeaderV1 {
            version: 1,
            method: KAGEMUSHA_WALLET_SUFFIX_METHOD_NATIVE_V1,
            network_id: installed.verifier().scheme().network_id,
            scheme_id: input.credential.body.scheme_id,
            wallet_id: input.credential.body.wallet_id,
            verifier_definition: kagemusha_wallet_suffix_verifier_definition_v1(
                installed.original(),
            ),
            artifact_manifest: installed.verifier().manifest_digest(),
            source_capsule: input.source_capsule,
            request_id: input.request.request_id,
            anchor_digest: input.anchor_digest,
            anchor_sequence: binding.anchor_sequence,
            step_count: u128::from(tip.steps().0),
            chunk_count: input.chunk_digests.len() as u128,
            stream_bytes: input.stream_bytes,
            chunk_digests: input.chunk_digests,
            tip: model(|| derivation.predecessor.commitment())?,
            tip_sequence: binding.source_sequence,
            tip_adjusted: derivation.predecessor_adjusted,
            statement: derivation.statement,
            sigma,
            recovery_witness_digest: model(|| {
                kagemusha_wallet_suffix_recovery_witness_digest_v1(&derivation.openings)
            })?,
            account: input.account,
            charge,
        };
        let header_original = model(|| header.to_canonical_bytes())?;
        let authority = model(|| kagemusha_wallet_suffix_authority_digest_v1(&header_original))?;
        let terms = model(|| header.payout_terms())?;
        let body = SuffixRecoveryBodyV1 {
            version: 1,
            discriminator: 10,
            scheme_id: header.scheme_id,
            wallet_id: header.wallet_id,
            operation_id: model(|| header.statement.operation_id(&header.wallet_id))?,
            predecessor_capsule_digest: input.source_capsule,
            successor_state: derivation.successor,
            statement: header.statement,
            step_proof: header.sigma.clone(),
            authority_header: header_original,
            authority_digest: authority,
            pinned_sources_root: input.pins.0,
            map_openings: derivation.openings,
            output: model(|| {
                KagemushaWalletOutputDescriptorV1::for_transition(
                    &header.statement,
                    &authority,
                    &[0; 32],
                )
            })?,
        };
        let frozen = if let Some(existing) = input.existing {
            if existing.capsule.body() != &body {
                return Err(Error::WitnessLost("suffix selected body changed"));
            }
            existing
        } else {
            // Freeze all body bytes first. Only then draw one independent canonical opening.
            let frozen_body = model(|| body.freeze())?;
            let mut opening = zeroize::Zeroizing::new([0; 32]);
            let mut accepted = false;
            for _ in 0..128 {
                rand::rngs::SysRng
                    .try_fill_bytes(&mut *opening)
                    .map_err(|_| {
                        ProviderError::Unavailable(
                        crate::kagemusha_wallet_advance_v1::KagemushaWalletUnavailableV1::Platform(
                            0,
                        ),
                    )
                    })?;
                opening[31] &= 0x7f;
                if kagemusha_wallet_is_canonical_field_v1(&opening) {
                    accepted = true;
                    break;
                }
            }
            if !accepted {
                return Err(ProviderError::Unavailable(
                    crate::kagemusha_wallet_advance_v1::KagemushaWalletUnavailableV1::Platform(0),
                )
                .into());
            }
            // Seal refusal never retries with another opening or changes the body.
            let capsule = model(|| frozen_body.seal(*opening))?;
            SuffixFrozenTransitionV1 {
                credential: input.credential,
                certificates: archive::encode(&input.certificates)?,
                capsule,
            }
        };
        frozen.validate()?;
        let projection = SuffixUnloadReviewProjectionV1 {
            format_version: 1,
            discriminator: 13,
            request_id: input.request.request_id,
            scheme_id: header.scheme_id,
            wallet_id: header.wallet_id,
            authority_digest: authority,
            source_operation_id: input.source_operation,
            source_capsule_digest: input.source_capsule,
            source_state_commitment: header.tip.value,
            source_sequence: binding.source_sequence,
            anchor_sequence: binding.anchor_sequence,
            suffix_step_count: header.step_count,
            chunk_count: header.chunk_count,
            instruction_count: header.chunk_count + 2,
            amount: terms.amount,
            charge: terms.online_charge,
            gross_debit: terms.amount,
            net_destination_amount: terms.account_payout,
            destination_account_digest: input.credential.body.account_digest,
            destination_account_original: input.account_original,
            charge_quote_digest: terms.charge_quote,
            charge_beneficiary_original: input.request.beneficiary,
            credential_digest: input.credential.credential_digest(),
            artifact_manifest_digest: header.artifact_manifest,
            payment_key: input.credential.body.payment_key,
        };
        model(|| projection.validate())?;
        Ok(Verified::Finish {
            tip,
            frozen,
            projection,
            successor_custody: input.successor_custody,
        })
    }

    fn release(
        installed: &InstalledVerifierPackV1,
        verifier: &SuffixVerifierV1<'_>,
        tip: SuffixTipV1,
        frozen: SuffixFrozenTransitionV1,
        completion: SuffixCompletionRecordV1,
        binding: WorkBinding,
        cancellation: &iroha_pasta::CancellationToken,
    ) -> Result<Verified, Error> {
        frozen.validate()?;
        let body = frozen.capsule.body();
        if body.predecessor_capsule_digest != binding.source_capsule
            || tip.point().0.core.sequence != binding.source_sequence
            || tip.anchor().0 != binding.anchor_sequence
        {
            return Err(Error::OperationConflict);
        }
        let header = model(|| {
            KagemushaWalletSuffixHeaderV1::decode_canonical(
                &body.authority_header,
                &installed.verifier().scheme().scheme_id(),
            )
        })?;
        // The released source is the suffix tip, so the header's tip sequence is the
        // bound source sequence.
        let selected = (
            header.request_id,
            header.source_capsule,
            header.tip_sequence,
            header.anchor_sequence,
        );
        let bound = (
            binding.request_id,
            binding.source_capsule,
            binding.source_sequence,
            binding.anchor_sequence,
        );
        if selected != bound
            || header.artifact_manifest != installed.verifier().manifest_digest()
            || header.verifier_definition
                != kagemusha_wallet_suffix_verifier_definition_v1(installed.original())
        {
            return Err(Error::WitnessLost("suffix release selected header"));
        }
        model(|| completion.verify(&frozen.credential, &frozen.capsule))?;
        let output = model(|| {
            KagemushaWalletSuffixUnloadV1::decode_canonical(&completion.output, &header.scheme_id)
        })?;
        if archive::encode(&output.certificates)? != frozen.certificates() {
            return Err(Error::WitnessLost("suffix release exact certificates"));
        }
        let terms = model(|| {
            output.verify_terms(
                installed.verifier().scheme(),
                &header,
                &body.authority_digest,
            )
        })?;
        let [low, empty] = output.map_openings.as_slice() else {
            return Err(Error::WitnessLost("suffix release recovery pair"));
        };
        let quote = header
            .charge
            .quote()
            .map(|quote| model(|| quote.to_canonical_bytes()))
            .transpose()?;
        let certificates = archive::encode(&output.certificates)?;
        let verified = suffix(verifier.finish(
            tip,
            SuffixUnloadClaimsV1 {
                authority_digest: body.authority_digest,
                anchor_sequence: header.anchor_sequence,
                step_count: header.step_count,
                tip: header.tip,
                tip_sequence: header.tip_sequence,
                tip_adjusted: &header.tip_adjusted,
                statement: &header.statement,
                sigma: &header.sigma,
                after: &output.after,
                recovery: [low, empty],
                charge: quote.as_ref().map(|quote| UnloadChargeOriginalsV1 {
                    quote,
                    certificate_set: &certificates,
                }),
                receipt: &output.receipt,
            },
            Some(cancellation),
        ))?;
        if verified.credential() != &frozen.credential
            || verified.successor().0 != &output.after
            || verified.statement().0 != &header.statement
            || verified.receipt() != &completion.receipt
            || verified.terms() != (terms.amount, terms.online_charge, terms.charge_quote)
            || verified.authority_digest() != body.authority_digest
        {
            return Err(Error::WitnessLost("suffix release verified binding"));
        }
        Ok(Verified::Release {
            operation: completion.operation_id,
        })
    }
}

#[cfg(test)]
mod observer_tests {
    use super::*;
    use crate::kagemusha_wallet_state_v1::tests::fixture;

    #[test]
    fn model_decoder_preserves_enclosing_refusal_and_exact_retry() {
        let payment: KagemushaWalletPaymentV1 = fixture("KagemushaWalletPaymentV1");
        let original = payment.to_canonical_bytes().unwrap();
        let scheme = payment.send.statement.scheme_id;
        let read = || model(|| KagemushaWalletPaymentV1::decode_canonical(&original, &scheme));
        let limits = norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 64);
        let error = norito::with_decode_limits_scope(limits, read).unwrap_err();
        assert!(matches!(error, Error::DecodeResource(ref cause)
            if cause.kind() == norito::core::DecodeLocalRefusalKind::EnclosingLimit));
        assert_eq!(read().unwrap().to_canonical_bytes().unwrap(), original);
        assert!(matches!(
            model(|| KagemushaWalletPaymentV1::decode_canonical(&[0], &scheme)),
            Err(Error::WitnessLost(_))
        ));
    }
}
