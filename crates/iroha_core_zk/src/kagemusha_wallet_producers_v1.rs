//! Complete source-bound native producer catalog under the installed signed Omega VK.
//!
//! The verifier pack supplies the independently authenticated sixteen sigma originals
//! and final Omega original. Every sigma, Q, A and W PK is imported against its actual
//! typed compiled source and exact VK; the final Omega PK then binds the complete,
//! immutable terminal catalog transitively to that installed authority. There is no
//! additional PK signature format, runtime key generation, profile fallback, foreign
//! identifier admission or readiness flag. This owner does not mutate custody.
//!
//! Source shapes and layouts are installation metadata selected once by released native
//! code. Actual operation witnesses cannot select them. Ordinary4 A profiles remain
//! unchanged. Complete original pack, capacity, physical memory/ABI and device
//! qualification remain required; verifier inventory alone cannot construct this owner.

use std::collections::BTreeMap;

use iroha_data_model::kagemusha::kagemusha_wallet_v1::*;
use iroha_kagemusha_proof::{
    PrefixMode, RelationShape, SigmaParams, SigmaProver, SigmaRelation, SigmaShape,
    a_relation::{
        AProofPlan, QProofPlan,
        bootstrap::BootstrapPolicy,
        native::{archive, bootstrap, load, receive, refresh, retiring, send, unload},
        own::OwnPolicy,
        receive::ReceiveStagePlan,
    },
    admin_sigma::native::{
        ArchiveProver, BootstrapProver, LoadProver, RefreshProver, RetiringProver, UnloadProver,
    },
    omega::native as final_omega,
    q_sigma::{
        QSigmaPlan, SigmaClass,
        native::{QSigmaProver, QSigmaSource},
    },
    q_signature::{QSignaturePlan, SignatureKey, SignatureSlot, native::QSignatureProver},
};
use iroha_pasta::{Ep, Eq};
use iroha_plonk::{
    DescriptorBinding, Protocol, VerifyingKey, keys::pk::artifact::ReadConfig,
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::p256::{VerifyMode, native::Affine};
use iroha_plonk_recursion::{obligation::ledger::Variant, verifier::VerifierPlan};
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize};

use crate::kagemusha_wallet_artifacts_v1::{
    DESCRIPTOR_MAX_BYTES_V1, InstalledVerifierPackV1, SIGMA_CATALOG_V1, VERIFYING_KEY_MAX_BYTES_V1,
};

/// Exact required semantic source count, before identical terminal VK deduplication.
pub const PRODUCER_CLASS_COUNT_V1: usize = 24;

/// Complete operation class. This is installation metadata, never an operation selector
/// accepted from a received proof or a foreign wallet-open request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.wallet.producer_class.v1")]
pub enum ProducerClassV1 {
    /// Unique initial enrolled zero-state source.
    Bootstrap,
    /// Authentic finalized load source.
    Load,
    /// One of all eight fixed Send control-mask sources.
    Send {
        /// Exact complete mask, in 0..=7.
        controls: u8,
    },
    /// Both recorded-list choices and both receiver credential conventions.
    Receive {
        /// Request-recorded blacklist selector.
        blacklist: bool,
        /// Whether the current credential differs from the quoted credential.
        renewed: bool,
    },
    /// Authentic beneficiary-bound irreversible unload source.
    Unload,
    /// Credited Receive proof, for each recorded Request blacklist selector.
    ArchiveReceive {
        /// Request-recorded Receive blacklist selector.
        blacklist: bool,
    },
    /// Credited folded-status evidence source.
    ArchiveStatus,
    /// Authentic lifecycle transition source.
    Retiring,
    /// Credential replacement under the one fixed Refresh sigma union.
    RefreshCredential,
    /// Scheme policy replacement under the one fixed Refresh sigma union.
    RefreshSchemePolicy,
    /// Blacklist history insertion under the one fixed Refresh sigma union.
    RefreshBlacklist,
    /// Complete quota rebuild under the one fixed Refresh sigma union.
    RefreshQuotaShare,
    /// Time anchor replacement under the one fixed Refresh sigma union.
    RefreshTimeAnchor,
}

/// Frozen full semantic order used by the source-key catalog. Extras, omissions,
/// duplicates and reordered classes are rejected before importing any original key.
pub const PRODUCER_CLASSES_V1: [ProducerClassV1; PRODUCER_CLASS_COUNT_V1] = [
    ProducerClassV1::Bootstrap,
    ProducerClassV1::Load,
    ProducerClassV1::Send { controls: 0 },
    ProducerClassV1::Send { controls: 1 },
    ProducerClassV1::Send { controls: 2 },
    ProducerClassV1::Send { controls: 3 },
    ProducerClassV1::Send { controls: 4 },
    ProducerClassV1::Send { controls: 5 },
    ProducerClassV1::Send { controls: 6 },
    ProducerClassV1::Send { controls: 7 },
    ProducerClassV1::Receive {
        blacklist: false,
        renewed: false,
    },
    ProducerClassV1::Receive {
        blacklist: true,
        renewed: false,
    },
    ProducerClassV1::Receive {
        blacklist: false,
        renewed: true,
    },
    ProducerClassV1::Receive {
        blacklist: true,
        renewed: true,
    },
    ProducerClassV1::Unload,
    ProducerClassV1::ArchiveReceive { blacklist: false },
    ProducerClassV1::ArchiveReceive { blacklist: true },
    ProducerClassV1::ArchiveStatus,
    ProducerClassV1::Retiring,
    ProducerClassV1::RefreshCredential,
    ProducerClassV1::RefreshSchemePolicy,
    ProducerClassV1::RefreshBlacklist,
    ProducerClassV1::RefreshQuotaShare,
    ProducerClassV1::RefreshTimeAnchor,
];

