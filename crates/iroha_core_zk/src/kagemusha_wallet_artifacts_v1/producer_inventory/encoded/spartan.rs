//! Single-original wallet carrier for the governed Spartan program installation.
use super::*;
use iroha_data_model::{
    isi::kagemusha_wallet::spartan::InstallKagemushaSpartanProgramsV1 as Install,
    kagemusha::KagemushaWalletSchemeV1,
};
use zk_spartan::native::{
    self, AuthenticatedManifestHeaderV5, AuthenticatedParameters, ParameterManifest,
    ProgramProfileV2,
};

/// Maximum fixed-layout carrier header, including the bounded signed program manifest.
pub const MAX_PACK_BYTES: usize = Install::MAX_MANIFEST_BYTES + 2048;
/// Bounded fixed-layout inventory of the selected producer programs and signed manifest.
pub const MAX_INVENTORY_BYTES: usize = 4096;
/// Maximum length of either single-original JSON metadata document.
pub const MAX_JSON_BYTES: usize = 2048;

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.spartan.wallet.CarrierPack.v1")]
struct Pack {
    scheme: [u8; 32],
    asset: [u8; 32],
    manifest_digest: [u8; 32],
    manifest: Vec<u8>,
    signature: [u8; 64],
    original: BlobV1,
    catalog_digest: [u8; 32],
}

/// Native producer's eight closed compiler outputs. Device callers obtain these
/// from their native build, independently of the signed verifier installation.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha.kagemusha.spartan.wallet.ProducerPrograms.v4")]
pub struct ProducerPrograms {
    /// Primary curve program digests, ordered Receive, Send, Bootstrap, Control.
    pub primary: [[u8; 32]; 4],
    /// Secondary curve program digests in the same order.
    pub secondary: [[u8; 32]; 4],
}

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.spartan.wallet.ProducerInventory.v1")]
struct Inventory {
    programs: ProducerPrograms,
    manifest_digest: [u8; 32],
    original: BlobV1,
}

/// Four carrier pins covered by the signed application release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleasePins {
    /// Exact pack length and SHA-256.
    pub pack: BlobV1,
    /// Exact inventory length and SHA-256.
    pub inventory: BlobV1,
    /// Domain-separated digest of the inventory.
    pub catalog_digest: [u8; 32],
    /// Exact stored transport metadata length and SHA-256.
    pub transport: BlobV1,
}

fn framed<T: norito::NoritoSerialize>(value: &T) -> io::Result<Vec<u8>> {
    let _layout = norito::core::WireLayoutGuard::enter(norito::core::WireLayout::Fixed);
    norito::to_bytes(value).map_err(io::Error::other)
}

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.spartan.wallet.ProducerInventory.v2")]
struct FiveInventory {
    programs: ProgramProfileV2,
    manifest_digest: [u8; 32],
    original: BlobV1,
}

/// Compiler pins selected independently of the signed verifier originals.
#[derive(Clone)]
pub enum CompiledPrograms {
    /// The compiled Receive, Send, Bootstrap and Control programs.
    Four(ProducerPrograms),
    /// The ten compiled tapes and deferred matrix dimensions for the ArchiveSent generation.
    Five(ProgramProfileV2),
}

impl From<&ProducerPrograms> for CompiledPrograms {
    fn from(programs: &ProducerPrograms) -> Self {
        Self::Four(*programs)
    }
}

impl From<&CompiledPrograms> for CompiledPrograms {
    fn from(programs: &CompiledPrograms) -> Self {
        programs.clone()
    }
}

impl CompiledPrograms {
    /// Select the native compiler registry for this manifest schema.
    pub fn for_manifest(bytes: &[u8]) -> Option<Self> {
        if AuthenticatedParameters::is_five_manifest(bytes) {
            native::built_in_five_programs().map(Self::Five)
        } else {
            Self::four()
        }
    }

    fn four() -> Option<Self> {
        native::built_in_producer_programs()
            .map(|(primary, secondary)| Self::Four(ProducerPrograms { primary, secondary }))
    }
}

