//! Five-program Artifact-role manifest. Issuance and review use independently
//! selected compiler pins and the same scheme and finality owners as installation.
use super::{Error, ProgramBundleGeometry, ProgramBundleV2, Result, engine};
use crate::{certificate, ipa::Gens};
use ff::PrimeField;
use iroha_data_model::{
    isi::kagemusha_wallet::spartan::InstallKagemushaSpartanProgramsV1,
    kagemusha::KagemushaWalletSchemeV1, sumeragi_finality::SumeragiCommitVerifierV1,
};
use iroha_pasta::{Ep, Eq};

/// Ordered public pins from the generation compiler. Each side uses Receive,
/// Send, Bootstrap, Control and ArchiveSent, in that order.
#[derive(
    Clone, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.ProgramProfile.v2")]
pub struct ProgramProfileV2 {
    pub primary: [[u8; 32]; 5],
    pub secondary: [[u8; 32]; 5],
    pub dimensions: [[u32; 2]; 2],
}

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.Parameters.v5")]
struct ParameterManifestV5 {
    version: u16,
    scheme: [u8; 32],
    relation: [u8; 32],
    programs: ProgramProfileV2,
    generator_domain: String,
    hash_to_curve: String,
    generator_capacity_bits: [u32; 2],
    generator_digests: [[u8; 32]; 2],
    table_construction: String,
    table_vector: [[u8; 32]; 2],
    finality_network: String,
    finality_chain: String,
    signer_certificate: Vec<u8>,
}

/// Public claims authenticated by the selected Artifact signature. Installation
/// also checks the independent producer pins, finality owner and all originals.
#[non_exhaustive]
pub struct AuthenticatedManifestHeaderV5 {
    pub scheme: [u8; 32],
    pub relation: [u8; 32],
    pub programs: ProgramProfileV2,
    pub manifest_digest: [u8; 32],
    pub finality_network: String,
    pub finality_chain: String,
    pub signer_certificate: Vec<u8>,
    pub generator_capacity_bits: [u32; 2],
    pub generator_digests: [[u8; 32]; 2],
    pub table_vector: [[u8; 32]; 2],
}

type GeneratorSets = (Gens<Eq>, Gens<Ep>);

fn derive_generators(geometry: &[ProgramBundleGeometry; 2]) -> GeneratorSets {
    // The checked per-side tape geometry bounds each shift and allocation.
    let [p, s] = geometry.map(|side| 1usize << side.column_bits);
    (Gens::new(p), Gens::new(s))
}

impl AuthenticatedManifestHeaderV5 {
    pub(super) fn verify_generators(&self, generators: &GeneratorSets) -> Result<()> {
        let profile = engine::generator_profile(&generators.0, &generators.1)?;
        if profile.capacity_bits != self.generator_capacity_bits
            || profile.digests != self.generator_digests
            || profile.table_vector != self.table_vector
        {
            return Err(Error::AuthorityMismatch(
                "canonical five-kind generator and table profile",
            ));
        }
        Ok(())
    }
}

impl ProgramProfileV2 {
    /// Authenticate the bounded canonical header before importing any tapes.
    pub fn authenticate_manifest_header(
        bytes: &[u8],
        signature: &[u8; 64],
        selected_digest: &[u8; 32],
        scheme: &KagemushaWalletSchemeV1,
    ) -> Result<AuthenticatedManifestHeaderV5> {
        let manifest = authenticate_header(bytes, signature, selected_digest, scheme)?;
        Ok(AuthenticatedManifestHeaderV5 {
            scheme: manifest.scheme,
            relation: manifest.relation,
            programs: manifest.programs,
            manifest_digest: *selected_digest,
            finality_network: manifest.finality_network,
            finality_chain: manifest.finality_chain,
            signer_certificate: manifest.signer_certificate,
            generator_capacity_bits: manifest.generator_capacity_bits,
            generator_digests: manifest.generator_digests,
            table_vector: manifest.table_vector,
        })
    }

    fn measure_programs(
        &self,
        bundle: &[u8],
    ) -> Result<([ProgramBundleGeometry; 2], engine::GeneratorProfile)> {
        let (geometry, _) = ProgramBundleV2::inspect_retail_originals(
            bundle,
            &self.primary,
            &self.secondary,
            self.dimensions,
        )?;
        let (p, s) = derive_generators(&geometry);
        let gens = engine::generator_profile(&p, &s)?;
        Ok((geometry, gens))
    }