impl ProducerClassV1 {
    fn variant(self) -> Variant {
        match self {
            Self::Bootstrap => Variant::Bootstrap,
            Self::Load => Variant::Load,
            Self::Send { .. } => Variant::Send,
            Self::Receive { renewed: false, .. } => Variant::Receive,
            Self::Receive { renewed: true, .. } => Variant::ReceiveRenewed,
            Self::Unload => Variant::Unload,
            Self::ArchiveReceive { .. } => Variant::ArchiveReceive,
            Self::ArchiveStatus => Variant::ArchiveStatus,
            Self::Retiring => Variant::Retiring,
            Self::RefreshCredential => Variant::RefreshCredential,
            Self::RefreshSchemePolicy => Variant::RefreshSchemePolicy,
            Self::RefreshBlacklist => Variant::RefreshBlacklist,
            Self::RefreshQuotaShare => Variant::RefreshQuotaShare,
            Self::RefreshTimeAnchor => Variant::RefreshTimeAnchor,
        }
    }
    fn sigma_index(self) -> usize {
        match self {
            Self::Bootstrap => 0,
            Self::Load => 1,
            Self::Send { controls } => 2 + usize::from(controls),
            Self::Receive { blacklist, .. } => 10 + usize::from(blacklist),
            Self::ArchiveReceive { .. } | Self::ArchiveStatus => 12,
            Self::Unload => 13,
            Self::Retiring => 15,
            _ => 14,
        }
    }
    fn stage_count(self) -> usize {
        match self {
            Self::Bootstrap => 2,
            Self::Send { .. } => 5,
            Self::Receive { .. } => 11,
            _ => 4,
        }
    }
    fn signature_count(self) -> usize {
        match self {
            Self::Bootstrap | Self::Send { .. } | Self::Unload | Self::Retiring => 1,
            _ => 2,
        }
    }
}

/// Original descriptor, VK and PK for one Q/A/W source. Authority is rooted by
/// strict source imports and the installed final Omega VK, never by this DTO.
#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.wallet.producer_original.v1")]
pub struct ProducerOriginalV1 {
    /// Exact canonical V2 descriptor.
    pub descriptor: Vec<u8>,
    /// Exact canonical verifying-key original.
    pub verifying_key: Vec<u8>,
    /// Exact original PIPAPK01 compiled tables and commitments.
    pub proving_key: Vec<u8>,
}

/// All genuine source originals for one frozen semantic class.
#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.wallet.operation_producer_originals.v1")]
pub struct OperationOriginalsV1 {
    /// Exact class in the complete frozen order.
    pub class: ProducerClassV1,
    /// Actual Q0 proving original for the complete fixed sigma catalog.
    pub sigma_q: ProducerOriginalV1,
    /// Actual signature Q originals in fixed Q1/Q2 order.
    pub signatures: Vec<ProducerOriginalV1>,
    /// All fixed A-stage originals in order, ending in the actual terminal A.
    pub a: Vec<ProducerOriginalV1>,
    /// Every internal W original in order; there is no wrapper after terminal A.
    pub w: Vec<ProducerOriginalV1>,
}

/// Bounded native package DATA carrier. Decoding it grants no monetary authority.
#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "iroha.core_zk.kagemusha.wallet.producer_pack.v1")]
pub struct ProducerPackV1 {
    /// Exactly one; retired first-release formats have no fallback decoder.
    pub version: u16,
    /// Original sigma PKs paired with the authenticated verifier pack's exact16 order.
    pub sigma_proving_keys: Vec<Vec<u8>>,
    /// All24 genuine semantic owners in exact frozen source order.
    pub operations: Vec<OperationOriginalsV1>,
    /// Original final Omega PK under the installed signed Omega verifier original.
    pub omega_proving_key: Vec<u8>,
}

/// Fixed Sigma geometry selected once by the released native installation owner.
/// Each relation/mask is supplied by this module's frozen semantic mapping.
#[derive(Clone, Copy, Debug)]
pub struct SigmaGeometryV1 {
    /// Fixed prefix mode; never selected by a payment or witness.
    pub prefix: PrefixMode,
    /// Fixed Pow5 lane count.
    pub lanes: usize,
    /// Fixed running-sum range limb width.
    pub limb_bits: usize,
}

/// Fixed source profile for Q_sigma. Imports never try another layout on failure.
#[derive(Clone, Copy, Debug)]
pub enum QSigmaLayoutV1 {
    /// Actual ordinary source configuration.
    Ordinary,
    /// Actual serialized foreign verifier with a fixed shared range-bus count.
    SerializedForeign {
        /// Immutable count checked by the actual Q importer.
        range_buses: usize,
    },
}

/// Native installation-only source metadata. The strict final Omega PK import
/// against the trusted installed VK authenticates its complete compiled key tree.
#[derive(Clone, Debug)]
pub struct NativeSourceV1 {
    /// Fixed geometry for all eight genuine Send sources.
    pub send_sigma: SigmaGeometryV1,
    /// Fixed geometry for both genuine Receive sources.
    pub receive_sigma: SigmaGeometryV1,
    /// One fixed Q layout across this complete installation.
    pub sigma_q: QSigmaLayoutV1,
    /// One fixed final Omega program layout across this complete installation.
    pub omega: final_omega::Layout,
}