/// Public Artifact-signed claims from the selected version of the wallet pack.
pub enum AuthenticatedPackManifest {
    /// Authenticated four-kind parameter and issuer claims.
    Four(ParameterManifest),
    /// Authenticated five-kind parameter and issuer claims.
    Five(AuthenticatedManifestHeaderV5),
}

impl AuthenticatedPackManifest {
    /// Root-certified signer original selected by the monetary manifest.
    pub fn signer_certificate(&self) -> &[u8] {
        match self {
            Self::Four(manifest) => &manifest.signer_certificate,
            Self::Five(manifest) => &manifest.signer_certificate,
        }
    }

    /// Look up the build's own producer registry independently of these claims.
    pub fn built_in_producer(&self) -> Option<CompiledPrograms> {
        match self {
            Self::Four(_) => CompiledPrograms::four(),
            Self::Five(_) => native::built_in_five_programs().map(CompiledPrograms::Five),
        }
    }

    fn authenticate(
        bytes: &[u8],
        signature: &[u8; 64],
        digest: &[u8; 32],
        scheme: &KagemushaWalletSchemeV1,
    ) -> io::Result<Self> {
        if AuthenticatedParameters::is_five_manifest(bytes) {
            ProgramProfileV2::authenticate_manifest_header(bytes, signature, digest, scheme)
                .map(Self::Five)
                .map_err(io::Error::other)
        } else {
            let (manifest, actual) =
                AuthenticatedParameters::authenticated_manifest(bytes, signature, scheme)
                    .map_err(io::Error::other)?;
            if actual != *digest {
                return Err(invalid("Spartan signed manifest digest"));
            }
            Ok(Self::Four(manifest))
        }
    }

    fn require_compiler(&self, producer: &CompiledPrograms) -> io::Result<()> {
        let matches = match (self, producer) {
            (Self::Four(m), CompiledPrograms::Four(p)) => {
                m.primary == p.primary && m.secondary == p.secondary
            }
            (Self::Five(m), CompiledPrograms::Five(p)) => m.programs == *p,
            _ => false,
        };
        if !matches {
            return Err(invalid("installed programs differ from native producer"));
        }
        Ok(())
    }

    /// Import the selected public program originals under the independent native compiler pins.
    ///
    /// # Errors
    /// Refuses mismatched compiler pins, malformed programs or invalid manifest claims.
    pub fn verify_originals(&self, bytes: &[u8], producer: &CompiledPrograms) -> io::Result<()> {
        self.require_compiler(producer)?;
        match self {
            Self::Four(m) => AuthenticatedParameters::verify_program_originals(bytes, m),
            Self::Five(m) => m.programs.verify_compiled_originals(bytes, m).map(|_| ()),
        }
        .map_err(io::Error::other)
    }
}

fn inventory(
    original: BlobV1,
    manifest_digest: [u8; 32],
    programs: &CompiledPrograms,
) -> io::Result<Vec<u8>> {
    match programs {
        CompiledPrograms::Four(programs) => framed(&Inventory {
            original,
            manifest_digest,
            programs: *programs,
        }),
        CompiledPrograms::Five(programs) => framed(&FiveInventory {
            original,
            manifest_digest,
            programs: programs.clone(),
        }),
    }
}

fn require_world_extent(install: &Install) -> io::Result<()> {
    if install.manifest.is_empty()
        || install.manifest.len() > Install::MAX_MANIFEST_BYTES
        || install.programs.is_empty()
        || install.programs.len() > Install::MAX_BYTES
        || install.asset == [0; 32]
        || install.scheme == [0; 32]
        || install.manifest_digest == [0; 32]
    {
        return Err(invalid("Spartan World installation bounds"));
    }
    let _layout = norito::core::WireLayoutGuard::enter(norito::core::WireLayout::Fixed);
    if norito::core::encoded_frame_len(install).map_err(io::Error::other)? > Install::MAX_BYTES {
        return Err(invalid("Spartan World installation extent"));
    }
    Ok(())
}

fn require_closure(directory: &PrivateDirectory, address: BlobV1) -> io::Result<()> {
    let expected = std::ffi::OsString::from(hex::encode(address.sha256));
    if directory.entries(2)? != [expected] {
        return Err(invalid(
            "Spartan carrier must contain one exact program original",
        ));
    }
    Ok(())
}

