//! Owner-private advancing checkpoints rooted in the retained native network identity.

use std::{fs::File, num::NonZeroU64, path::Path};

use iroha_data_model::sumeragi_finality::{SumeragiFinalityCheckpoint, SumeragiFinalityVerifier};
use iroha_fs::{FileIdentity, OwnerDirectory, PrivateDirectory, PublishMode};
use norito::{Decode, Encode};

use super::{AuthenticatedBootstrap, BootstrapError, MAX_BOOTSTRAP_BYTES, Result, decode};
use crate::verify::finality::{AttestationQuorum, FinalityError, FinalitySource, FinalityVerifier};

#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::bootstrap::RuntimeCheckpointV1")]
struct Record {
    network_name: String,
    checkpoint: Vec<u8>,
}

/// Exclusive custody of one parent generation's advancing native finality prefix.
///
/// Reopening retains newer verified progress even when the bootstrap starts earlier.
/// A reset needs a separate context. Only verification operations can advance this store;
/// publication succeeds before their in-memory state or readiness report is exposed.
pub struct ParentFinalityStore {
    directory: PrivateDirectory,
    lock: File,
    network_name: String,
    verifier: FinalityVerifier,
    publication_uncertain: bool,
}

impl ParentFinalityStore {
    /// Create from authenticated native bootstrap material or reopen the same generation.
    /// Missing/corrupt records or locks are not repaired. This does not establish fresh readiness.
    ///
    /// # Errors
    /// Unsafe custody, concurrent ownership, incomplete creation, changed network,
    /// noncanonical checkpoint or a conflicting independently signed same-height decision.
    pub fn open(path: &Path, bootstrap: &AuthenticatedBootstrap) -> Result<Self> {
        let name = path
            .file_name()
            .ok_or(BootstrapError::Invalid("parent checkpoint path"))?;
        let parent = path
            .parent()
            .ok_or(BootstrapError::Invalid("parent checkpoint path"))?;
        let parent = OwnerDirectory::open_or_create(parent)?;
        let release = bootstrap.metadata();
        let initial = record_bytes(
            &release.network_name,
            &bootstrap.verifier,
        )?;
        let (directory, created) = match parent
            .publish_private_child(name, &[("lock", &[]), ("verified.nrt", &initial)])
        {
            Ok(directory) => (directory, true),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                (PrivateDirectory::open(parent.path().join(name))?, false)
            }
            Err(error) => return Err(error.into()),
        };
        let lock = directory.open_existing_lock("lock")?;
        lock.try_lock()
            .map_err(|_| BootstrapError::Invalid("parent finality custody is already in use"))?;
        lock.sync_all()?;
        directory.sync()?;
        let verifier = if created {
            bootstrap.verifier.clone()
        } else {
            let record: Record = decode(
                &directory.read("verified.nrt", MAX_BOOTSTRAP_BYTES)?,
                MAX_BOOTSTRAP_BYTES,
            )?;
            if record.network_name != release.network_name
            {
                return Err(BootstrapError::Invalid(
                    "parent network differs; create a new context",
                ));
            }
            let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(&record.checkpoint)
                .map_err(|_| BootstrapError::Invalid("invalid retained parent checkpoint"))?;
            if checkpoint.height() < 2 {
                return Err(BootstrapError::Invalid(
                    "retained parent checkpoint lacks certified successor",
                ));
            }
            let verifier = FinalityVerifier::from_checkpoint(
                checkpoint,
                release.network_id,
                &release.chain_id,
            )?;
            if verifier.checkpoint().height() == bootstrap.verifier.checkpoint().height() {
                let same = SumeragiFinalityVerifier::from_trusted_checkpoint(
                    verifier.checkpoint(),
                    &release.network_id,
                    &release.chain_id,
                )
                .map_err(FinalityError::from)?;
                same.verify_same_decision(
                    verifier.checkpoint().tip(),
                    bootstrap.verifier.checkpoint().tip(),
                )
                .map_err(FinalityError::from)?;
            }
            if bootstrap.verifier.checkpoint().height() > verifier.checkpoint().height() {
                bootstrap.verifier.clone()
            } else {
                verifier
            }
        };
        let mut result = Self {
            directory,
            lock,
            network_name: release.network_name.clone(),
            verifier,
            publication_uncertain: false,
        };
        let selected = result.verifier.clone();
        result.publish(&selected, PublishMode::Replace)?;
        Ok(result)
    }

    /// Current authenticated prefix; this accessor does not imply a fresh committee observation.
    pub fn verifier(&self) -> &FinalityVerifier {
        &self.verifier
    }

    /// Independently installed network label retained by this runtime context.
    pub fn network_name(&self) -> &str {
        &self.network_name
    }

    /// Verify one bounded page and durably retain progress without claiming current readiness.
    ///
    /// # Errors
    /// Native verification, source, bounded-work, custody or atomic publication failure.
    pub fn catch_up<S: FinalitySource + ?Sized>(
        &mut self,
        source: &S,
        target: NonZeroU64,
    ) -> Result<u64> {
        self.revalidate()?;
        let mut next = self.verifier.clone();
        let height = next.catch_up(source, target)?;
        self.publish(&next, PublishMode::Replace)?;
        self.verifier = next;
        Ok(height)
    }

    /// Advance to a certificate-verified parent state through one public endpoint.
    /// This establishes a certified historical cut for registration and inclusion proofs;
    /// it does not assert fresh quorum readiness. Ledger execution still enforces the
    /// current owner, lease generation and exact signed operation.
    ///
    /// # Errors
    /// Invalid certificates, a regressed tip, bounded-read or durable-publication failure.
    pub fn refresh_certified<S: FinalitySource + ?Sized>(&mut self, source: &S) -> Result<u64> {
        self.revalidate()?;
        let member = self.verifier.checkpoint().tip().committee.first()
            .ok_or(BootstrapError::Invalid("parent checkpoint has no committee"))?;
        let peer = iroha_model_base::peer::PeerId::new(member.public_key.clone());
        let proof = source.latest_finality_proof(&peer, &rand::random())
            .map_err(|_| BootstrapError::Invalid("cannot retrieve parent certified tip"))?;
        let mut next = self.verifier.clone();
        next.advance(source, &proof)?;
        self.publish(&next, PublishMode::Replace)?;
        self.verifier = next;
        Ok(proof.height())
    }

    /// Require fresh exact committee quorum, then durably publish before exposing readiness.
    /// `CatchingUp` durably retains only its native-verified certified prefix and remains an error;
    /// every other failure leaves the retained prefix unchanged. Neither that progress nor an old
    /// successful observation is reusable as a fresh readiness claim after reopening.
    ///
    /// # Errors
    /// Missing quorum, invalid challenge/chain, transport, resource or custody failure.
    pub fn observe<S: FinalitySource + ?Sized>(
        &mut self,
        source: &S,
        challenge: &[u8; 32],
    ) -> Result<AttestationQuorum> {
        self.observe_using(|next| next.observe(source, challenge))
    }

    fn observe_using(
        &mut self,
        observe: impl FnOnce(
            &mut FinalityVerifier,
        ) -> std::result::Result<AttestationQuorum, FinalityError>,
    ) -> Result<AttestationQuorum> {
        self.revalidate()?;
        let mut next = self.verifier.clone();
        let result = observe(&mut next);
        match &result {
            Ok(_) => {}
            Err(FinalityError::CatchingUp { .. }) => {
                if !next.promote_verified_progress() {
                    return Err(BootstrapError::Invalid(
                        "catch-up observation lacks a verified successor prefix",
                    ));
                }
            }
            Err(_) => return result.map_err(BootstrapError::from),
        }
        self.publish(&next, PublishMode::Replace)?;
        self.verifier = next;
        result.map_err(BootstrapError::from)
    }

    /// Exercise the same publication owner with a smaller verifier work budget in unit fixtures.
    #[cfg(test)]
    pub(crate) fn observe_for_testing(
        &mut self,
        observe: impl FnOnce(
            &mut FinalityVerifier,
        ) -> std::result::Result<AttestationQuorum, FinalityError>,
    ) -> Result<AttestationQuorum> {
        self.observe_using(observe)
    }

    fn revalidate(&self) -> Result<()> {
        if self.publication_uncertain {
            return Err(BootstrapError::Invalid(
                "parent checkpoint publication is uncertain; reopen custody",
            ));
        }
        self.directory.revalidate()?;
        if FileIdentity::of(&self.directory.open_read("lock")?)? != FileIdentity::of(&self.lock)? {
            return Err(BootstrapError::Invalid("parent finality lock was replaced"));
        }
        Ok(())
    }

    fn publish(&mut self, verifier: &FinalityVerifier, mode: PublishMode) -> Result<()> {
        self.revalidate()?;
        let bytes = record_bytes(&self.network_name, verifier)?;
        if let Err(error) = self.directory.write_atomic("verified.nrt", &bytes, mode) {
            self.publication_uncertain = true;
            return Err(error.into());
        }
        Ok(())
    }
}

fn record_bytes(
    network_name: &str,
    verifier: &FinalityVerifier,
) -> Result<Vec<u8>> {
    let record = Record {
        network_name: network_name.to_owned(),
        checkpoint: verifier
            .checkpoint()
            .encode_canonical()
            .map_err(|_| BootstrapError::Invalid("cannot encode parent checkpoint"))?,
    };
    let bytes = norito::encode_canonical(&record)
        .map_err(|_| BootstrapError::Invalid("cannot encode retained parent record"))?;
    if bytes.len() > MAX_BOOTSTRAP_BYTES {
        return Err(BootstrapError::Invalid(
            "retained parent checkpoint exceeds byte bound",
        ));
    }
    // Publication must fit the exact reopen decoder, including its nested archive limits.
    // A refusal here has not attempted filesystem publication and does not poison custody.
    let _: Record = decode(&bytes, MAX_BOOTSTRAP_BYTES)?;
    Ok(bytes)
}