/// Explicit native installation resource limits. These do not qualify a phone's
/// total synthesis/prover heap, storage availability or physical memory budget.
#[derive(Clone, Copy, Debug)]
pub struct CatalogReadConfigV1 {
    /// Exact original key/domain/source import policy.
    pub key: ReadConfig,
    /// Complete canonical package bound, enforced before decoding.
    pub maximum_package_bytes: usize,
}

/// Admission refusal. No refusal changes custody or supplies a ready wallet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Canonical package, counts, class order or original resource bound differs.
    #[error("invalid complete native producer inventory")]
    Inventory,
    /// Scheme scope, fixed source class, descriptor, signed VK or capacity differs.
    #[error("native producer source profile differs")]
    Profile,
    /// Actual compiled-source/commitment PK import or exact VK equality failed.
    #[error("native producer original key rejected")]
    Artifact,
}

/// One actual installed typed sigma producer. Construction is private to the
/// complete loader; all sixteen original source checks are mandatory.
pub enum SigmaProducerV1 {
    /// Actual initial-state sigma leaf.
    Bootstrap(BootstrapProver),
    /// Actual authentic load sigma leaf.
    Load(LoadProver),
    /// Actual mask-specific Send relation.
    Send(SigmaProver<Eq>),
    /// Actual Request-recorded Receive relation.
    Receive(SigmaProver<Eq>),
    /// Actual archive sigma leaf.
    Archive(ArchiveProver),
    /// Actual unload sigma leaf.
    Unload(UnloadProver),
    /// One genuine five-kind Refresh sigma leaf.
    Refresh(RefreshProver),
    /// Actual retiring sigma leaf.
    Retiring(RetiringProver),
}

macro_rules! sigma_member {
    ($this:expr, $admin:ident, $($generic:tt)+) => {
        match $this {
            SigmaProducerV1::Bootstrap(p) => p.$admin(),
            SigmaProducerV1::Load(p) => p.$admin(),
            SigmaProducerV1::Send(p) | SigmaProducerV1::Receive(p) => p.$($generic)+,
            SigmaProducerV1::Archive(p) => p.$admin(),
            SigmaProducerV1::Unload(p) => p.$admin(),
            SigmaProducerV1::Refresh(p) => p.$admin(),
            SigmaProducerV1::Retiring(p) => p.$admin(),
        }
    };
}
impl SigmaProducerV1 {
    /// Exact immutable descriptor of the source-imported sigma owner.
    #[must_use]
    pub fn binding(&self) -> &DescriptorBinding {
        sigma_member!(self, binding, proving_key().binding())
    }
    /// Exact signed installed VK, retained by the imported PK.
    #[must_use]
    pub fn verifying_key(&self) -> &VerifyingKey<Eq> {
        sigma_member!(self, verifying_key, proving_key().vk())
    }
    /// Transparent pinned parameters of the immutable source class.
    #[must_use]
    pub fn params(&self) -> &PinnedParams<Eq> {
        sigma_member!(self, params, params())
    }
}

/// Genuine A/W source owner for one complete class. There is no alternate
/// signature-only monetary engine or arbitrary structural proof implementation.
pub enum OperationProducerV1 {
    /// Actual Bootstrap two-A/one-W producer.
    Bootstrap(bootstrap::Prover),
    /// Actual Load four-A/three-W producer.
    Load(load::Prover),
    /// Actual Send five-A/four-W producer.
    Send(send::Prover),
    /// Actual Receive eleven-A/ten-W producer.
    Receive(receive::Prover),
    /// Actual Unload four-A/three-W producer.
    Unload(unload::Prover),
    /// Actual Archive four-A/three-W producer.
    Archive(archive::Prover),
    /// Actual Retiring four-A/three-W producer.
    Retiring(retiring::Prover),
    /// Actual Refresh four-A/three-W producer.
    Refresh(refresh::Prover),
}

/// All exact Q and A/W typed owners for one immutable semantic source class.
pub struct InstalledOperationProducerV1 {
    class: ProducerClassV1,
    sigma_plan: QSigmaPlan,
    sigma_q: QSigmaProver,
    signatures: Vec<QSignatureProver>,
    producer: OperationProducerV1,
    terminal_key: VerifyingKey<Eq>,
    terminal_index: usize,
}
impl InstalledOperationProducerV1 {
    /// Exact frozen semantic class.
    #[must_use]
    pub const fn class(&self) -> ProducerClassV1 {
        self.class
    }
    /// Complete fixed own/incoming SigmaClass catalogs and obligation schedule.
    #[must_use]
    pub const fn sigma_plan(&self) -> &QSigmaPlan {
        &self.sigma_plan
    }
    /// Actual original-key imported Q_sigma owner.
    #[must_use]
    pub const fn sigma_q(&self) -> &QSigmaProver {
        &self.sigma_q
    }
    /// Actual hard/soft/fixed signature leaves in Q1/Q2 order.
    #[must_use]
    pub fn signatures(&self) -> &[QSignatureProver] {
        &self.signatures
    }
    /// Actual typed native A/W producer.
    #[must_use]
    pub const fn producer(&self) -> &OperationProducerV1 {
        &self.producer
    }
    /// Actual source-imported terminal A VK rooted by the complete final Omega program.
    #[must_use]
    pub const fn terminal_key(&self) -> &VerifyingKey<Eq> {
        &self.terminal_key
    }
    /// Deterministic first-occurrence index in the complete immutable Omega catalog.
    #[must_use]
    pub const fn terminal_index(&self) -> usize {
        self.terminal_index
    }
}

