//! Original execution custody through native Pasta witness and receipt publication refusals.

use super::*;
use crate::sumeragi::attestation::{
    AttestationPublishError, LocalCommitAttestation, NativeAttestationPublisher,
    NativePastaVerifier, attest_original,
};
use crate::zk::kagemusha_v1_recursion::KagemushaMintFinalityLocalAuthorityV1;
use iroha_allocation::{ChargedBuffer, ChargedBufferError};
use iroha_sumeragi::message::{ByteAdmissionError, ResultWitness};

const _: () = assert!(
    super::super::commitment::MAX_RESULT_PREIMAGE_BYTES
        == iroha_sumeragi::message::MAX_RESULT_WITNESS_BYTES
);

pub(super) struct Custody {
    verifier: NativePastaVerifier,
    authority: Option<Arc<KagemushaMintFinalityLocalAuthorityV1>>,
    pub(super) publisher: NativeAttestationPublisher,
}

/// Every phase owns its actual backing. Admission never reconstructs a prior successful phase.
pub(super) enum Progress {
    WaitingBacking(Option<ChargedBufferError>),
    WaitingControl {
        bytes: ChargedBuffer<u8>,
        refusal: Option<ByteAdmissionError>,
    },
    Signing(ResultWitness),
    Publishing(LocalCommitAttestation),
    Published,
    /// Startup replay or a node outside the exact scheduled committee has no local vote to sign.
    NotSeated,
}

