//! Public-only build-time ordinary context codec. The sole DataModel Norito schema owns bytes.
//! This executable never signs, reads keys, constructs installed authority or grants money readiness.
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_MAX_V1, KagemushaOrdinaryInstalledContextV1,
};
use std::io::{self, Read as _, Write as _};
const MAX_JSON: usize = 1024 * 1024;
fn transform(mode: &str, input: &[u8]) -> Result<Vec<u8>, String> {
    match mode {
        "encode" => {
            if input.is_empty() || input.len() > MAX_JSON {
                return Err("public context JSON bound rejected".into());
            }
            let data: KagemushaOrdinaryInstalledContextV1 =
                norito::json::from_slice(input).map_err(|e| e.to_string())?;
            data.encode_original()
        }
        "inspect" => {
            let data = KagemushaOrdinaryInstalledContextV1::decode_original(input)?;
            norito::json::to_vec(&data).map_err(|e| e.to_string())
        }
        _ => Err("expected sole public context encode or inspect mode".into()),
    }
}
fn main() -> io::Result<()> {
    let mut args = std::env::args();
    args.next();
    let mode = args.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing public context codec mode",
        )
    })?;
    if args.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unexpected public context codec argument",
        ));
    }
    let maximum = match mode.as_str() {
        "encode" => MAX_JSON,
        "inspect" => KAGEMUSHA_ORDINARY_INSTALLED_CONTEXT_MAX_V1,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unknown public context codec mode",
            ));
        }
    };
    let mut input = Vec::new();
    io::stdin()
        .take(u64::try_from(maximum).expect("bounded transport") + 1)
        .read_to_end(&mut input)?;
    if input.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "public context input exceeds bound",
        ));
    }
    let output =
        transform(&mode, &input).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    io::stdout().write_all(&output)
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::kagemusha::{
        KagemushaOrdinaryInstalledContextFileV1, KagemushaOrdinaryInstalledContextLibraryV1,
    };
    // Pure known-public DATA. No role signature, installed root, account or financial grant exists.
    fn data() -> KagemushaOrdinaryInstalledContextV1 {
        KagemushaOrdinaryInstalledContextV1 {
            version: 1,
            purpose: 1,
            app_signer: [1; 32],
            runtime_signer: [2; 32],
            sdk_signer: [3; 32],
            source_policy_sha256: [4; 32],
            app_source_sha256: [5; 32],
            sdk_source_sha256: [6; 32],
            runtime_manifest_sha256: [7; 32],
            sdk_release_sha256: [8; 32],
            runtime_signature_domain: "unit:runtime:v1".into(),
            sdk_signature_domain: "unit:sdk:v1".into(),
            runtime_signature: [0; 64],
            sdk_signature: [0; 64],
            package_name: "unit.fixture".into(),
            version_code: 1,
            certificate_sha256: [9; 32],
            dex_sha256: [10; 32],
            native_abi: 25,
            libraries: vec![KagemushaOrdinaryInstalledContextLibraryV1 {
                abi: "arm64-v8a".into(),
                sha256: [11; 32],
            }],
            inventory_sha256: [12; 32],
            minimum_sequence: u64::MAX,
            recursive_profile_sha256: [13; 32],
            recursive_profile_size: 1,
            native_layout_digest: [13; 32],
            originals: vec![KagemushaOrdinaryInstalledContextFileV1 {
                path: "originals/authority-policy.norito".into(),
                sha256: [14; 32],
                byte_len: 1,
            }],
        }
    }
    #[test]
    fn public_context_codec_uses_exact_model_norito_and_full_u64() {
        let data = data();
        let json = norito::json::to_vec(&data).unwrap();
        let bytes = transform("encode", &json).unwrap();
        assert_eq!(bytes, data.encode_original().unwrap());
        let projection = transform("inspect", &bytes).unwrap();
        let decoded: KagemushaOrdinaryInstalledContextV1 =
            norito::json::from_slice(&projection).unwrap();
        assert_eq!(decoded, data);
        assert_eq!(transform("encode", &projection).unwrap(), bytes);
    }
    #[test]
    fn public_context_codec_refuses_material_modes_purpose_and_noncanonical_input() {
        for mode in ["sign", "root", "key-fd", ""] {
            assert!(transform(mode, b"{}").is_err());
        }
        assert!(transform("encode", b"{}").is_err());
        assert!(transform("encode", &vec![0; MAX_JSON + 1]).is_err());
        let mut data = data();
        data.purpose = 2;
        assert!(transform("encode", &norito::json::to_vec(&data).unwrap()).is_err());
        let mut canonical = self::data().encode_original().unwrap();
        canonical.push(0);
        assert!(transform("inspect", &canonical).is_err());
    }
}