/// Complete typed original-key inventory. Private construction requires every
/// genuine source import and the final installed signed Omega-root equality.
pub struct InstalledProducerCatalogV1 {
    manifest: [u8; 32],
    sigma: [SigmaProducerV1; 16],
    operations: Vec<InstalledOperationProducerV1>,
    omega: final_omega::Prover,
}

fn add_size(total: &mut usize, bytes: &[u8], cap: usize, aggregate: usize) -> Result<(), Error> {
    if bytes.is_empty() || bytes.len() > cap {
        return Err(Error::Inventory);
    }
    *total = total.checked_add(bytes.len()).ok_or(Error::Inventory)?;
    if *total > aggregate {
        return Err(Error::Inventory);
    }
    Ok(())
}
fn original_bounds(
    original: &ProducerOriginalV1,
    total: &mut usize,
    config: CatalogReadConfigV1,
) -> Result<(), Error> {
    add_size(
        total,
        &original.descriptor,
        DESCRIPTOR_MAX_BYTES_V1,
        config.maximum_package_bytes,
    )?;
    add_size(
        total,
        &original.verifying_key,
        VERIFYING_KEY_MAX_BYTES_V1,
        config.maximum_package_bytes,
    )?;
    add_size(
        total,
        &original.proving_key,
        config.key.maximum_bytes,
        config.maximum_package_bytes,
    )
}
fn inventory_bounds(pack: &ProducerPackV1, config: CatalogReadConfigV1) -> Result<(), Error> {
    if config.key.maximum_rows < (1 << 16)
        || config.key.maximum_bytes == 0
        || config.maximum_package_bytes == 0
        || pack.version != 1
        || pack.sigma_proving_keys.len() != 16
        || pack.operations.len() != PRODUCER_CLASS_COUNT_V1
    {
        return Err(Error::Inventory);
    }
    let mut total = 0;
    for key in &pack.sigma_proving_keys {
        add_size(
            &mut total,
            key,
            config.key.maximum_bytes,
            config.maximum_package_bytes,
        )?;
    }
    for (original, expected) in pack.operations.iter().zip(PRODUCER_CLASSES_V1) {
        if original.class != expected
            || original.signatures.len() != expected.signature_count()
            || original.a.len() != expected.stage_count()
            || original.w.len() + 1 != expected.stage_count()
        {
            return Err(Error::Inventory);
        }
        original_bounds(&original.sigma_q, &mut total, config)?;
        for item in original
            .signatures
            .iter()
            .chain(&original.a)
            .chain(&original.w)
        {
            original_bounds(item, &mut total, config)?;
        }
    }
    add_size(
        &mut total,
        &pack.omega_proving_key,
        config.key.maximum_bytes,
        config.maximum_package_bytes,
    )
}

fn params<C: iroha_pasta::PastaCurve>(k: u32) -> Result<PinnedParams<C>, Error> {
    PinnedParams::<C>::derive(k).map_err(|_| Error::Profile)
}

fn geometry_shape(
    geometry: SigmaGeometryV1,
    relation: SigmaRelation,
    k: u32,
) -> Result<SigmaShape, Error> {
    if !matches!(k, 12 | 14) {
        return Err(Error::Profile);
    }
    let parameters = SigmaParams::new(
        RelationShape::new(relation, geometry.prefix),
        geometry.lanes,
        geometry.limb_bits,
    )
    .map_err(|_| Error::Profile)?;
    Ok(SigmaShape::new(parameters, k))
}

fn import_sigma(
    installed: &InstalledVerifierPackV1,
    pack: &ProducerPackV1,
    source: &NativeSourceV1,
    config: ReadConfig,
) -> Result<[SigmaProducerV1; 16], Error> {
    if installed.originals().steps.len() != 16 {
        return Err(Error::Inventory);
    }
    let mut all = Vec::with_capacity(16);
    let mut parameter_classes: BTreeMap<u32, PinnedParams<Eq>> = BTreeMap::new();
    for (index, (step, pk)) in installed
        .originals()
        .steps
        .iter()
        .zip(&pack.sigma_proving_keys)
        .enumerate()
    {
        if (step.kind.tag(), step.enabled_controls) != SIGMA_CATALOG_V1[index] {
            return Err(Error::Profile);
        }
        let d = &step.artifact.descriptor;
        let v = &step.artifact.verifying_key;
        let binding = DescriptorBinding::decode_v2(d).map_err(|_| Error::Profile)?;
        if binding.n() > config.maximum_rows {
            return Err(Error::Inventory);
        }
        let k = u32::from(binding.descriptor().k);
        if ((2..=11).contains(&index) && !matches!(k, 12 | 14))
            || (!(2..=11).contains(&index) && k != 12)
        {
            return Err(Error::Profile);
        }
        let parameter_k = if (2..=11).contains(&index) { k } else { 12 };
        let pinned = match parameter_classes.get(&parameter_k) {
            Some(p) => p.clone(),
            None => {
                let p = params::<Eq>(parameter_k)?;
                parameter_classes.insert(parameter_k, p.clone());
                p
            }
        };
        macro_rules! admin {
            ($owner:ident,$variant:ident) => {
                SigmaProducerV1::$variant(
                    $owner::from_original_artifact(pinned, d, v, pk, config)
                        .map_err(|_| Error::Artifact)?,
                )
            };
        }
        let owner = match index {
            0 => admin!(BootstrapProver, Bootstrap),
            1 => admin!(LoadProver, Load),
            2..=9 => SigmaProducerV1::Send(
                SigmaProver::<Eq>::from_original_artifact(
                    geometry_shape(
                        source.send_sigma,
                        SigmaRelation::send((index - 2) as u32),
                        k,
                    )?,
                    pinned,
                    d,
                    v,
                    pk,
                    config,
                )
                .map_err(|_| Error::Artifact)?,
            ),
            10..=11 => SigmaProducerV1::Receive(
                SigmaProver::<Eq>::from_original_artifact(
                    geometry_shape(
                        source.receive_sigma,
                        SigmaRelation::receive((index - 10) as u32),
                        k,
                    )?,
                    pinned,
                    d,
                    v,
                    pk,
                    config,
                )
                .map_err(|_| Error::Artifact)?,
            ),
            12 => admin!(ArchiveProver, Archive),
            13 => admin!(UnloadProver, Unload),
            14 => admin!(RefreshProver, Refresh),
            15 => admin!(RetiringProver, Retiring),
            _ => return Err(Error::Inventory),
        };
        all.push(owner);
    }
    all.try_into().map_err(|_| Error::Inventory)
}