    fn measure(
        &self,
        bundle: &[u8],
        scheme: &KagemushaWalletSchemeV1,
        finality: &SumeragiCommitVerifierV1,
        certificate: &[u8],
    ) -> Result<(ParameterManifestV5, [ProgramBundleGeometry; 2])> {
        let (geometry, gens) = self.measure_programs(bundle)?;
        let manifest = ParameterManifestV5 {
            version: 5,
            scheme: scheme.scheme_id(),
            relation: scheme.relation_id,
            programs: self.clone(),
            generator_domain: crate::ipa::GENERATOR_DOMAIN.into(),
            hash_to_curve: engine::HASH_TO_CURVE_SUITE.into(),
            generator_capacity_bits: gens.capacity_bits,
            generator_digests: gens.digests,
            table_construction: engine::TABLE_CONSTRUCTION.into(),
            table_vector: gens.table_vector,
            finality_network: finality.network().to_string(),
            finality_chain: finality.chain_id().to_owned(),
            signer_certificate: certificate.to_vec(),
        };
        Ok((manifest, geometry))
    }

    /// Prepare the exact unsigned manifest and message for an independent
    /// Artifact signer. This operation only processes public inputs.
    pub fn prepare_manifest_for_signing(
        &self,
        bundle: &[u8],
        scheme: &KagemushaWalletSchemeV1,
        finality: &SumeragiCommitVerifierV1,
        certificate: &[u8],
    ) -> Result<(Vec<u8>, [u8; 32])> {
        if finality.network().as_bytes() != &scheme.network_id {
            return Err(Error::AuthorityMismatch("parameter ledger root"));
        }
        artifact_key(scheme, certificate)?;
        let (manifest, _) = self.measure(bundle, scheme, finality, certificate)?;
        let bytes = super::encode(&manifest)?;
        require_install_extent(bundle, &bytes)?;
        let digest = engine::manifest_signing_message(&bytes).to_repr();
        Ok((bytes, digest))
    }

    /// Authenticate the selected signed original and independently reconstruct
    /// all ten program pins, both generator sets, and the certified table vector.
    /// Returns public geometry for offline release review.
    pub fn verify_signed_originals(
        &self,
        bundle: &[u8],
        bytes: &[u8],
        signature: &[u8; 64],
        selected_digest: &[u8; 32],
        scheme: &KagemushaWalletSchemeV1,
        finality: &SumeragiCommitVerifierV1,
    ) -> Result<[ProgramBundleGeometry; 2]> {
        let header =
            self.install_header(bundle, bytes, signature, selected_digest, scheme, finality)?;
        self.verify_compiled_originals(bundle, &header)
    }

    fn install_header(
        &self,
        bundle: &[u8],
        bytes: &[u8],
        signature: &[u8; 64],
        selected_digest: &[u8; 32],
        scheme: &KagemushaWalletSchemeV1,
        finality: &SumeragiCommitVerifierV1,
    ) -> Result<AuthenticatedManifestHeaderV5> {
        if bundle.len() > super::MAX_PROGRAM_BUNDLE_BYTES {
            return Err(Error::ResourceLimit);
        }
        let header = Self::authenticate_manifest_header(bytes, signature, selected_digest, scheme)?;
        if header.programs != *self
            || header.finality_network != finality.network().to_string()
            || header.finality_chain != finality.chain_id()
        {
            return Err(Error::AuthorityMismatch("selected five-kind manifest"));
        }
        require_install_extent(bundle, bytes)?;
        Ok(header)
    }

    pub(super) fn import_signed_programs(
        &self,
        bundle: &[u8],
        bytes: &[u8],
        signature: &[u8; 64],
        selected_digest: &[u8; 32],
        scheme: &KagemushaWalletSchemeV1,
        finality: &SumeragiCommitVerifierV1,
    ) -> Result<(Box<[engine::Program]>, GeneratorSets)> {
        let header =
            self.install_header(bundle, bytes, signature, selected_digest, scheme, finality)?;
        let (geometry, programs) = ProgramBundleV2::import_retail_programs(bundle, self)?;
        let generators = derive_generators(&geometry);
        header.verify_generators(&generators)?;
        Ok((programs, generators))
    }