fn measure(lengths: [usize; 4], row: EncodedOriginalV1) -> io::Result<()> {
    let caps = [
        MAX_PACK_BYTES,
        MAX_INVENTORY_BYTES,
        MAX_JSON_BYTES,
        MAX_JSON_BYTES,
    ];
    if lengths.iter().zip(caps).any(|(&length, cap)| length > cap) {
        return Err(invalid("Spartan carrier metadata exceeds role limit"));
    }
    let total = lengths
        .into_iter()
        .try_fold(row.stored.bytes, |total, bytes| {
            total.checked_add(bytes as u64)
        });
    if row.original.bytes > Install::MAX_BYTES as u64
        || total.is_none_or(|n| n > Install::MAX_BYTES as u64)
    {
        return Err(invalid("Spartan wallet carrier exceeds aggregate limit"));
    }
    Ok(())
}

/// Publish the five wallet entries from the exact World installation. Returns the identities for the application-release signer.
///
/// # Errors
/// Returns an error for a mismatched scope, signature, compiled profile, aggregate
/// extent, source identity or directory custody. Exact retries reuse immutable entries.
pub fn publish(
    install: &Install,
    scheme: &KagemushaWalletSchemeV1,
    producer: impl Into<CompiledPrograms>,
    destination: &PrivateDirectory,
) -> io::Result<ReleasePins> {
    let producer = producer.into();
    require_world_extent(install)?;
    if install.scheme != scheme.scheme_id() {
        return Err(invalid("Spartan publisher scope"));
    }
    let manifest = AuthenticatedPackManifest::authenticate(
        &install.manifest,
        &install.signature,
        &install.manifest_digest,
        scheme,
    )?;
    manifest.verify_originals(&install.programs, &producer)?;
    publish_files(install, install.manifest_digest, &producer, destination)
}

fn publish_files(
    install: &Install,
    digest: [u8; 32],
    producer: impl Into<CompiledPrograms>,
    destination: &PrivateDirectory,
) -> io::Result<ReleasePins> {
    let original = BlobV1::of(&install.programs);
    let catalog = inventory(original, digest, &producer.into())?;
    let catalog_digest = artifact_digest(b"producer-catalog", &catalog);
    let pack = framed(&Pack {
        scheme: install.scheme,
        asset: install.asset,
        manifest_digest: digest,
        manifest: install.manifest.clone(),
        signature: install.signature,
        original,
        catalog_digest,
    })?;
    require_carrier_entries(destination, false)?;
    let target = destination.ensure_child("wallet-originals")?;
    let row = encode_stream(&mut install.programs.as_slice(), &target, original)?;
    require_closure(&target, original)?;
    let transport = EncodedTransportV1::new(vec![row])?.to_json();
    let financial =
        financial_json_for_scope(&pack, &catalog, &transport, Some((install.asset, digest)))?;
    measure(
        [pack.len(), catalog.len(), transport.len(), financial.len()],
        row,
    )?;
    for (name, bytes) in [
        ("verifier-pack.norito", &pack),
        ("producer-inventory.norito", &catalog),
        ("transport.json", &transport),
        ("financial-originals.json", &financial),
    ] {
        super::super::transport_metadata::publish_metadata(destination, name, bytes)?;
    }
    require_carrier_entries(destination, true)?;
    target.revalidate()?;
    destination.sync()?;
    destination.revalidate()?;
    Ok(ReleasePins {
        pack: BlobV1::of(&pack),
        inventory: BlobV1::of(&catalog),
        catalog_digest,
        transport: BlobV1::of(&transport),
    })
}

