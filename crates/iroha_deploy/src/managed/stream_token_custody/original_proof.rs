//! Original current-only custody witnesses retained separately from small signing journals.
use super::*;
use iroha_data_model::sorafs::stream_token_custody::proof::{
    MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1, StreamTokenCustodyProofV1,
};

pub(in crate::managed) const DIRECTORY: &str = "original-custody-proofs";
pub(in crate::managed) struct CapturedCustody {
    pub(in crate::managed) state: VerifiedStreamTokenCustodyStateV1,
    pub(in crate::managed) frame: Vec<u8>,
}

/// Cold component fixtures can exercise previously published journals without a witness.
pub(super) enum SelectionWitness<'a> {
    Captured(&'a [u8]),
    #[cfg(test)]
    PreviouslyUncaptured,
}
impl SelectionWitness<'_> {
    pub(super) fn publish(
        self,
        owner: &ManagedStreamTokenCustody,
        block_hash: &[u8; 32],
    ) -> Result<()> {
        match self {
            Self::Captured(frame) => {
                publish_proof(&owner.authority.directory, block_hash, frame, MAX_PROOFS)
            }
            #[cfg(test)]
            Self::PreviouslyUncaptured => Ok(()),
        }
    }
}

// Configure and each initial/renewal body can introduce one original cut. Allow one
// interrupted replacement publication per bounded authorization epoch as well.
const MAX_PROOFS: usize = 1
    + (renewal::MAX_GENERATED_SEQUENCE as usize) * (body_history::MAX_BODIES as usize)
    + crate::managed::native_operation::authorization::MAX_EPOCHS;

pub(in crate::managed) fn validate_names(parent: &PrivateDirectory) -> Result<()> {
    proof_count(&parent.open_child(DIRECTORY)?).map(|_| ())
}

fn proof_count(directory: &PrivateDirectory) -> Result<usize> {
    directory.read_scope(|scope| {
        let names = scope.entries(MAX_PROOFS)?;
        for name in &names {
            let canonical = name
                .to_str()
                .and_then(|name| name.strip_suffix(".nrt"))
                .is_some_and(|hash| {
                    hash.len() == 64
                        && hash
                            .bytes()
                            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                });
            if !canonical {
                return Err(invalid("invalid original custody proof name"));
            }
        }
        Ok(names.len())
    })
}

fn name(hash: &[u8; 32]) -> String {
    format!("{}.nrt", hex::encode(hash))
}