    /// Compare independently selected producer pins and imported tapes with the
    /// generator and table claims of a previously authenticated carrier header.
    /// Offline native installation separately binds its own finality reader.
    pub fn verify_compiled_originals(
        &self,
        bundle: &[u8],
        header: &AuthenticatedManifestHeaderV5,
    ) -> Result<[ProgramBundleGeometry; 2]> {
        if header.programs != *self {
            return Err(Error::AuthorityMismatch("selected five-kind manifest"));
        }
        let (geometry, _) = ProgramBundleV2::inspect_retail_originals(
            bundle,
            &self.primary,
            &self.secondary,
            self.dimensions,
        )?;
        header.verify_generators(&derive_generators(&geometry))?;
        Ok(geometry)
    }
}

fn authenticate_header(
    bytes: &[u8],
    signature: &[u8; 64],
    selected_digest: &[u8; 32],
    scheme: &KagemushaWalletSchemeV1,
) -> Result<ParameterManifestV5> {
    const LIMIT: usize = InstallKagemushaSpartanProgramsV1::MAX_MANIFEST_BYTES;
    if bytes.len() > LIMIT {
        return Err(Error::ResourceLimit);
    }
    let limits = norito::DecodeLimits::new(LIMIT, LIMIT, 2 * LIMIT, 4 * LIMIT, 16);
    let manifest: ParameterManifestV5 =
        norito::core::with_decode_limits_scope(limits, || super::decode(bytes))?;
    if manifest.version != 5
        || manifest.scheme != scheme.scheme_id()
        || manifest.relation != scheme.relation_id
        || manifest.finality_chain.is_empty()
    {
        return Err(Error::AuthorityMismatch("selected five-kind manifest"));
    }
    if manifest.generator_domain != crate::ipa::GENERATOR_DOMAIN
        || manifest.hash_to_curve != engine::HASH_TO_CURVE_SUITE
        || manifest.table_construction != engine::TABLE_CONSTRUCTION
    {
        return Err(Error::AuthorityMismatch(
            "canonical five-kind generator and table profile",
        ));
    }
    let network: iroha_data_model::prelude::NetworkId = manifest
        .finality_network
        .parse()
        .map_err(|_| Error::Malformed("parameter finality network"))?;
    if network.as_bytes() != &scheme.network_id || network.to_string() != manifest.finality_network
    {
        return Err(Error::AuthorityMismatch("parameter ledger root"));
    }
    norito::verify_exact_canonical_frame_with_layout(
        &manifest,
        bytes,
        norito::core::WireLayout::Fixed,
    )
    .map_err(|_| Error::Malformed("canonical five-kind manifest"))?;
    let message = engine::manifest_signing_message(bytes);
    if message.to_repr() != *selected_digest {
        return Err(Error::AuthorityMismatch(
            "selected five-kind manifest digest",
        ));
    }
    let key = artifact_key(scheme, &manifest.signer_certificate)?;
    engine::verify_signature(&key, message, signature)?;
    Ok(manifest)
}

fn require_install_extent(bundle: &[u8], manifest: &[u8]) -> Result<()> {
    use iroha_data_model::isi::kagemusha_wallet::spartan::InstallKagemushaSpartanProgramsV1 as Install;
    if manifest.len() > Install::MAX_MANIFEST_BYTES
        || bundle.len().saturating_add(manifest.len()) > Install::MAX_BYTES
    {
        return Err(Error::ResourceLimit);
    }
    // The frame has fixed-width identities and signature. Their values do not
    // affect its extent; the ledger uses this same concrete encoder and layout.
    let install = Install {
        scheme: [0; 32],
        asset: [0; 32],
        manifest_digest: [0; 32],
        manifest: manifest.to_vec(),
        signature: [0; 64],
        programs: bundle.to_vec(),
    };
    let _layout = norito::core::WireLayoutGuard::enter(norito::core::WireLayout::Fixed);
    let frame_len = norito::core::encoded_frame_len(&install)
        .map_err(|error| Error::Rejected(error.to_string()))?;
    if frame_len > Install::MAX_BYTES {
        return Err(Error::ResourceLimit);
    }
    Ok(())
}

fn artifact_key(
    scheme: &KagemushaWalletSchemeV1,
    certificate: &[u8],
) -> Result<crate::p256::Point> {
    scheme
        .validate()
        .map_err(|e| Error::Rejected(e.to_string()))?;
    let signer = engine::read_certificate(certificate)?;
    certificate::authenticate_signer(
        &signer,
        &scheme.scheme_id(),
        &super::point(scheme.scheme_root_key.as_ref())?,
        certificate::Role::Artifact,
    )
    .map_err(Error::AuthorityMismatch)
}

#[cfg(test)]
#[path = "successor_manifest_tests.rs"]
mod tests;