impl Worker<'_> {
    pub(super) fn attach_attestation(
        &mut self,
        verifier: NativePastaVerifier,
        authority: Option<Arc<KagemushaMintFinalityLocalAuthorityV1>>,
        publisher: NativeAttestationPublisher,
    ) -> Result<(), PublicationError> {
        if self.attestation.is_some() {
            return Err(PublicationError::RecoveryRequired(
                "native Pasta custody is already attached".into(),
            ));
        }
        if let Some(reason) = &self.recovery {
            return Err(PublicationError::RecoveryRequired(reason.clone()));
        }
        // Attach follows startup replay and precedes the driver. A live uncommitted
        // overlay must not have bypassed local signing by executing before this boundary.
        if self.finishing.is_some()
            || self
                .live
                .as_ref()
                .is_some_and(|live| !matches!(live.phase, PublicationPhase::Published { .. }))
        {
            return Err(PublicationError::RecoveryRequired(
                "cannot attach native custody over an uncommitted execution".into(),
            ));
        }
        self.attestation = Some(Custody {
            verifier,
            authority,
            publisher,
        });
        Ok(())
    }

    pub(super) fn clear_local_attestation(&mut self) -> Result<(), String> {
        if self
            .attestation
            .as_ref()
            .is_some_and(|custody| !custody.publisher.discard(0, &[]))
        {
            let reason = "native attestation mailbox requires recovery".to_owned();
            self.recovery = Some(reason.clone());
            return Err(reason);
        }
        Ok(())
    }

    pub(super) fn finish_local_attestation(&mut self) -> ExecOutcome {
        let Some(live) = self.live.as_mut() else {
            return ExecOutcome::Failed("local attestation lacks its original execution".into());
        };
        if !live.header.attest
            || matches!(live.attestation, Progress::Published | Progress::NotSeated)
        {
            return ExecOutcome::Valid(live.result);
        }
        let Some(custody) = self.attestation.as_ref() else {
            // Startup replay runs before custody attachment and separately verifies
            // the complete persisted native QC before preparing any publication.
            live.attestation = Progress::NotSeated;
            return ExecOutcome::Valid(live.result);
        };
        let seated = live
            .commitment
            .get()
            .schedule
            .current
            .committee
            .iter()
            .any(|member| {
                super::super::crypto::core_key(member.validator.public_key())
                    .is_ok_and(|key| key.as_bytes() == custody.publisher.key())
            });
        if !seated {
            live.attestation = Progress::NotSeated;
            return ExecOutcome::Valid(live.result);
        }
        let Some(authority) = custody.authority.as_ref() else {
            let reason =
                "scheduled local seat lacks its provisioned native Pasta custody".to_owned();
            self.recovery = Some(reason.clone());
            return ExecOutcome::Failed(reason);
        };
        let PublicationPhase::Executed { preimage, .. } = &live.phase else {
            let reason = "local attestation lost its original result preimage".to_owned();
            self.recovery = Some(reason.clone());
            return ExecOutcome::Failed(reason);
        };
        let budget = self.state.ivm_execution_budget();
        if matches!(live.attestation, Progress::WaitingBacking(_)) {
            let mut bytes = match ChargedBuffer::new(preimage.as_slice().len(), &budget) {
                Ok(bytes) => bytes,
                Err(error) => {
                    let reason = error.to_string();
                    live.attestation = Progress::WaitingBacking(Some(error));
                    return ExecOutcome::Failed(reason);
                }
            };
            bytes
                .append(preimage.as_slice())
                .expect("exact admitted original witness capacity");
            live.attestation = Progress::WaitingControl {
                bytes,
                refusal: None,
            };
        }
        if matches!(live.attestation, Progress::WaitingControl { .. }) {
            let Progress::WaitingControl { bytes, .. } =
                std::mem::replace(&mut live.attestation, Progress::WaitingBacking(None))
            else {
                unreachable!("same retained witness backing")
            };
            match ResultWitness::from_charged(bytes, &budget) {
                Ok(witness) => live.attestation = Progress::Signing(witness),
                Err((bytes, error)) => {
                    let reason = error.to_string();
                    if !error.is_local_refusal() {
                        self.recovery = Some(reason.clone());
                    }
                    live.attestation = Progress::WaitingControl {
                        bytes,
                        refusal: Some(error),
                    };
                    return ExecOutcome::Failed(reason);
                }
            }
        }
        if matches!(live.attestation, Progress::Signing(_)) {
            let Progress::Signing(witness) =
                std::mem::replace(&mut live.attestation, Progress::WaitingBacking(None))
            else {
                unreachable!("same immutable admitted witness")
            };
            match attest_original(
                authority,
                custody.verifier,
                &live.header,
                &live.commitment,
                preimage,
                witness,
                &budget,
            ) {
                Ok(receipt) => live.attestation = Progress::Publishing(receipt),
                Err((witness, error)) => {
                    let reason = error.to_string();
                    live.attestation = Progress::Signing(witness);
                    self.recovery = Some(reason.clone());
                    return ExecOutcome::Failed(reason);
                }
            }
        }
        let Progress::Publishing(receipt) =
            std::mem::replace(&mut live.attestation, Progress::WaitingBacking(None))
        else {
            unreachable!("same signed original receipt")
        };
        match custody.publisher.publish(receipt) {
            Ok(()) => {
                live.attestation = Progress::Published;
                ExecOutcome::Valid(live.result)
            }
            Err((receipt, error)) => {
                let reason = format!("native attestation publication refused: {error:?}");
                if !matches!(error, AttestationPublishError::Busy) {
                    self.recovery = Some(reason.clone());
                }
                live.attestation = Progress::Publishing(receipt);
                ExecOutcome::Failed(reason)
            }
        }
    }

    /// Reconstruct authority from the independently authenticated committed source even
    /// during startup replay, before a local signer or mailbox has been attached.
    pub(super) fn verify_prepared_certificate(
        &self,
        block: &AvailableBody,
        qc: &Qc,
    ) -> Result<(), String> {
        let view = self
            .state
            .try_view_once()
            .map_err(|error| error.to_string())?
            .ok_or("committed publication is busy")?;
        let genesis = crate::sumeragi::certified_chain::committed_block(&view, 1)
            .map_err(|error| error.to_string())?;
        let instance =
            crate::sumeragi::node::global_instance(genesis.block(), &view.chain_id().to_string());
        let scheduled = view
            .world()
            .consensus_schedule()
            .ready(block.header().height)
            .map_err(|error| error.to_string())?;
        let config = scheduled
            .height_config()
            .map_err(|error| error.to_string())?;
        if block.source().instance() != instance || block.source().config() != &config {
            return Err(
                "available body does not bind the independently authenticated authority".into(),
            );
        }
        let crypto = self
            .context
            .crypto
            .as_ref()
            .ok_or("native committee crypto is not attached")?;
        // New scheduled keys must be admitted from their exact authenticated PoPs.
        for member in &scheduled.epoch.committee {
            crypto
                .admit(member.validator.public_key(), &member.proof_of_possession)
                .map_err(|error| error.to_string())?;
        }
        let verifier = NativePastaVerifier::new(instance, *view.network_id());
        iroha_sumeragi::crypto::Verifier::new(
            &**crypto,
            &instance,
            &config.epoch.id,
            &config.committee,
        )
        .verify_qc(&verifier, qc)
        .map_err(|error| format!("native quorum verification failed: {error:?}"))
    }
}

impl Drop for Worker<'_> {
    fn drop(&mut self) {
        // The driver's read handle can outlive the serialized worker. Clear its
        // receipt before Rust drops any original overlay. A poisoned mailbox is
        // already unreadable to the attestor and requires process recovery.
        if let Some(custody) = &self.attestation {
            let _ = custody.publisher.discard(0, &[]);
        }
    }
}
