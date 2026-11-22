use super::inspect_spartan_bundle::read_profile;
use super::*;
use zk_spartan::native::MAX_PROGRAM_BUNDLE_BYTES;
#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ReleaseScope {
    schema: String,
    scheme_original: String,
    scheme_id_hex: String,
    signed_genesis: String,
    genesis_public_key: String,
    chain_id: String,
}

pub(super) enum ManifestAction<'a> {
    Prepare {
        certificate: &'a str,
    },
    Verify {
        manifest: &'a str,
        signature: &'a str,
        digest: &'a str,
    },
}

pub(super) fn manifest(
    bundle: &str,
    profile: &str,
    scope: &str,
    action: ManifestAction<'_>,
) -> io::Result<()> {
    use iroha_data_model::{
        isi::kagemusha_wallet::spartan::InstallKagemushaSpartanProgramsV1,
        sumeragi_finality::SumeragiCommitVerifierV1,
    };
    let scope_file = Original::open(scope, REQUEST_MAX, None)?;
    let scope: ReleaseScope = checked(
        json::from_slice(&scope_file.bytes),
        "five-kind release scope",
    )?;
    if scope.schema != "iroha.kagemusha.spartan.manifest-scope.v5" {
        return Err(invalid("unsupported five-kind manifest scope"));
    }
    let scheme_file = Original::open(
        &scope.scheme_original,
        KAGEMUSHA_WALLET_SCHEME_MAX_BYTES_V1,
        None,
    )?;
    let scheme = KagemushaWalletSchemeV1::decode_canonical(
        &scheme_file.bytes,
        &digest(&scope.scheme_id_hex)?,
    )
    .map_err(io::Error::other)?;
    let genesis = Original::open(&scope.signed_genesis, GENESIS_MAX, None)?;
    let finality = native_finality(
        &scope.chain_id,
        &hex::encode(scheme.network_id),
        &scope.genesis_public_key,
        &genesis.bytes,
    )?;
    let finality = SumeragiCommitVerifierV1::new(&finality).map_err(io::Error::other)?;
    let (profile_file, compiler) = read_profile(profile)?;
    let bundle = Original::open(bundle, MAX_PROGRAM_BUNDLE_BYTES, None)?;
    let output = match action {
        ManifestAction::Prepare { certificate } => {
            let certificate =
                Original::open(certificate, KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1, None)?;
            let (manifest, message) = compiler
                .prepare_manifest_for_signing(&bundle.bytes, &scheme, &finality, &certificate.bytes)
                .map_err(io::Error::other)?;
            certificate.recheck()?;
            format!(
                "unsigned_manifest_hex={}\nartifact_signing_message_hex={}",
                hex::encode(manifest),
                hex::encode(message)
            )
        }
        ManifestAction::Verify {
            manifest,
            signature,
            digest: selected,
        } => {
            let manifest = Original::open(
                manifest,
                InstallKagemushaSpartanProgramsV1::MAX_MANIFEST_BYTES,
                None,
            )?;
            let signature = Original::open(signature, 64, None)?;
            let bytes: &[u8; 64] = signature
                .bytes
                .as_slice()
                .try_into()
                .map_err(|_| invalid("expected raw 64-byte Artifact signature"))?;
            let selected = digest(selected)?;
            compiler
                .verify_signed_originals(
                    &bundle.bytes,
                    &manifest.bytes,
                    bytes,
                    &selected,
                    &scheme,
                    &finality,
                )
                .map_err(io::Error::other)?;
            manifest.recheck()?;
            signature.recheck()?;
            format!(
                "verified_manifest_digest={} program_bundle_version=2 kinds=5",
                hex::encode(selected)
            )
        }
    };
    for original in [&scope_file, &scheme_file, &genesis, &profile_file, &bundle] {
        original.recheck()?;
    }
    println!("{output}");
    Ok(())
}
