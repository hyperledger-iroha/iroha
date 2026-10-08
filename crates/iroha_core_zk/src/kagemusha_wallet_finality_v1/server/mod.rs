//! Server-only ordinary-finality production selected by the signed artifact graph.
//!
//! Wallet installations remain verifier-only. This owner qualifies the complete fixed
//! descriptor/verifier graph before retaining a proving topology. Original server keys
//! are loaded or exactly regenerated and strictly imported at each actual use. Neither
//! graph construction, cached bytes nor native block certificates are completed Load proofs. Checkpoint restoration verifies the complete
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
    catalog::ArtifactRecord,
    continuity::{
        SourceNodeEvidence,
        tree::{NodeRandomness, SourceIdentity},
    },
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
mod cache;
mod custody;
mod randomness;
mod storage;
#[cfg(test)]
mod tests;

/// Closed failure of original server installation, DATA custody or genuine proving.
#[derive(Debug, thiserror::Error)]
pub enum ServerFinalityErrorV1 {
    /// A configured finite bound, independent installation or exact original differs.
    #[error("server finality installation or source binding differs")]
    Binding,
    /// Complete compiled D/V graph reconstruction refused identity, bounds or cancellation.
    #[error(transparent)]
    Catalog(#[from] iroha_kagemusha_proof::finality::catalog::CompileError),
    /// Existing immutable proof custody is unavailable or changed.
    #[error(transparent)]
    Storage(#[from] std::io::Error),
    /// OS entropy was temporarily unavailable or bounded salt sampling exhausted.
    /// This is a retryable resource refusal, not an invalid-proof verdict.
    #[error("server finality entropy unavailable")]
    EntropyUnavailable,
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
impl ServerFinalityErrorV1 {
    /// Whether the operation ended due to typed cooperative cancellation.
    pub fn is_cancelled(&self) -> bool {
        match self {
            Self::Proof(error) => error.is_cancelled(),
            Self::Catalog(error) => error.is_cancelled(),
            _ => false,
        }
    }
}

type Result<T> = std::result::Result<T, ServerFinalityErrorV1>;

/// Explicit server resource ceilings. These are limits, never trusted proof inputs.
#[derive(Clone, Copy, Debug)]
pub struct ServerFinalityLimitsV1 {
    /// Maximum one original server proving key, up to 1 GiB.
    pub maximum_key_bytes: usize,
    /// Maximum resident cache PK and interrupted PK staging bytes; 512 MiB by default.
    /// Must hold one maximum-sized key and be finite at or below 16 GiB.
    pub maximum_resident_proving_key_bytes: usize,
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
            || self.maximum_resident_proving_key_bytes < self.maximum_key_bytes
            || self.maximum_resident_proving_key_bytes == usize::MAX
            || self.maximum_resident_proving_key_bytes as u64 > 16u64 << 30
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

/// Reimport a complete offline compiler archive through the shared immutable byte reader.
/// Records are unadmitted compiler DATA; success returns only the terminal source identity.
/// It does not create a runtime, authenticate a release, sign an inventory or prove a Load.
/// The serving owner separately requires its independently selected signed verifier pack
/// and producer inventory. No key generation, fallback or repair occurs here.
/// # Errors
/// Wrong original identity/extent, incomplete graph, native genesis, source import or limits.
pub fn qualify_server_archive(
    verifier: &SumeragiFinalityVerifier,
    records: &[ArtifactRecord],
    path: &Path,
    limits: ImportLimits,
) -> Result<SourceIdentity> {
    let imports = limits;
    let anchor = derive_history_anchor(verifier).map_err(|_| ServerFinalityErrorV1::Binding)?;
    let mut originals = artifacts::ArchiveOriginals::from_records(records, path, imports)?;
    let installed = InstalledFinality::from_original_artifacts(
        anchor,
        &mut originals,
        Parameters {
            pallas: PinnedParams::derive(16).map_err(|_| ServerFinalityErrorV1::Binding)?,
            vesta: PinnedParams::derive(16).map_err(|_| ServerFinalityErrorV1::Binding)?,
        },
        imports,
    )
    .map_err(|error| {
        originals
            .take_failure()
            .unwrap_or(ServerFinalityErrorV1::Proof(error))
    })?;
    originals.require_complete()?;
    let source = installed.qualified_source();
    Ok(SourceIdentity {
        descriptor: *source.binding().digest(),
        key: source
            .key_digest()
            .map_err(|_| ServerFinalityErrorV1::Binding)?
            .to_repr(),
    })
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
    parameters: [[u8; 32]; 2],
    chain: String,
}

/// Distinct existing private namespaces for immutable inputs, regenerable PKs and proofs.
/// Initialization never adopts a populated cache or journal. Normal opening never creates
/// missing selection records, directories or ownership locks.
#[derive(Clone, Copy)]
pub struct ServerFinalityStorageV1<'a> {
    /// Independently authenticated content-addressed descriptor and verifying-key originals.
    pub verifier_originals: &'a Path,
    /// Exclusively owned regenerable proving-key cache, separate from proof custody.
    pub proving_cache: &'a Path,
    /// Exclusively owned durable proof/checkpoint originals.
    pub journal: &'a Path,
}
struct MountStorage<'a> {
    paths: ServerFinalityStorageV1<'a>,
    mode: custody::Mode,
}

/// One server installation and exclusively owned immutable recovery journal.
/// Construction authenticates the complete fixed descriptor/verifier graph.
/// Selected proving originals are regenerated or read and strictly imported only
/// when used; successful construction is not a claim that all proving keys are ready.
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
    /// Initialize fresh empty private cache and journal namespaces after full authenticated
    /// D/V reconstruction. No PK is generated during initialization. Every actual proof
    /// loads or regenerates exact signed PK bytes and strictly imports its selected source.
    /// # Errors
    /// Invalid installation, populated namespace, custody/resource/source failure or cancellation.
    pub fn initialize(
        verifier: &SumeragiFinalityVerifier,
        installation: InstallationV1,
        verifier_pack: &[u8],
        producer_inventory: &[u8],
        storage: ServerFinalityStorageV1<'_>,
        limits: ServerFinalityLimitsV1,
        cancellation: ServerFinalityCancellationV1,
    ) -> Result<Self> {
        Self::mount(
            verifier,
            installation,
            verifier_pack,
            producer_inventory,
            MountStorage {
                paths: storage,
                mode: custody::Mode::Initialize,
            },
            limits,
            cancellation,
        )
    }

    /// Reopen existing exact selections; missing cache/journal selections never initialize.
    /// Proving tables may be regenerated only at actual use from fully matched compiled
    /// recipes, followed by strict source import. Opening grants no all-PK-ready verdict.
    /// # Errors
    /// Invalid installation, lost/substituted custody, source/resource failure or cancellation.
    pub fn open(
        verifier: &SumeragiFinalityVerifier,
        installation: InstallationV1,
        verifier_pack: &[u8],
        producer_inventory: &[u8],
        storage: ServerFinalityStorageV1<'_>,
        limits: ServerFinalityLimitsV1,
        cancellation: ServerFinalityCancellationV1,
    ) -> Result<Self> {
        Self::mount(
            verifier,
            installation,
            verifier_pack,
            producer_inventory,
            MountStorage {
                paths: storage,
                mode: custody::Mode::Open,
            },
            limits,
            cancellation,
        )
    }

    fn mount(
        verifier: &SumeragiFinalityVerifier,
        installation: InstallationV1,
        verifier_pack: &[u8],
        producer_inventory: &[u8],
        storage: MountStorage<'_>,
        limits: ServerFinalityLimitsV1,
        cancellation: ServerFinalityCancellationV1,
    ) -> Result<Self> {
        let imports = limits.imports()?;
        iroha_pasta::CancellationToken::checkpoint(Some(&cancellation.0))
            .map_err(|_| iroha_kagemusha_proof::finality::continuity::producer::Error::Cancelled)?;
        let paths = storage.paths;
        let roots = [paths.verifier_originals, paths.proving_cache, paths.journal]
            .map(iroha_fs::PrivateDirectory::open_exact);
        let [verifiers, cache_root, journal_root] = roots;
        let roots = [verifiers?, cache_root?, journal_root?];
        let identities = [
            roots[0].identity()?,
            roots[1].identity()?,
            roots[2].identity()?,
        ];
        if identities[0] == identities[1]
            || identities[0] == identities[2]
            || identities[1] == identities[2]
        {
            return Err(ServerFinalityErrorV1::Binding);
        }
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
        let params = Parameters {
            pallas: pack.verifier().pallas_parameters().as_ref().clone(),
            vesta: pack
                .verifier()
                .vesta_parameters(16)
                .map_err(|_| ServerFinalityErrorV1::Binding)?
                .as_ref()
                .clone(),
        };
        let mut verifier_originals = artifacts::VerifierOriginals::open(
            paths.verifier_originals,
            &selected.originals,
            cancellation.0.clone(),
        )?;
        let recipes = iroha_kagemusha_proof::finality::catalog::qualify_server_recipes(
            anchor,
            &selected.originals,
            &mut verifier_originals,
            params.clone(),
            iroha_kagemusha_proof::finality::catalog::VerifierLimits {
                maximum_artifacts: limits.maximum_artifacts,
                maximum_verifier_bytes: 512 << 20,
                msm_budget: MemoryBudget::new(limits.msm_bytes),
            },
            imports,
            Some(&cancellation.0),
        )
        .map_err(|error| {
            verifier_originals
                .take_failure()
                .unwrap_or(ServerFinalityErrorV1::Catalog(error))
        })?;
        // Complete D/V qualification fixes all children and obligations. No PK is
        // opened here; every proof path strictly imports its exact signed originals.
        let installed = recipes.installed_graph(Some(&cancellation.0))?;
        let selection = norito::to_bytes(&Selection {
            version: 1,
            scheme: installation.scheme_id,
            manifest: installation.manifest_digest,
            verifier_pack_sha256: Sha256::digest(verifier_pack).into(),
            producer_inventory_sha256: Sha256::digest(producer_inventory).into(),
            anchor: anchor.digest().to_repr(),
            parameters: [params.pallas.digest(), params.vesta.digest()],
            chain: chain.clone(),
        })
        .map_err(|_| ServerFinalityErrorV1::Binding)?;
        // Namespaces are selected only after complete D/V closure. They bind their role
        // as well as installation/anchor/parameter identity, preventing cache/journal swaps.
        let mut cache_selection = b"kagemusha-server-proving-cache-v1".to_vec();
        cache_selection.extend_from_slice(&selection);
        let mut journal_selection = b"kagemusha-server-proof-journal-v1".to_vec();
        journal_selection.extend_from_slice(&selection);
        for root in &roots {
            root.revalidate()?;
        }
        let cache = cache::Cache::acquire(
            paths.proving_cache,
            &cache_selection,
            &selected.originals,
            limits.maximum_key_bytes,
            limits.maximum_resident_proving_key_bytes,
            storage.mode,
        )?;
        let journal = storage::Journal::acquire(
            paths.journal,
            &journal_selection,
            limits.maximum_journal_entries,
            limits.maximum_journal_bytes,
            storage.mode,
        )?;
        for root in &roots {
            root.revalidate()?;
        }
        let originals = artifacts::Originals::new(
            verifier_originals,
            selected.originals.clone(),
            recipes,
            cache,
        );
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
        let mut entropy = randomness::NodeEntropy::new(&self.cancellation.0);
        let mut randomness =
            |_| entropy.draw(crate::kagemusha_wallet_proofs_v1::randomness::os_entropy);
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
        let result = operation(&self.installed, &mut context);
        drop(context);
        entropy.finish(result, || self.originals.take_failure())
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
