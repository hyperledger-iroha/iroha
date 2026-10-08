//! Explicit first-use initialization of authenticated server cache and proof custody.

use super::*;
use iroha_core_zk::kagemusha_wallet_finality_v1::server::{
    ServerFinalityCancellationV1, ServerFinalityLimitsV1, ServerFinalityStorageV1, ServerFinalityV1,
};

#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServerRequest {
    schema: String,
    chain_id: String,
    network_id_hex: String,
    genesis_public_key: String,
    signed_genesis: OriginalInput,
    scheme_id_hex: String,
    manifest_digest_hex: String,
    verifier_pack: OriginalInput,
    producer_inventory: OriginalInput,
    verifier_originals_directory: String,
    proving_cache_directory: String,
    journal_directory: String,
    maximum_key_bytes: u64,
    maximum_resident_proving_key_bytes: u64,
    maximum_original_bytes: u64,
    maximum_artifacts: usize,
    maximum_journal_entries: usize,
    maximum_journal_bytes: u64,
}
impl ServerRequest {
    fn limits(&self) -> io::Result<ServerFinalityLimitsV1> {
        let bounded = |value| {
            usize::try_from(value).map_err(|_| invalid("server bound unavailable on this host"))
        };
        Ok(ServerFinalityLimitsV1 {
            maximum_key_bytes: bounded(self.maximum_key_bytes)?,
            maximum_resident_proving_key_bytes: bounded(self.maximum_resident_proving_key_bytes)?,
            maximum_original_bytes: bounded(self.maximum_original_bytes)?,
            maximum_artifacts: self.maximum_artifacts,
            msm_bytes: 64 << 20,
            maximum_journal_entries: self.maximum_journal_entries,
            maximum_journal_bytes: self.maximum_journal_bytes,
        })
    }
}
fn parse(bytes: &[u8]) -> io::Result<ServerRequest> {
    let request: ServerRequest = checked(
        json::from_slice(bytes),
        "closed server initialization request required",
    )?;
    if request.schema != "iroha.kagemusha.finality-server-initialization.v1"
        || request.chain_id.is_empty()
        || request.chain_id.len() > 256
        || request.chain_id.chars().any(char::is_control)
        || request.maximum_key_bytes == 0
        || request.maximum_key_bytes > 1 << 30
        || request.maximum_resident_proving_key_bytes < request.maximum_key_bytes
        || request.maximum_resident_proving_key_bytes > 16 << 30
        || request.maximum_original_bytes < request.maximum_key_bytes
        || request.maximum_original_bytes > 1 << 40
        || !(1..=65_536).contains(&request.maximum_artifacts)
        || !(3..=1_000_000).contains(&request.maximum_journal_entries)
        || request.maximum_journal_bytes == 0
        || request.maximum_journal_bytes > 1 << 40
    {
        return Err(invalid(
            "invalid server initialization policy or finite limits",
        ));
    }
    for value in [
        &request.network_id_hex,
        &request.scheme_id_hex,
        &request.manifest_digest_hex,
        &request.signed_genesis.sha256,
        &request.verifier_pack.sha256,
        &request.producer_inventory.sha256,
    ] {
        digest(value)?;
    }
    for path in [
        &request.verifier_originals_directory,
        &request.proving_cache_directory,
        &request.journal_directory,
    ] {
        if !Path::new(path).is_absolute() {
            return Err(invalid("server storage must be an absolute private path"));
        }
    }
    request.limits()?;
    Ok(request)
}

pub fn run(path: &str) -> io::Result<()> {
    let request_original = Original::open(path, REQUEST_MAX, None)?;
    let request = parse(&request_original.bytes)?;
    let genesis = Original::open(
        &request.signed_genesis.path,
        GENESIS_MAX,
        Some(digest(&request.signed_genesis.sha256)?),
    )?;
    let pack = Original::open(
        &request.verifier_pack.path,
        iroha_core_zk::kagemusha_wallet_artifacts_v1::VERIFIER_PACK_MAX_BYTES_V1,
        Some(digest(&request.verifier_pack.sha256)?),
    )?;
    let inventory = Original::open(
        &request.producer_inventory.path,
        INVENTORY_MAX,
        Some(digest(&request.producer_inventory.sha256)?),
    )?;
    let native = native_finality(
        &request.chain_id,
        &request.network_id_hex,
        &request.genesis_public_key,
        &genesis.bytes,
    )?;
    for original in [&request_original, &genesis, &pack, &inventory] {
        original.recheck()?;
    }
    eprintln!(
        "Reconstructing authenticated finality metadata, initializing fresh server storage, and strictly importing the complete graph."
    );
    let server = ServerFinalityV1::initialize(
        &native,
        InstallationV1 {
            scheme_id: digest(&request.scheme_id_hex)?,
            manifest_digest: digest(&request.manifest_digest_hex)?,
        },
        &pack.bytes,
        &inventory.bytes,
        ServerFinalityStorageV1 {
            verifier_originals: Path::new(&request.verifier_originals_directory),
            proving_cache: Path::new(&request.proving_cache_directory),
            journal: Path::new(&request.journal_directory),
        },
        request.limits()?,
        ServerFinalityCancellationV1::default(),
    )
    .map_err(io::Error::other)?;
    for original in [&request_original, &genesis, &pack, &inventory] {
        original.recheck()?;
    }
    drop(server);
    println!(
        "Server selections initialized and complete graph strictly imported. No live finality proof was produced."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Vec<u8> {
        format!(r#"{{"schema":"iroha.kagemusha.finality-server-initialization.v1","chain_id":"actual selected chain","network_id_hex":"{h}","genesis_public_key":"selected","signed_genesis":{{"path":"/private/genesis","sha256":"{h}"}},"scheme_id_hex":"{h}","manifest_digest_hex":"{h}","verifier_pack":{{"path":"/private/pack","sha256":"{h}"}},"producer_inventory":{{"path":"/private/inventory","sha256":"{h}"}},"verifier_originals_directory":"/private/dv","proving_cache_directory":"/private/cache","journal_directory":"/private/journal","maximum_key_bytes":268435456,"maximum_resident_proving_key_bytes":536870912,"maximum_original_bytes":549755813888,"maximum_artifacts":4096,"maximum_journal_entries":10000,"maximum_journal_bytes":1073741824}}"#, h="01".repeat(32)).into_bytes()
    }
    #[test]
    fn closed_initializer_requires_finite_separate_selected_inputs() {
        let bytes = request();
        assert_eq!(parse(&bytes).unwrap().limits().unwrap().msm_bytes, 64 << 20);
        let text = String::from_utf8(bytes).unwrap();
        for changed in [
            text.replacen('{', "{\"adopt_missing_selection\":true,", 1),
            text.replace("536870912", "1"),
            text.replace("549755813888", "18446744073709551615"),
            text.replace("/private/cache", "../cache"),
            text.replace("actual selected chain", ""),
            text.replace(&"01".repeat(32), &"00".repeat(32)),
        ] {
            assert!(parse(changed.as_bytes()).is_err());
        }
    }
}
