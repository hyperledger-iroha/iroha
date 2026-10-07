//! Server-only ordinary-finality production selected by the signed artifact graph.
//!
//! Wallet installations remain verifier-only. This owner separately mounts the original
//! server proving tables and never accepts a receipt, cached byte string or native block
//! certificate as a completed Load proof. Checkpoint restoration verifies the complete
//! source statement and both curve obligations before reusing work.

use std::path::Path;

use ff::{Field as _, PrimeField as _};
use iroha_crypto::MerkleProof;
use iroha_data_model::{
    events::EventBox,
    isi::kagemusha_wallet::KagemushaWalletLoadReceiptV1,
    kagemusha::{KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1, KagemushaWalletLoadFinalityV1},
    sumeragi_finality::{SumeragiFinalityVerifier, VerifiedSumeragiBlock},
};
use iroha_kagemusha_proof::finality::{
    continuity::{SourceNodeEvidence, tree::NodeRandomness},
    native::{HistoryPrefix, ImportLimits, InstalledFinality, Parameters, ProvingContext},
    receipt_finality::{CONTEXT_DOMAIN, PROGRAM_ID},
};
use iroha_pasta::{Ep, Eq, Fp, Fq, msm::MemoryBudget, poseidon::hash_with_domain};
use iroha_plonk::{
    ProverConfig, ProverRandomness,
    keys::{CosetCachePolicy, pk::artifact::ReadConfig},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_recursion::{AccumulatorT, FoldConfig};
use norito::{Decode, Encode, NoritoSchema};
use sha2::{Digest as _, Sha256};

use super::{block_witness, derive_history_anchor, load_witness, retain_load_finality};
use crate::kagemusha_wallet_artifacts_v1::{InstallationV1, InstalledVerifierPackV1};

mod artifacts;
mod storage;
#[cfg(test)]
mod tests;

/// Closed failure of original server installation, DATA custody or genuine proving.
#[derive(Debug, thiserror::Error)]
pub enum ServerFinalityErrorV1 {
    /// A configured finite bound, independent installation or exact original differs.
    #[error("server finality installation or source binding differs")]
    Binding,
    /// Existing immutable proof custody is unavailable or changed.
    #[error(transparent)]
    Storage(#[from] std::io::Error),
    /// The complete installed native proof graph refused its input or resources.
    #[error(transparent)]
    Proof(#[from] iroha_kagemusha_proof::finality::continuity::producer::Error),
    /// The original independently verified native block/event could not be bound.
    #[error(transparent)]
    Input(#[from] super::FinalityInputError),
    /// The terminal proof failed exact receipt/claim retention checks.
    #[error(transparent)]
    Retention(#[from] super::FinalityRetentionError),
}
type Result<T> = std::result::Result<T, ServerFinalityErrorV1>;

/// Explicit server resource ceilings. These are limits, never trusted proof inputs.
#[derive(Clone, Copy, Debug)]
pub struct ServerFinalityLimitsV1 {
    /// Maximum one original server proving key, up to 1 GiB.
    pub maximum_key_bytes: usize,
    /// Maximum aggregate original graph extent, including all D/V/PK entries.
    pub maximum_original_bytes: usize,
    /// Maximum source graph entries.
    pub maximum_artifacts: usize,
    /// Scratch ceiling of each real MSM/proof operation.
    pub msm_bytes: usize,
    /// Maximum journal files including immutable originals, partials and lock.
    pub maximum_journal_entries: usize,
    /// Maximum actual bytes of all journal files, including interrupted partials.
    pub maximum_journal_bytes: u64,
}
impl ServerFinalityLimitsV1 {
    fn imports(self) -> Result<ImportLimits> {
        if !(1..=1 << 30).contains(&self.maximum_key_bytes)
            || self.maximum_original_bytes == 0
            || self.maximum_original_bytes == usize::MAX
            || !(1..=65_536).contains(&self.maximum_artifacts)
            || !(1 << 20..=1 << 30).contains(&self.msm_bytes)
            || !(3..=1_000_000).contains(&self.maximum_journal_entries)
            || self.maximum_journal_bytes == 0
            || self.maximum_journal_bytes == u64::MAX
        {
            return Err(ServerFinalityErrorV1::Binding);
        }
        Ok(ImportLimits {
            key: ReadConfig {
                maximum_bytes: self.maximum_key_bytes,
                maximum_rows: 1 << 16,
                coset_cache: CosetCachePolicy::OnDemand,
                msm_budget: MemoryBudget::new(self.msm_bytes),
            },
            maximum_artifacts: self.maximum_artifacts,
            maximum_original_bytes: self.maximum_original_bytes,
        })
    }
}

/// Cooperative cancellation of genuine proof work; contains no proof authority.
#[derive(Clone, Default)]
pub struct ServerFinalityCancellationV1(iroha_pasta::CancellationToken);
impl ServerFinalityCancellationV1 {
    /// Signal permanent cancellation. Previously published exact work remains retained.
    pub fn cancel(&self) {
        self.0.cancel();
    }
}

#[derive(Encode, Decode, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.server.finality_selection.v1")]
struct Selection {
    version: u8,
    scheme: [u8; 32],
    manifest: [u8; 32],
    verifier_pack_sha256: [u8; 32],
    producer_inventory_sha256: [u8; 32],
    anchor: [u8; 32],
    chain: String,
}

/// One server installation and exclusively owned immutable recovery journal.
/// Construction always authenticates and imports the full selected original graph.
pub struct ServerFinalityV1 {
    installed: InstalledFinality,
    originals: artifacts::Originals,
    journal: storage::Journal,
    chain: String,
    scheme: [u8; 32],
    budget: MemoryBudget,
    cancellation: ServerFinalityCancellationV1,
}
impl ServerFinalityV1 {
    /// Mount real server proving originals under independently selected signed installation
    /// and actual native genesis. All paths must be existing exact owner-private directories.
    /// No keys, directories, receipt authority or fallback profile are generated here.
    /// # Errors
    /// Refuses changed signatures/source graph/genesis, missing PKs, competing ownership,
    /// incompatible recovery selection and exhausted finite resource limits.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        verifier: &SumeragiFinalityVerifier,
        installation: InstallationV1,
        verifier_pack: &[u8],
        producer_inventory: &[u8],
        server_originals: &Path,
        journal: &Path,
        limits: ServerFinalityLimitsV1,
        cancellation: ServerFinalityCancellationV1,
    ) -> Result<Self> {
        let imports = limits.imports()?;
        let anchor = derive_history_anchor(verifier).map_err(|_| ServerFinalityErrorV1::Binding)?;
        let chain = verifier.chain_id().to_owned();
        if chain.is_empty() || chain.len() > 1024 {
            return Err(ServerFinalityErrorV1::Binding);
        }
        let pack = InstalledVerifierPackV1::load(verifier_pack, installation)
            .map_err(|_| ServerFinalityErrorV1::Binding)?;
        let inventory = pack
            .authenticate_producer_inventory(producer_inventory)
            .map_err(|_| ServerFinalityErrorV1::Binding)?;
        let selected = &inventory.inventory().finality;
        if (
            selected.network,
            selected.instance,
            selected.initial_context,
            selected.initial_epoch,
            selected.parameters,
        ) != (
            anchor.network,
            anchor.instance,
            anchor.initial_context,
            anchor.initial_epoch,
            anchor.parameters,
        ) {
            return Err(ServerFinalityErrorV1::Binding);
        }
        let mut originals =
            artifacts::Originals::open(&inventory, server_originals, limits.maximum_key_bytes)?;
        let params = Parameters {
            pallas: PinnedParams::derive(16).map_err(|_| ServerFinalityErrorV1::Binding)?,
            vesta: PinnedParams::derive(16).map_err(|_| ServerFinalityErrorV1::Binding)?,
        };
        let selection = norito::to_bytes(&Selection {
            version: 1,
            scheme: installation.scheme_id,
            manifest: installation.manifest_digest,
            verifier_pack_sha256: Sha256::digest(verifier_pack).into(),
            producer_inventory_sha256: Sha256::digest(producer_inventory).into(),
            anchor: anchor.digest().to_repr(),
            chain: chain.clone(),
        })
        .map_err(|_| ServerFinalityErrorV1::Binding)?;
        let journal = storage::Journal::open(
            journal,
            &selection,
            limits.maximum_journal_entries,
            limits.maximum_journal_bytes,
        )?;
        iroha_pasta::CancellationToken::checkpoint(Some(&cancellation.0))
            .map_err(|_| ServerFinalityErrorV1::Binding)?;
        let installed =
            InstalledFinality::from_original_artifacts(anchor, &mut originals, params, imports)?;
        Ok(Self {
            installed,
            originals,
            journal,
            chain,
            scheme: installation.scheme_id,
            budget: MemoryBudget::new(limits.msm_bytes),
            cancellation,
        })
    }

    fn with_context<T>(
        &mut self,
        operation: impl FnOnce(
            &InstalledFinality,
            &mut ProvingContext<'_, '_>,
        ) -> std::result::Result<
            T,
            iroha_kagemusha_proof::finality::continuity::producer::Error,
        >,
    ) -> Result<T> {
        let mut randomness = |_| {
            Ok(NodeRandomness {
                inner_salt: Fp::random(rand_core_06::OsRng),
                outer_salt: Fq::random(rand_core_06::OsRng).to_repr(),
                source: ProverRandomness::hedged(),
                wrapper: ProverRandomness::hedged(),
            })
        };
        let fold = FoldConfig {
            kernel_budget: self.budget,
            cancellation: Some(self.cancellation.0.clone()),
            ..FoldConfig::default()
        };
        let mut context = ProvingContext::new(
            &mut self.originals,
            &mut randomness,
            ProverConfig {
                msm_budget: self.budget,
                cancellation: Some(&self.cancellation.0),
            },
            &fold,
        )
        .with_checkpoints(&mut self.journal);
        Ok(operation(&self.installed, &mut context)?)
    }

    /// Start or fully reverify the exact immutable genesis proof checkpoint.
    /// # Errors
    /// Original custody, compiled source, proof, claim, cancellation or resource refusal.
    pub fn genesis(&mut self) -> Result<HistoryPrefix> {
        self.with_context(|installed, context| installed.genesis(context))
    }

    /// Prove the next actual native block; the original prefix is unchanged on failure.
    /// # Errors
    /// Noncontiguous/foreign block, source/custody change or genuine proof refusal.
    pub fn append(
        &mut self,
        previous: &HistoryPrefix,
        block: &VerifiedSumeragiBlock,
    ) -> Result<HistoryPrefix> {
        let witness = block_witness(self.installed.anchor(), &self.chain, block)?;
        self.with_context(|installed, context| installed.append_block(previous, &witness, context))
    }

    /// Read and reverify an exact previously completed terminal proof for this receipt.
    /// Absence alone returns `None`; malformed or unavailable custody is an error.
    /// The caller must still authenticate the receipt's current payer/ledger ownership.
    /// # Errors
    /// Changed receipt, installed graph, proof/claims, provenance or retained originals.
    pub fn retained(&self, receipt: &KagemushaWalletLoadReceiptV1) -> Result<Option<Vec<u8>>> {
        let digest = receipt
            .receipt_digest()
            .map_err(|_| ServerFinalityErrorV1::Binding)?;
        if receipt.scheme_id != self.scheme {
            return Err(ServerFinalityErrorV1::Binding);
        }
        let Some(bytes) = self.journal.read(
            &format!("receipt-{}", hex::encode(digest)),
            KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1,
        )?
        else {
            return Ok(None);
        };
        self.verify_retained(receipt, &bytes)?;
        Ok(Some(bytes))
    }

    fn verify_retained(&self, receipt: &KagemushaWalletLoadReceiptV1, bytes: &[u8]) -> Result<()> {
        let retained = KagemushaWalletLoadFinalityV1::decode_canonical(bytes)
            .map_err(|_| ServerFinalityErrorV1::Binding)?;
        let digest = receipt
            .receipt_digest()
            .map_err(|_| ServerFinalityErrorV1::Binding)?;
        if retained.receipt_digest != digest
            || retained.anchor_digest != self.installed.anchor().digest().to_repr()
        {
            return Err(ServerFinalityErrorV1::Binding);
        }
        let digest =
            Option::<Fp>::from(Fp::from_repr(digest)).ok_or(ServerFinalityErrorV1::Binding)?;
        let context = hash_with_domain(CONTEXT_DOMAIN, &[self.installed.anchor().digest(), digest]);
        let evidence = SourceNodeEvidence {
            endpoints: [
                Fp::from(PROGRAM_ID),
                context,
                Fp::ZERO,
                Fp::ONE,
                Fp::ZERO,
                context,
            ],
            proof: retained.proof,
            pallas: AccumulatorT::<Ep>::from_bytes(&retained.pallas_claim)
                .map_err(|_| ServerFinalityErrorV1::Binding)?,
            vesta: AccumulatorT::<Eq>::from_bytes(&retained.vesta_claim)
                .map_err(|_| ServerFinalityErrorV1::Binding)?,
        };
        self.installed
            .verify_receipt_evidence(digest, &evidence, self.budget)?;
        Ok(())
    }

    /// Complete the exact counted native Load event under its complete history proof.
    /// Reverify before immutable publication; retries return the original exact bytes.
    /// # Errors
    /// Wrong payer-bound receipt/event/block, source graph, proof/claims or journal custody.
    pub fn prove(
        &mut self,
        prefix: &HistoryPrefix,
        block: &VerifiedSumeragiBlock,
        receipt: &KagemushaWalletLoadReceiptV1,
        path: &MerkleProof<EventBox>,
    ) -> Result<Vec<u8>> {
        if let Some(bytes) = self.retained(receipt)? {
            return Ok(bytes);
        }
        let witness = load_witness(self.installed.anchor(), &self.chain, block, receipt, path)?;
        let evidence = self.with_context(|installed, context| {
            installed.prove_receipt(prefix, &witness, context)
        })?;
        let retained = retain_load_finality(&self.installed, receipt, evidence, self.budget)?;
        let bytes = retained
            .to_canonical_bytes()
            .map_err(|_| ServerFinalityErrorV1::Binding)?;
        self.verify_retained(receipt, &bytes)?;
        self.journal.put(
            &format!("receipt-{}", hex::encode(retained.receipt_digest)),
            &bytes,
        )?;
        Ok(bytes)
    }
}