fn authenticated_header(
    bytes: &[u8],
    scheme: &KagemushaWalletSchemeV1,
    asset: [u8; 32],
) -> io::Result<(Pack, AuthenticatedPackManifest)> {
    if bytes.is_empty() || bytes.len() > MAX_PACK_BYTES {
        return Err(invalid("Spartan carrier header extent"));
    }
    let header: Pack = norito::decode_canonical_with_layout(bytes, norito::core::WireLayout::Fixed)
        .map_err(io::Error::other)?;
    if header.scheme != scheme.scheme_id()
        || header.asset != asset
        || asset == [0; 32]
        || header.catalog_digest == [0; 32]
        || header.original.bytes == 0
        || header.original.bytes > Install::MAX_BYTES as u64
        || header.original.sha256 == [0; 32]
        || header.manifest.is_empty()
        || header.manifest.len() > Install::MAX_MANIFEST_BYTES
    {
        return Err(invalid("Spartan pack selection or original extent"));
    }
    let manifest = AuthenticatedPackManifest::authenticate(
        &header.manifest,
        &header.signature,
        &header.manifest_digest,
        scheme,
    )?;
    Ok((header, manifest))
}

/// Authenticate the bounded pack offered in a signed service or phone runtime.
/// The caller independently selects the profile, asset, exact pack and expected
/// manifest digest. This inspection alone grants no proving or settlement authority.
///
/// # Errors
/// Returns an error for noncanonical framing, mismatched scope, bad certificate,
/// bad signature, an invalid original extent or a different manifest digest.
pub fn authenticated_pack_selection(
    bytes: &[u8],
    scheme: &KagemushaWalletSchemeV1,
    asset: [u8; 32],
) -> io::Result<(ParameterManifest, [u8; 32])> {
    match authenticated_pack_selection_any(bytes, scheme, asset)? {
        (AuthenticatedPackManifest::Four(manifest), digest) => Ok((manifest, digest)),
        _ => Err(invalid("carrier requires five-kind manifest selection")),
    }
}

/// Authenticate the bounded pack and its signed manifest across both supported schemas.
///
/// # Errors
/// Refuses a changed signer, scope, digest, asset, signature or noncanonical pack.
pub fn authenticated_pack_selection_any(
    bytes: &[u8],
    scheme: &KagemushaWalletSchemeV1,
    asset: [u8; 32],
) -> io::Result<(AuthenticatedPackManifest, [u8; 32])> {
    let (header, manifest) = authenticated_header(bytes, scheme, asset)?;
    Ok((manifest, header.manifest_digest))
}

/// Reconstruct the exact governed installation from an installed carrier, signed release pins,
/// registered asset, separately selected manifest and the device build's compiler outputs.
/// Run ordinary program installation with the authenticated genesis before acquiring custody.
///
/// # Errors
/// Returns an error for any change to the signed selection, signer, compiled producer
/// identity, encoding, original closure, aggregate size or retained file.
pub fn read(
    root: &PrivateDirectory,
    release: ReleasePins,
    scheme: &KagemushaWalletSchemeV1,
    asset: [u8; 32],
    manifest_digest: [u8; 32],
    producer: impl Into<CompiledPrograms>,
) -> io::Result<Install> {
    read_using(root, release, scheme, asset, manifest_digest, |_| {
        Ok(producer.into())
    })
    .map(|(install, _)| install)
}

/// An authenticated generation has no independently verified compiler registry in this native build.
#[derive(Clone, Copy, Debug)]
pub struct CompilerUnavailable;

impl std::fmt::Display for CompilerUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native compiler registry unavailable")
    }
}

impl std::error::Error for CompilerUnavailable {}

/// Reconstruct the release and select the installed native compiler registry.
/// Returns both so the caller can mount the identical selected program set.
///
/// # Errors
/// In addition to ordinary carrier refusals, returns Unsupported when this
/// native build has no verified compiler registry for the authenticated schema.
pub fn read_from_build(
    root: &PrivateDirectory,
    release: ReleasePins,
    scheme: &KagemushaWalletSchemeV1,
    asset: [u8; 32],
    manifest_digest: [u8; 32],
) -> io::Result<(Install, CompiledPrograms)> {
    read_using(root, release, scheme, asset, manifest_digest, |claims| {
        claims
            .built_in_producer()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, CompilerUnavailable))
    })
}