fn publish_proof(
    parent: &PrivateDirectory,
    block_hash: &[u8; 32],
    frame: &[u8],
    max_proofs: usize,
) -> Result<()> {
    if frame.is_empty() || frame.len() > MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1 {
        return Err(invalid("original custody proof exceeds its bound"));
    }
    let directory = parent.ensure_child(DIRECTORY)?;
    let name = name(block_hash);
    match directory.read_optional(&name, MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1)? {
        Some(bytes) if bytes.as_slice() == frame => return Ok(()),
        Some(_) => return Err(invalid("published original custody proof changed")),
        None => {
            if proof_count(&directory)? >= max_proofs {
                return Err(invalid("original custody proof directory is full"));
            }
            match directory.write_atomic(&name, frame, PublishMode::CreateNew) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    if directory
        .read(&name, MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1)?
        .as_slice()
        != frame
    {
        return Err(invalid("original custody proof changed at publication"));
    }
    Ok(())
}

impl ManagedStreamTokenCustody {
    pub(super) fn historical_custody(
        &self,
        binding: &SignerCustodyBindingV1,
        checkpoint: &FinalityVerifier,
        deadline: Instant,
    ) -> Result<VerifiedStreamTokenCustodyStateV1> {
        self.historical_custody_using(binding, checkpoint, deadline, |tip| {
            self.capture_current_at(binding, tip, deadline)
        })
    }

    fn historical_custody_using(
        &self,
        binding: &SignerCustodyBindingV1,
        checkpoint: &FinalityVerifier,
        deadline: Instant,
        capture_original: impl FnOnce(&VerifiedSumeragiBlock) -> Result<CapturedCustody>,
    ) -> Result<VerifiedStreamTokenCustodyStateV1> {
        require_deadline(deadline)?;
        let tip = checkpoint
            .verified_tip_ref()
            .map_err(|_| invalid("invalid original custody checkpoint"))?;
        let block_hash = *tip.header().hash().as_ref();
        tip.verify_global_scope(
            self.authority.config.network_id,
            self.authority.config.chain.as_str(),
        )
        .map_err(|_| {
            invalid("original custody checkpoint differs from the configured Global scope")
        })?;
        let verify = |frame: &[u8]| -> Result<VerifiedStreamTokenCustodyStateV1> {
            let proof = StreamTokenCustodyProofV1::decode_frame(frame)
                .map_err(|_| invalid("invalid retained original custody frame"))?;
            let schemas = iroha_core::state::State::native_world_schema_hashes_v1()
                .map_err(|_| invalid("native custody schema is unavailable"))?;
            let state = proof
                .verify(
                    self.authority.config.network_id,
                    self.authority.provider_id()?,
                    self.authority
                        .provider_role(StreamTokenAuthorityRole::IssuerOperator)?,
                    binding,
                    schemas.as_slice(),
                    tip,
                )
                .map_err(|_| {
                    invalid("retained custody proof differs from original checkpoint or binding")
                })?;
            require_deadline(deadline)?;
            Ok(state)
        };
        if let Some(directory) = self.authority.directory.open_child_optional(DIRECTORY)?
            && let Some(frame) = directory
                .read_optional(name(&block_hash), MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1)?
        {
            return verify(&frame);
        }
        // Previously published journals may acquire only the original independently verified cut.
        // A newer World snapshot, a selected row alone, or peer unavailability cannot fill it.
        let captured = capture_original(tip)?;
        let state = verify(&captured.frame)?;
        publish_proof(
            &self.authority.directory,
            &block_hash,
            &captured.frame,
            MAX_PROOFS,
        )?;
        require_deadline(deadline)?;
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed::{
        native_operation::test_support::native_fixture::quote_instructions,
        stream_token_custody::renewal_tests::Fixture,
    };
    use iroha_data_model::isi::{InstructionBox, Log};
    use std::time::Duration;

    #[test]
    fn original_proof_namespace_bounds_new_cuts_and_preserves_immutable_records() {
        let temporary = tempfile::tempdir().unwrap();
        let parent =
            PrivateDirectory::open_or_create(temporary.path().join("proof-owner")).unwrap();
        assert!(publish_proof(&parent, &[1; 32], &[], 1).is_err());
        assert!(parent.open_child_optional(DIRECTORY).unwrap().is_none());
        publish_proof(&parent, &[1; 32], b"first", 1).unwrap();
        publish_proof(&parent, &[1; 32], b"first", 1).unwrap();
        assert!(publish_proof(&parent, &[1; 32], b"changed", 1).is_err());
        assert!(publish_proof(&parent, &[2; 32], b"second", 1).is_err());
        let directory = parent.open_child(DIRECTORY).unwrap();
        assert_eq!(directory.entries(1).unwrap().len(), 1);
        assert_eq!(
            directory.read(name(&[1; 32]), 8).unwrap().as_slice(),
            b"first"
        );
        validate_names(&parent).unwrap();
        directory
            .write_atomic("unexpected.nrt", b"x", PublishMode::CreateNew)
            .unwrap();
        assert!(validate_names(&parent).is_err());
    }

    #[test]
    fn original_proof_survives_tip_advance_for_attester_and_backfills_only_its_own_cut() {
        let _guard = crate::managed::native_test_guard();
        let mut fixture =
            Fixture::predecessor_with_policy_lifetime(600_000, Duration::from_secs(1_800));
        fixture.wait_renewable();
        fixture.begin_operation();
        let deadline = fixture.deadline();
        let cut = fixture.native.observe(&fixture.owner.authority);
        let captured = fixture
            .native
            .bootstrap_custody_captured(
                &fixture.owner.authority,
                &fixture.policy,
                &cut,
                Some(deadline),
            )
            .unwrap();
        let now = now_ms().unwrap();
        let terms = Terms::new(now + 180_000, &fixture.options()).unwrap();
        let unsigned = fixture
            .owner
            .select_renewal_unsigned(2, &fixture.policy, &captured.state, &cut, &terms, deadline)
            .unwrap();
        let body = BodyHistory::initialize_with_witness(
            &fixture.owner,
            CustodyPurpose::Renewal(2),
            unsigned,
            &terms.fees,
            &SigningTurn::Explicit(&terms),
            deadline,
            SelectionWitness::Captured(&captured.frame),
        )
        .unwrap();
        let log = quote_instructions(
            &fixture.native,
            &fixture.owner.authority.config,
            [InstructionBox::from(Log::new(
                iroha_data_model::Level::INFO,
                "advance past custody reservation".into(),
            ))],
        );
        assert_eq!(fixture.native.chain.commit(vec![log]), vec![true]);
        let advanced = fixture.native.observe(&fixture.owner.authority);
        let fresh = fixture
            .native
            .bootstrap_custody_captured(
                &fixture.owner.authority,
                &fixture.policy,
                &advanced,
                Some(deadline),
            )
            .unwrap();
        assert!(fresh.state.height() > captured.state.height());
        let body = body
            .finish_pending(
                &fixture.owner,
                &fresh.state,
                &SigningTurn::Explicit(&terms),
                deadline,
            )
            .unwrap();
        let original = body
            .original()
            .expect("the actual attester signed after the chain advanced");
        let recovered = fixture
            .owner
            .historical_custody(&fixture.policy.binding, &cut, deadline)
            .unwrap();
        assert_eq!(recovered.context_id(), captured.state.context_id());
        fixture
            .owner
            .fresh_predecessor_observation(original, &recovered, &fresh.state, deadline)
            .unwrap();

        // An older journal can fill missing evidence only if that exact original still verifies.
        let backfill = |candidate| {
            fixture.owner.historical_custody_using(
                &fixture.policy.binding,
                &advanced,
                deadline,
                |_| candidate,
            )
        };
        assert!(backfill(Err(invalid("original cut unavailable"))).is_err());
        assert!(backfill(Ok(captured)).is_err());
        let expected = fresh.state.context_id();
        assert_eq!(backfill(Ok(fresh)).unwrap().context_id(), expected);
        let again = fixture
            .owner
            .historical_custody_using(&fixture.policy.binding, &advanced, deadline, |_| {
                panic!("retained original must not refetch")
            })
            .unwrap();
        assert_eq!(again.context_id(), expected);
    }
}