fn sigma_class(all: &[SigmaProducerV1; 16], indices: &[usize]) -> Result<SigmaClass, Error> {
    let first = all
        .get(*indices.first().ok_or(Error::Profile)?)
        .ok_or(Error::Profile)?;
    let verifier = VerifierPlan::new(first.binding().clone(), first.params().clone())
        .map_err(|_| Error::Profile)?;
    let mut entries = Vec::with_capacity(indices.len());
    for &index in indices {
        let item = all.get(index).ok_or(Error::Profile)?;
        if item.binding() != first.binding() {
            return Err(Error::Profile);
        }
        entries.push((
            u8::try_from(index).map_err(|_| Error::Profile)?,
            item.verifying_key()
                .kagemusha_digest(item.binding())
                .map_err(|_| Error::Profile)?,
        ));
    }
    SigmaClass::new(verifier, entries).map_err(|_| Error::Profile)
}
fn sigma_plan(
    class: ProducerClassV1,
    all: &[SigmaProducerV1; 16],
    vesta: &PinnedParams<Eq>,
) -> Result<QSigmaPlan, Error> {
    let own = match class {
        ProducerClassV1::Send { .. } => sigma_class(all, &[2, 3, 4, 5, 6, 7, 8, 9])?,
        ProducerClassV1::Receive { .. } => sigma_class(all, &[10, 11])?,
        _ => sigma_class(all, &[class.sigma_index()])?,
    };
    let incoming = match class {
        ProducerClassV1::Receive { .. } => Some(sigma_class(all, &[2, 3, 4, 5, 6, 7, 8, 9])?),
        ProducerClassV1::ArchiveReceive { .. } => Some(sigma_class(all, &[10, 11])?),
        _ => None,
    };
    QSigmaPlan::new(own, incoming, vesta).map_err(|_| Error::Profile)
}
#[allow(clippy::too_many_arguments)]
fn import_sigma_q(
    plan: QSigmaPlan,
    class: ProducerClassV1,
    all: &[SigmaProducerV1; 16],
    original: &ProducerOriginalV1,
    source: &NativeSourceV1,
    pallas: &PinnedParams<Ep>,
    config: ReadConfig,
) -> Result<QSigmaProver, Error> {
    let incoming = match class {
        ProducerClassV1::Receive { .. } => Some(all[2].verifying_key().clone()),
        ProducerClassV1::ArchiveReceive { .. } => Some(all[10].verifying_key().clone()),
        _ => None,
    };
    let fixed = QSigmaSource::new(
        plan,
        all[class.sigma_index()].verifying_key().clone(),
        incoming,
    )
    .map_err(|_| Error::Profile)?;
    match source.sigma_q {
        QSigmaLayoutV1::Ordinary => QSigmaProver::from_original_artifact(
            &fixed,
            pallas.clone(),
            &original.descriptor,
            &original.verifying_key,
            &original.proving_key,
            config,
        ),
        QSigmaLayoutV1::SerializedForeign { range_buses } => {
            QSigmaProver::from_original_artifact_serialized_foreign(
                &fixed,
                pallas.clone(),
                &original.descriptor,
                &original.verifying_key,
                &original.proving_key,
                config,
                range_buses,
            )
        }
    }
    .map_err(|_| Error::Artifact)
}