fn read_using(
    root: &PrivateDirectory,
    release: ReleasePins,
    scheme: &KagemushaWalletSchemeV1,
    asset: [u8; 32],
    manifest_digest: [u8; 32],
    select: impl FnOnce(&AuthenticatedPackManifest) -> io::Result<CompiledPrograms>,
) -> io::Result<(Install, CompiledPrograms)> {
    require_carrier_entries(root, true)?;
    let pack = root.read("verifier-pack.norito", MAX_PACK_BYTES)?;
    let catalog = root.read("producer-inventory.norito", MAX_INVENTORY_BYTES)?;
    let transport = root.read("transport.json", MAX_JSON_BYTES)?;
    let financial = root.read("financial-originals.json", MAX_JSON_BYTES)?;
    if BlobV1::of(&pack) != release.pack
        || BlobV1::of(&catalog) != release.inventory
        || BlobV1::of(&transport) != release.transport
        || artifact_digest(b"producer-catalog", &catalog) != release.catalog_digest
        || financial_json_for_scope(&pack, &catalog, &transport, Some((asset, manifest_digest)))?
            != *financial
    {
        return Err(invalid("Spartan carrier differs from signed release"));
    }
    let (header, manifest) = authenticated_header(&pack, scheme, asset)?;
    if header.manifest_digest != manifest_digest {
        return Err(invalid("Spartan selected manifest digest"));
    }
    if header.catalog_digest != release.catalog_digest {
        return Err(invalid("Spartan carrier authority mismatch"));
    }
    let producer = select(&manifest)?;
    let digest = header.manifest_digest;
    if inventory(header.original, digest, &producer)? != *catalog {
        return Err(invalid("Spartan carrier authority mismatch"));
    }
    manifest.require_compiler(&producer)?;
    let transport_rows = EncodedTransportV1::from_json(&transport)?;
    let [row] = transport_rows.originals() else {
        return Err(invalid("Spartan carrier has extra transport rows"));
    };
    if row.original != header.original {
        return Err(invalid(
            "Spartan transported programs differ from selected inventory",
        ));
    }
    measure(
        [pack.len(), catalog.len(), transport.len(), financial.len()],
        *row,
    )?;
    let target = root.open_child("wallet-originals")?;
    require_closure(&target, header.original)?;
    let source = EncodedDirectoryOriginalsV1::open_existing(target.path(), &transport_rows)?;
    let mut programs = Vec::new();
    programs
        .try_reserve_exact(header.original.bytes as usize)
        .map_err(io::Error::other)?;
    source
        .open_original(header.original.sha256)
        .map_err(io::Error::other)?
        .read_to_end(&mut programs)?;
    if BlobV1::of(&programs) != header.original {
        return Err(invalid("Spartan program original differs"));
    }
    require_closure(&target, header.original)?;
    require_carrier_entries(root, true)?;
    target.revalidate()?;
    root.revalidate()?;
    let result = Install {
        scheme: header.scheme,
        asset,
        manifest_digest: digest,
        manifest: header.manifest,
        signature: header.signature,
        programs,
    };
    require_world_extent(&result)?;
    Ok((result, producer))
}

