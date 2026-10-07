//! Fixed mobile shared-library policy, including immutable public application trust.

fn main() {
    wallet_runtime_trust();
    if matches!(
        std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("android")
    ) {
        println!("cargo:rustc-link-arg-cdylib=-Wl,-z,max-page-size=16384");
    }
}
fn parse_wallet_runtime_trust(value: Option<&str>) -> Result<Option<[u8; 32]>, &'static str> {
    let Some(value) = value else { return Ok(None) };
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || value.bytes().all(|byte| byte == b'0')
    {
        return Err("expected one nonzero lowercase 32-byte Ed25519 public key");
    }
    let mut key = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        key[index] =
            u8::from_str_radix(std::str::from_utf8(pair).expect("validated ASCII hex"), 16)
                .expect("validated lowercase hex");
    }
    Ok(Some(key))
}
fn wallet_runtime_trust() {
    const INPUT: &str = "MOBILE_SDK_WALLET_RUNTIME_TRUST_ED25519_HEX";
    println!("cargo:rerun-if-env-changed={INPUT}");
    let value = match std::env::var(INPUT) {
        Err(std::env::VarError::NotPresent) => None,
        Err(_) => panic!("{INPUT} is not valid UTF-8"),
        Ok(value) => Some(value),
    };
    let key = parse_wallet_runtime_trust(value.as_deref())
        .unwrap_or_else(|error| panic!("{INPUT}: {error}"));
    let source = key.map_or_else(|| "None".to_owned(), |key| format!("Some({key:?})"));
    let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"))
        .join("kagemusha_wallet_runtime_trust.rs");
    std::fs::write(path, format!("// Immutable CBSI application-release public key selected by the native artifact build owner.\nconst INSTALLED_RUNTIME_TRUST: Option<[u8; 32]> = {source};\n"))
        .expect("write immutable native wallet installation policy");
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_public_build_selection_disables_original_installation() {
        assert_eq!(parse_wallet_runtime_trust(None), Ok(None));
    }
    #[test]
    fn configured_malformed_public_selection_refuses_the_native_build() {
        for key in [
            "".to_owned(),
            "0".repeat(64),
            "A".repeat(64),
            "g".repeat(64),
            "1".repeat(63),
            "1".repeat(65),
            format!("{}\n", "1".repeat(64)),
        ] {
            assert!(parse_wallet_runtime_trust(Some(&key)).is_err());
        }
    }
    #[test]
    fn native_build_selection_retains_exact_public_key_bytes() {
        let selected = (0u8..32)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            parse_wallet_runtime_trust(Some(&selected)),
            Ok(Some(std::array::from_fn(|index| index as u8)))
        );
    }
}