fn digest_limbs(bytes: [u8; 32]) -> [u128; 2] {
    [
        u128::from_le_bytes(bytes[..16].try_into().expect("fixed digest half")),
        u128::from_le_bytes(bytes[16..].try_into().expect("fixed digest half")),
    ]
}
fn root_point(key: KagemushaDevicePublicKeyV1) -> Result<Affine, Error> {
    key.validate().map_err(|_| Error::Profile)?;
    let original = key.as_sec1_bytes();
    let words = |bytes: &[u8]| -> Result<[u64; 4], Error> {
        let chunks = bytes
            .chunks_exact(8)
            .map(|chunk| {
                Ok(u64::from_be_bytes(
                    chunk.try_into().map_err(|_| Error::Profile)?,
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let mut values: [u64; 4] = chunks.try_into().map_err(|_| Error::Profile)?;
        values.reverse();
        Ok(values)
    };
    let point = Affine {
        x: words(&original[1..33])?,
        y: words(&original[33..65])?,
    };
    if !point.is_valid() {
        return Err(Error::Profile);
    }
    Ok(point)
}
fn signature_plan(
    root: Affine,
    variable: usize,
    mode: VerifyMode,
    fixed: bool,
) -> Result<QSignaturePlan, Error> {
    let mut slots = vec![
        SignatureSlot {
            mode,
            key: SignatureKey::Variable
        };
        variable
    ];
    if fixed {
        slots.push(SignatureSlot {
            mode,
            key: SignatureKey::Fixed(root),
        });
    }
    QSignaturePlan::new(slots).map_err(|_| Error::Profile)
}
fn signature_plans(
    class: ProducerClassV1,
    policy: OwnPolicy,
    root: Affine,
) -> Result<Vec<QSignaturePlan>, Error> {
    let hard = || signature_plan(root, 2, VerifyMode::Hard, true);
    match class {
        ProducerClassV1::Bootstrap
        | ProducerClassV1::Send { .. }
        | ProducerClassV1::Unload
        | ProducerClassV1::Retiring => Ok(vec![hard()?]),
        ProducerClassV1::Load => Ok(vec![
            hard()?,
            signature_plan(root, 1, VerifyMode::Hard, true)?,
        ]),
        ProducerClassV1::Receive { .. } => {
            ReceiveStagePlan::signature_schemas(class.variant(), policy)
                .map(Vec::from)
                .map_err(|_| Error::Profile)
        }
        ProducerClassV1::ArchiveReceive { .. } | ProducerClassV1::ArchiveStatus => {
            archive::Plan::signature_schemas(policy)
                .map(Vec::from)
                .map_err(|_| Error::Profile)
        }
        _ => Ok(vec![
            hard()?,
            signature_plan(root, 1, VerifyMode::Hard, true)?,
        ]),
    }
}

macro_rules! originals {
    ($originals:expr,$module:ident,$n:expr) => {{
        let values = ($originals)
            .iter()
            .map(|item| $module::OriginalArtifact {
                descriptor: &item.descriptor,
                verifying_key: &item.verifying_key,
                proving_key: &item.proving_key,
            })
            .collect::<Vec<_>>();
        let values: [$module::OriginalArtifact<'_>; $n] =
            values.try_into().map_err(|_| Error::Inventory)?;
        values
    }};
}

#[allow(clippy::too_many_arguments)]
fn import_operation(
    class: ProducerClassV1,
    original: &OperationOriginalsV1,
    operation: AProofPlan,
    policy: OwnPolicy,
    bootstrap_policy: BootstrapPolicy,
    signatures: Vec<QSignaturePlan>,
    omega_key: &VerifyingKey<Ep>,
    omega_capacity: usize,
    sigma_capacity: usize,
    pallas: &PinnedParams<Ep>,
    vesta: &PinnedParams<Eq>,
    config: ReadConfig,
) -> Result<OperationProducerV1, Error> {
    let one = || signatures.first().cloned().ok_or(Error::Profile);
    let two = || -> Result<[QSignaturePlan; 2], Error> {
        signatures.clone().try_into().map_err(|_| Error::Profile)
    };
    match class {
        ProducerClassV1::Bootstrap => {
            let plan = bootstrap::Plan::new(
                operation,
                bootstrap_policy,
                one()?,
                pallas.clone(),
                vesta.clone(),
            )
            .map_err(|_| Error::Profile)?;
            let a = originals!(&original.a, bootstrap, 2);
            let w = originals!(&original.w, bootstrap, 1);
            bootstrap::Prover::from_original_artifacts(plan, a[0], w[0], a[1], config)
                .map(OperationProducerV1::Bootstrap)
                .map_err(|_| Error::Artifact)
        }
        ProducerClassV1::Load => {
            let plan = load::Plan::new(
                operation,
                policy,
                two()?,
                omega_key.clone(),
                pallas.clone(),
                vesta.clone(),
            )
            .map_err(|_| Error::Profile)?;
            load::Prover::from_original_artifacts(
                plan,
                originals!(&original.a, load, 4),
                originals!(&original.w, load, 3),
                config,
            )
            .map(OperationProducerV1::Load)
            .map_err(|_| Error::Artifact)
        }
        ProducerClassV1::Send { controls } => {
            let plan = send::Plan::new(
                operation,
                controls,
                policy,
                one()?,
                omega_key.clone(),
                pallas.clone(),
                vesta.clone(),
            )
            .map_err(|_| Error::Profile)?;
            send::Prover::from_original_artifacts(
                plan,
                originals!(&original.a, send, 5),
                originals!(&original.w, send, 4),
                config,
            )
            .map(OperationProducerV1::Send)
            .map_err(|_| Error::Artifact)
        }
        ProducerClassV1::Receive { blacklist, .. } => {
            let plan = receive::Plan::new(
                operation,
                blacklist,
                policy,
                two()?,
                omega_key.clone(),
                omega_capacity,
                sigma_capacity,
                pallas.clone(),
                vesta.clone(),
            )
            .map_err(|_| Error::Profile)?;
            receive::Prover::from_original_artifacts(
                plan,
                originals!(&original.a, receive, 11),
                originals!(&original.w, receive, 10),
                config,
            )
            .map(OperationProducerV1::Receive)
            .map_err(|_| Error::Artifact)
        }
        ProducerClassV1::Unload => {
            let plan = unload::Plan::new(
                operation,
                policy,
                one()?,
                omega_key.clone(),
                pallas.clone(),
                vesta.clone(),
            )
            .map_err(|_| Error::Profile)?;
            unload::Prover::from_original_artifacts(
                plan,
                originals!(&original.a, unload, 4),
                originals!(&original.w, unload, 3),
                config,
            )
            .map(OperationProducerV1::Unload)
            .map_err(|_| Error::Artifact)
        }
        ProducerClassV1::ArchiveReceive { .. } | ProducerClassV1::ArchiveStatus => {
            let blacklist = matches!(class, ProducerClassV1::ArchiveReceive { blacklist: true });
            let plan = archive::Plan::new(
                operation,
                blacklist,
                policy,
                two()?,
                omega_key.clone(),
                omega_capacity,
                sigma_capacity,
                pallas.clone(),
                vesta.clone(),
            )
            .map_err(|_| Error::Profile)?;
            archive::Prover::from_original_artifacts(
                plan,
                originals!(&original.a, archive, 4),
                originals!(&original.w, archive, 3),
                config,
            )
            .map(OperationProducerV1::Archive)
            .map_err(|_| Error::Artifact)
        }
        ProducerClassV1::Retiring => {
            let plan = retiring::Plan::new(
                operation,
                policy,
                one()?,
                omega_key.clone(),
                pallas.clone(),
                vesta.clone(),
            )
            .map_err(|_| Error::Profile)?;
            retiring::Prover::from_original_artifacts(
                plan,
                originals!(&original.a, retiring, 4),
                originals!(&original.w, retiring, 3),
                config,
            )
            .map(OperationProducerV1::Retiring)
            .map_err(|_| Error::Artifact)
        }
        _ => {
            let plan = refresh::Plan::new(
                operation,
                policy,
                two()?,
                omega_key.clone(),
                pallas.clone(),
                vesta.clone(),
            )
            .map_err(|_| Error::Profile)?;
            refresh::Prover::from_original_artifacts(
                plan,
                originals!(&original.a, refresh, 4),
                originals!(&original.w, refresh, 3),
                config,
            )
            .map(OperationProducerV1::Refresh)
            .map_err(|_| Error::Artifact)
        }
    }
}

impl InstalledProducerCatalogV1 {
    /// Decode exact bounded canonical originals, then import the complete genuine
    /// source tree under the installed signed Omega verifier authority.
    /// # Errors
    /// Oversized/noncanonical/partial package or any refusal in [`Self::load`].
    pub fn from_originals(
        installed: &InstalledVerifierPackV1,
        original: &[u8],
        source: NativeSourceV1,
        config: CatalogReadConfigV1,
    ) -> Result<Self, Error> {
        if original.is_empty() || original.len() > config.maximum_package_bytes {
            return Err(Error::Inventory);
        }
        let pack: ProducerPackV1 = norito::decode_canonical_with_limits(
            original,
            norito::canonical_decode_limits(config.maximum_package_bytes),
        )
        .map_err(|_| Error::Inventory)?;
        Self::load(installed, &pack, source, config)
    }

    /// Import all16 typed sigma PKs, all24 genuine operation/Q/A/W classes, and
    /// the complete immutable final Omega program rooted under the installed VK.
    /// This result is a producer capability, not an enrolled custody/session owner.
    /// # Errors
    /// Wrong full order/resource/profile, missing source originals, nonuniform
    /// sigma descriptor classes/A descriptor, source/key mismatch or signed-root
    /// inequality. There is no successful partial catalog or profile fallback.
    pub fn load(
        installed: &InstalledVerifierPackV1,
        pack: &ProducerPackV1,
        source: NativeSourceV1,
        config: CatalogReadConfigV1,
    ) -> Result<Self, Error> {
        inventory_bounds(pack, config)?;
        let sigma = import_sigma(installed, pack, &source, config.key)?;
        let pallas = params::<Ep>(16)?;
        let vesta = params::<Eq>(16)?;
        let scheme = installed.verifier().scheme();
        let root = root_point(scheme.scheme_root_key)?;
        let own_policy = OwnPolicy::new(
            digest_limbs(scheme.scheme_id()),
            digest_limbs(scheme.provider_contract),
            root,
        )
        .map_err(|_| Error::Profile)?;
        let bootstrap_policy = BootstrapPolicy::new(
            digest_limbs(scheme.scheme_id()),
            digest_limbs(scheme.provider_contract),
            root,
        )
        .map_err(|_| Error::Profile)?;
        let omega_original = &installed.originals().lineage;
        let omega_binding =
            DescriptorBinding::decode_v2(&omega_original.descriptor).map_err(|_| Error::Profile)?;
        let omega_key = VerifyingKey::<Ep>::read(&omega_original.verifying_key, &omega_binding)
            .map_err(|_| Error::Profile)?;
        let omega_plan =
            VerifierPlan::new(omega_binding.clone(), pallas.clone()).map_err(|_| Error::Profile)?;
        let omega_capacity = 320_usize
            .checked_add(omega_plan.proof_length())
            .and_then(|n| n.checked_add(1088))
            .ok_or(Error::Profile)?;
        let send_capacity = Protocol::new(sigma[2].binding().descriptor())
            .map_err(|_| Error::Profile)?
            .proof_length();
        let receive_capacity = Protocol::new(sigma[10].binding().descriptor())
            .map_err(|_| Error::Profile)?
            .proof_length();
        if omega_capacity
            .checked_add(send_capacity)
            .ok_or(Error::Profile)?
            > receive::MAX_PAYMENT_ORIGINAL_BYTES
        {
            return Err(Error::Profile);
        }
        let mut operations = Vec::with_capacity(PRODUCER_CLASS_COUNT_V1);
        let mut terminal_keys: Vec<Vec<u8>> = Vec::new();
        let mut terminal_descriptor: Option<Vec<u8>> = None;
        for original in &pack.operations {
            let class = original.class;
            let plan = sigma_plan(class, &sigma, &vesta)?;
            let q_sigma = import_sigma_q(
                plan.clone(),
                class,
                &sigma,
                &original.sigma_q,
                &source,
                &pallas,
                config.key,
            )?;
            let signature_plans = signature_plans(class, own_policy, root)?;
            let signatures = signature_plans
                .iter()
                .cloned()
                .zip(&original.signatures)
                .map(|(plan, item)| {
                    QSignatureProver::from_original_artifact(
                        plan,
                        pallas.clone(),
                        &item.descriptor,
                        &item.verifying_key,
                        &item.proving_key,
                        config.key,
                    )
                    .map_err(|_| Error::Artifact)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut qs = vec![
                QProofPlan::new(
                    VerifierPlan::new(q_sigma.binding().clone(), pallas.clone())
                        .map_err(|_| Error::Profile)?,
                    q_sigma.verifying_key().clone(),
                )
                .map_err(|_| Error::Profile)?,
            ];
            for item in &signatures {
                qs.push(
                    QProofPlan::new(
                        VerifierPlan::new(item.binding().clone(), pallas.clone())
                            .map_err(|_| Error::Profile)?,
                        item.verifying_key().clone(),
                    )
                    .map_err(|_| Error::Profile)?,
                );
            }
            let operation = AProofPlan::new(
                class.variant(),
                plan.clone(),
                qs,
                if class == ProducerClassV1::Bootstrap {
                    None
                } else {
                    Some(omega_plan.clone())
                },
                &pallas,
            )
            .map_err(|_| Error::Profile)?;
            let incoming_sigma_capacity = if matches!(class, ProducerClassV1::ArchiveReceive { .. })
            {
                receive_capacity
            } else {
                send_capacity
            };
            let producer = import_operation(
                class,
                original,
                operation,
                own_policy,
                bootstrap_policy,
                signature_plans,
                &omega_key,
                omega_capacity,
                incoming_sigma_capacity,
                &pallas,
                &vesta,
                config.key,
            )?;
            let terminal = original.a.last().ok_or(Error::Inventory)?;
            if terminal_descriptor
                .as_ref()
                .is_some_and(|d| d != &terminal.descriptor)
            {
                return Err(Error::Profile);
            }
            terminal_descriptor.get_or_insert_with(|| terminal.descriptor.clone());
            // Only after every semantic owner has passed its real source import
            // may identical actual terminal VK originals share a catalog entry.
            let terminal_index = match terminal_keys
                .iter()
                .position(|key| key == &terminal.verifying_key)
            {
                Some(index) => index,
                None => {
                    terminal_keys.push(terminal.verifying_key.clone());
                    terminal_keys.len() - 1
                }
            };
            let binding =
                DescriptorBinding::decode_v2(&terminal.descriptor).map_err(|_| Error::Profile)?;
            let terminal_key = VerifyingKey::<Eq>::read(&terminal.verifying_key, &binding)
                .map_err(|_| Error::Profile)?;
            operations.push(InstalledOperationProducerV1 {
                class,
                sigma_plan: plan,
                sigma_q: q_sigma,
                signatures,
                producer,
                terminal_key,
                terminal_index,
            });
        }
        let program = final_omega::Program::new(
            &terminal_descriptor.ok_or(Error::Inventory)?,
            &terminal_keys,
            vesta,
            pallas,
            source.omega,
        )
        .map_err(|_| Error::Profile)?;
        // This last strict source import is the authority root for the entire
        // complete tree. Failure drops all local components without publishing
        // a partial catalog, wallet owner or native-ready state.
        let omega = final_omega::Prover::from_original_artifact(
            program,
            &omega_original.descriptor,
            &omega_original.verifying_key,
            &pack.omega_proving_key,
            config.key,
        )
        .map_err(|_| Error::Artifact)?;
        Ok(Self {
            manifest: installed.verifier().manifest_digest(),
            sigma,
            operations,
            omega,
        })
    }
    /// Exact signed installed ArtifactManifest identity of this immutable catalog.
    #[must_use]
    pub const fn manifest_digest(&self) -> [u8; 32] {
        self.manifest
    }
    /// Actual typed sigma owner at one fixed signed global catalog selector.
    #[must_use]
    pub fn sigma(&self, selector: usize) -> Option<&SigmaProducerV1> {
        self.sigma.get(selector)
    }
    /// Actual complete operation owner for one required immutable class.
    #[must_use]
    pub fn operation(&self, class: ProducerClassV1) -> Option<&InstalledOperationProducerV1> {
        self.operations.iter().find(|item| item.class == class)
    }
    /// Actual complete final Omega producer under the signed installed VK.
    #[must_use]
    pub const fn omega(&self) -> &final_omega::Prover {
        &self.omega
    }
}

#[cfg(test)]
#[path = "kagemusha_wallet_producers_v1/tests.rs"]
mod tests;