/// Check the app build's freshly compiled bundle against the complete signed carrier.
/// The caller supplies the scheme, asset and manifest selected by the application.
///
/// # Errors
/// Refuses a malformed carrier, a different producer or any change to the compiled bytes.
pub fn verify_compiled_bundle(
    root: &PrivateDirectory,
    scheme: &KagemushaWalletSchemeV1,
    asset: [u8; 32],
    manifest_digest: [u8; 32],
    producer: impl Into<CompiledPrograms>,
    compiled: &[u8],
) -> io::Result<()> {
    let producer = producer.into();
    let catalog = root.read("producer-inventory.norito", MAX_INVENTORY_BYTES)?;
    let pins = ReleasePins {
        catalog_digest: artifact_digest(b"producer-catalog", &catalog),
        inventory: BlobV1::of(&catalog),
        pack: BlobV1::of(&root.read("verifier-pack.norito", MAX_PACK_BYTES)?),
        transport: BlobV1::of(&root.read("transport.json", MAX_JSON_BYTES)?),
    };
    let install = read(root, pins, scheme, asset, manifest_digest, &producer)?;
    if install.programs != compiled {
        return Err(invalid(
            "signed programs differ from the fresh compiler output",
        ));
    }
    let manifest = AuthenticatedPackManifest::authenticate(
        &install.manifest,
        &install.signature,
        &install.manifest_digest,
        scheme,
    )?;
    manifest.verify_originals(compiled, &producer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{kagemusha::*, prelude::NetworkId};
    use p256::ecdsa::{Signature, SigningKey, signature::Signer};

    fn signed_install() -> (Install, KagemushaWalletSchemeV1, ProducerPrograms) {
        fn public(key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
            KagemushaDevicePublicKeyV1::try_from(
                key.verifying_key().to_sec1_point(false).as_bytes(),
            )
            .unwrap()
        }
        fn sign(key: &SigningKey, msg: &[u8; 32]) -> [u8; 64] {
            let sig: Signature = key.sign(msg);
            sig.normalize_s().to_bytes().into()
        }
        let root = SigningKey::from_slice(&[0x19; 32]).unwrap();
        let artifact = SigningKey::from_slice(&[0x21; 32]).unwrap();
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"carrier-test"),
        ));
        let scheme = KagemushaWalletSchemeV1 {
            version: 1,
            network_id: *network.as_bytes(),
            scheme_root_key: public(&root),
            relation_id: [9; 32],
            provider_contract: kagemusha_wallet_provider_contract_v1(),
        };
        let cert = KagemushaWalletSignerCertificateBodyV1 {
            version: 1,
            scheme_id: scheme.scheme_id(),
            role: KagemushaWalletSignerRoleV1::Artifact,
            key: public(&artifact),
            serial: 5,
        };
        let mut certificate = cert.transcript();
        certificate.extend_from_slice(&sign(&root, &cert.signing_message()));
        let producer = ProducerPrograms {
            primary: [[3; 32]; 4],
            secondary: [[4; 32]; 4],
        };
        let manifest = ParameterManifest {
            version: zk_spartan::native::PARAMETER_MANIFEST_VERSION,
            scheme: scheme.scheme_id(),
            relation: scheme.relation_id,
            primary: producer.primary,
            secondary: producer.secondary,
            dimensions_p: [3, 4],
            dimensions_s: [3, 4],
            generator_domain: "test".into(),
            hash_to_curve: "test".into(),
            generator_capacity_bits: [3; 2],
            generator_digests: [[2; 32]; 2],
            table_construction: "test".into(),
            table_vector: [[3; 32]; 2],
            finality_network: network.to_string(),
            finality_chain: "carrier-test".into(),
            signer_certificate: certificate,
        };
        let manifest = framed(&manifest).unwrap();
        let digest =
            kagemusha_wallet_poseidon_bytes_v1(u64::from_le_bytes(*b"kgwparm1"), &manifest);
        (
            Install {
                scheme: scheme.scheme_id(),
                asset: [5; 32],
                manifest_digest: digest,
                signature: sign(&artifact, &digest),
                manifest,
                programs: vec![17; 32768],
            },
            scheme,
            producer,
        )
    }

    #[test]
    fn carrier_reconstructs_exact_world_row_and_closes_release_and_directory() {
        let temp = tempfile::tempdir().unwrap();
        let root =
            PrivateDirectory::open_or_create(temp.path().canonicalize().unwrap().join("carrier"))
                .unwrap();
        let (install, scheme, producer) = signed_install();
        // This transport test deliberately excludes compiler import. The publisher must refuse the synthetic tape.
        assert!(publish(&install, &scheme, &producer, &root).is_err());
        let pins = publish_files(&install, install.manifest_digest, &producer, &root).unwrap();
        assert_eq!(
            pins,
            publish_files(&install, install.manifest_digest, &producer, &root).unwrap()
        );
        let load = |pins, asset, digest, producer: &ProducerPrograms| {
            read(&root, pins, &scheme, asset, digest, producer)
        };
        assert_eq!(
            install,
            load(pins, install.asset, install.manifest_digest, &producer).unwrap()
        );
        // Equal carrier bytes alone cannot establish that the compiler produced valid programs.
        for candidate in [
            install.programs.as_slice(),
            b"different compilation".as_slice(),
        ] {
            assert!(
                verify_compiled_bundle(
                    &root,
                    &scheme,
                    install.asset,
                    install.manifest_digest,
                    &producer,
                    candidate,
                )
                .is_err()
            );
        }
        assert!(load(pins, [6; 32], install.manifest_digest, &producer).is_err());
        assert!(load(pins, install.asset, [1; 32], &producer).is_err());
        // A repinned pack can still name the wrong catalog. Reject it before
        // selecting a producer, even on a device without that compiler.
        let crossed =
            PrivateDirectory::open_or_create(root.path().with_file_name("crossed")).unwrap();
        crossed.ensure_child("wallet-originals").unwrap();
        let pack = root.read("verifier-pack.norito", MAX_PACK_BYTES).unwrap();
        let (mut header, _) = authenticated_header(&pack, &scheme, install.asset).unwrap();
        header.catalog_digest[0] ^= 1;
        let pack = framed(&header).unwrap();
        let catalog = root
            .read("producer-inventory.norito", MAX_INVENTORY_BYTES)
            .unwrap();
        let transport = root.read("transport.json", MAX_JSON_BYTES).unwrap();
        let financial = financial_json_for_scope(
            &pack,
            &catalog,
            &transport,
            Some((install.asset, install.manifest_digest)),
        )
        .unwrap();
        for (name, data) in [
            ("verifier-pack.norito", pack.as_slice()),
            ("producer-inventory.norito", &catalog),
            ("transport.json", &transport),
            ("financial-originals.json", &financial),
        ] {
            super::super::super::transport_metadata::publish_metadata(&crossed, name, data)
                .unwrap();
        }
        let crossed_pins = ReleasePins {
            pack: BlobV1::of(&pack),
            ..pins
        };
        let error = read_using(
            &crossed,
            crossed_pins,
            &scheme,
            install.asset,
            install.manifest_digest,
            |_| panic!("producer lookup preceded the catalog identity check"),
        )
        .err()
        .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        let mut wrong_pins = pins;
        wrong_pins.catalog_digest[0] ^= 1;
        assert!(
            load(
                wrong_pins,
                install.asset,
                install.manifest_digest,
                &producer
            )
            .is_err()
        );
        let mut other = producer.clone();
        other.secondary[1][2] ^= 1;
        assert!(load(pins, install.asset, install.manifest_digest, &other).is_err());
        let originals = root.open_child("wallet-originals").unwrap();
        assert_eq!(originals.entries(2).unwrap().len(), 1);
        super::super::super::transport_metadata::publish_metadata(
            &originals,
            "stray",
            b"unselected",
        )
        .unwrap();
        assert!(load(pins, install.asset, install.manifest_digest, &producer).is_err());
    }

    #[test]
    #[ignore = "requires the independently pinned public five-kind manifest/producer test fixture"]
    fn five_kind_carrier_authenticates_all_ten_pins_and_cross_version_refusals() {
        let fixtures = std::path::PathBuf::from(
            std::env::var_os("KAGEMUSHA_SPARTAN_FIVE_CARRIER_FIXTURE").unwrap(),
        );
        let read_fixture = |name: &str| std::fs::read(fixtures.join(name)).unwrap();
        let identity = read_fixture("identity.bin");
        let scheme_id: [u8; 32] = identity[..32].try_into().unwrap();
        let scheme =
            KagemushaWalletSchemeV1::decode_canonical(&read_fixture("scheme.norito"), &scheme_id)
                .unwrap();
        let install = Install {
            scheme: scheme_id,
            asset: [7; 32],
            manifest_digest: identity[32..64].try_into().unwrap(),
            manifest: read_fixture("manifest.norito"),
            signature: read_fixture("signature.bin").try_into().unwrap(),
            programs: read_fixture("program-bundle.norito"),
        };
        let pins = read_fixture("producer-pins.bin");
        assert_eq!(pins.len(), 10 * 32);
        let side = |offset: usize| {
            std::array::from_fn(|i| {
                pins[32 * (offset + i)..32 * (offset + i + 1)]
                    .try_into()
                    .unwrap()
            })
        };
        let producer = ProgramProfileV2 {
            primary: side(0),
            secondary: side(5),
            dimensions: [[24, 25], [24, 24]],
        };
        let selected = CompiledPrograms::Five(producer.clone());
        let tmp = tempfile::tempdir().unwrap();
        let destination = std::env::var_os("KAGEMUSHA_SPARTAN_FIVE_CARRIER_OUTPUT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| tmp.path().canonicalize().unwrap().join("carrier"));
        let root = PrivateDirectory::open_or_create(destination).unwrap();
        let release = publish(&install, &scheme, &selected, &root).unwrap();
        let restored = read(
            &root,
            release,
            &scheme,
            install.asset,
            install.manifest_digest,
            &selected,
        )
        .unwrap();
        assert_eq!(restored, install);
        // The synthetic tapes can be compared with explicit independent test pins.
        // They can never supply the device build's own monetary producer selection.
        let unavailable = read_from_build(
            &root,
            release,
            &scheme,
            install.asset,
            install.manifest_digest,
        )
        .err()
        .expect("the synthetic compiler fixture is not an installed monetary generation");
        if native::built_in_five_programs().is_none() {
            assert!(
                unavailable
                    .get_ref()
                    .is_some_and(|error| error.is::<CompilerUnavailable>())
            );
        } else {
            assert_eq!(unavailable.kind(), io::ErrorKind::InvalidData);
        }
        let pack = root.read("verifier-pack.norito", MAX_PACK_BYTES).unwrap();
        let (manifest, digest) =
            authenticated_pack_selection_any(&pack, &scheme, install.asset).unwrap();
        assert!(matches!(manifest, AuthenticatedPackManifest::Five(_)));
        assert_eq!(digest, install.manifest_digest);
        assert_eq!(
            manifest.signer_certificate(),
            read_fixture("certificate.bin")
        );
        assert!(authenticated_pack_selection(&pack, &scheme, install.asset).is_err());
        verify_compiled_bundle(
            &root,
            &scheme,
            install.asset,
            digest,
            &selected,
            &install.programs,
        )
        .unwrap();
        let four = signed_install().2;
        assert!(read(&root, release, &scheme, install.asset, digest, &four).is_err());
        let mut alternate = producer.clone();
        alternate.secondary.swap(3, 4);
        assert!(
            read(
                &root,
                release,
                &scheme,
                install.asset,
                digest,
                &CompiledPrograms::Five(alternate)
            )
            .is_err()
        );
        let mut wrong_dimensions = producer;
        wrong_dimensions.dimensions[0][1] -= 1;
        assert!(
            read(
                &root,
                release,
                &scheme,
                install.asset,
                digest,
                &CompiledPrograms::Five(wrong_dimensions)
            )
            .is_err()
        );
        let catalog = root
            .read("producer-inventory.norito", MAX_INVENTORY_BYTES)
            .unwrap();
        assert!(!catalog.is_empty());
        eprintln!(
            "five-kind inventory {} bytes, pack {} bytes, exact program original {} bytes",
            catalog.len(),
            pack.len(),
            install.programs.len()
        );
    }

    #[test]
    fn all_four_metadata_lengths_count_towards_carrier_limit() {
        let mut row = EncodedOriginalV1 {
            original: BlobV1::of(b"a"),
            stored: BlobV1::of(b"b"),
        };
        assert!(measure([5170, 2536, 303, 642], row).is_ok());
        assert!(measure([5170, MAX_INVENTORY_BYTES + 1, 303, 642], row).is_err());
        assert!(measure([5170, 2536, MAX_JSON_BYTES + 1, 642], row).is_err());
        row.stored.bytes = Install::MAX_BYTES as u64 - 100;
        assert!(measure([25; 4], row).is_ok());
        assert!(measure([25, 25, 25, 26], row).is_err());
        row.original.bytes = Install::MAX_BYTES as u64 + 1;
        assert!(measure([1; 4], row).is_err());
    }
}
