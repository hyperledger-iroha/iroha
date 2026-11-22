//! Five-program Artifact-role manifest. Issuance and review use independently
//! selected compiler pins and the same scheme and finality owners as installation.
use super::{Error, ProgramBundleGeometry, ProgramBundleV2, Result, engine};
use crate::{certificate, ipa::Gens};
use ff::PrimeField;
use iroha_data_model::{
    isi::kagemusha_wallet::spartan::InstallKagemushaSpartanProgramsV1,
    kagemusha::KagemushaWalletSchemeV1, sumeragi_finality::SumeragiCommitVerifierV1,
};

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

impl ProgramProfileV2 {
    fn measure(
        &self,
        bundle: &[u8],
        scheme: &KagemushaWalletSchemeV1,
        finality: &SumeragiCommitVerifierV1,
        certificate: &[u8],
    ) -> Result<(ParameterManifestV5, [ProgramBundleGeometry; 2])> {
        let geometry = ProgramBundleV2::inspect_originals(
            bundle,
            &self.primary,
            &self.secondary,
            self.dimensions,
        )?;
        // Capacities come from the independently pinned, imported tapes. Import
        // bounds each column width before we allocate or derive generators.
        let [p, s] = geometry.map(|g| 1usize << g.column_bits);
        let gens = engine::generator_profile(&Gens::new(p), &Gens::new(s))?;
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
        artifact_key(scheme, finality, certificate)?;
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
        const LIMIT: usize = InstallKagemushaSpartanProgramsV1::MAX_MANIFEST_BYTES;
        if bytes.len() > LIMIT || bundle.len() > super::MAX_PROGRAM_BUNDLE_BYTES {
            return Err(Error::ResourceLimit);
        }
        let limits = norito::DecodeLimits::new(LIMIT, LIMIT, 2 * LIMIT, 4 * LIMIT, 16);
        let manifest: ParameterManifestV5 =
            norito::core::with_decode_limits_scope(limits, || super::decode(bytes))?;
        if manifest.version != 5
            || manifest.programs != *self
            || manifest.scheme != scheme.scheme_id()
            || manifest.relation != scheme.relation_id
            || manifest.finality_network != finality.network().to_string()
            || manifest.finality_chain != finality.chain_id()
        {
            return Err(Error::AuthorityMismatch("selected five-kind manifest"));
        }
        let message = engine::manifest_signing_message(bytes);
        if message.to_repr() != *selected_digest {
            return Err(Error::AuthorityMismatch(
                "selected five-kind manifest digest",
            ));
        }
        let key = artifact_key(scheme, finality, &manifest.signer_certificate)?;
        engine::verify_signature(&key, message, signature)?;
        require_install_extent(bundle, bytes)?;
        let (expected, geometry) =
            self.measure(bundle, scheme, finality, &manifest.signer_certificate)?;
        if super::encode(&expected)? != bytes {
            return Err(Error::AuthorityMismatch(
                "canonical five-kind generator and table profile",
            ));
        }
        Ok(geometry)
    }
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
    finality: &SumeragiCommitVerifierV1,
    certificate: &[u8],
) -> Result<crate::p256::Point> {
    scheme
        .validate()
        .map_err(|e| Error::Rejected(e.to_string()))?;
    if finality.network().as_bytes() != &scheme.network_id {
        return Err(Error::AuthorityMismatch("parameter ledger root"));
    }
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
