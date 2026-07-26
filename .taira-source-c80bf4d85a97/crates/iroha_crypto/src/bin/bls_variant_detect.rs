//! Dev-only CLI: Detect BLS verification flavor on this machine.
//!
//! Prints which ciphersuite flavor matches the local build for both
//! BLS Normal (pk in G1, sig in G2) and BLS Small (pk in G2, sig in G1):
//! - CONCAT: `MESSAGE_CONTEXT` || message hashed-to-curve (RO)
//! - AUG:    message hashed-to-curve with `MESSAGE_CONTEXT` as augmentation (`RO_AUG`)
//!
//! Build (requires blstrs backend):
//!   cargo run -p `iroha_crypto` --features "bls bls-backend-blstrs" --bin bls-variant-detect

#![allow(clippy::print_stdout)]

use std::error::Error;

use iroha_crypto::{BlsNormal, BlsSmall, KeyGenOption};
use w3f_bls::SerializableToBytes as _;

const MESSAGE_CONTEXT: &[u8; 20] = b"for signing messages";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flavor {
    Concat,
    Aug,
    None,
    Both,
}

// no blstrs-based helpers needed; use w3f-bls detection only

fn detect_normal(message: &[u8], signature: &[u8], pk_bytes: &[u8]) -> Flavor {
    // Use w3f-bls to match semantics precisely (bytes + ciphersuite)
    let sig = if let Ok(s) = w3f_bls::Signature::<w3f_bls::ZBLS>::from_bytes(signature) {
        s
    } else {
        return Flavor::None;
    };
    let pk = if let Ok(p) = w3f_bls::PublicKey::<w3f_bls::ZBLS>::from_bytes(pk_bytes) {
        p
    } else {
        return Flavor::None;
    };
    let ok_concat = {
        let msg = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
        sig.verify(&msg, &pk)
    };
    // Approximate AUG by pre-pending pk bytes; should fail for our ciphersuite
    let ok_aug = {
        let mut buf = Vec::with_capacity(pk_bytes.len() + message.len());
        buf.extend_from_slice(pk_bytes);
        buf.extend_from_slice(message);
        let msg = w3f_bls::Message::new(MESSAGE_CONTEXT, &buf);
        sig.verify(&msg, &pk)
    };

    match (ok_concat, ok_aug) {
        (true, false) => Flavor::Concat,
        (false, true) => Flavor::Aug,
        (true, true) => Flavor::Both,
        (false, false) => Flavor::None,
    }
}

fn detect_small(message: &[u8], signature: &[u8], pk_bytes: &[u8]) -> Flavor {
    let sig = if let Ok(s) = w3f_bls::Signature::<w3f_bls::TinyBLS381>::from_bytes(signature) {
        s
    } else {
        return Flavor::None;
    };
    let pk = if let Ok(p) = w3f_bls::PublicKey::<w3f_bls::TinyBLS381>::from_bytes(pk_bytes) {
        p
    } else {
        return Flavor::None;
    };
    let ok_concat = {
        let msg = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
        sig.verify(&msg, &pk)
    };
    let ok_aug = {
        let mut buf = Vec::with_capacity(pk_bytes.len() + message.len());
        buf.extend_from_slice(pk_bytes);
        buf.extend_from_slice(message);
        let msg = w3f_bls::Message::new(MESSAGE_CONTEXT, &buf);
        sig.verify(&msg, &pk)
    };

    match (ok_concat, ok_aug) {
        (true, false) => Flavor::Concat,
        (false, true) => Flavor::Aug,
        (true, true) => Flavor::Both,
        (false, false) => Flavor::None,
    }
}

fn banner() -> &'static str {
    // Simple retro-styled banner with a Japanese vibe (Shift-JIS era aesthetics)
    r"
┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
┃      ■ I R O H A   B L S   V A R I A N T   D E T E C T O R ■       ┃
┃      ■ イ　ロ　ハ　■ ボネ・リン・シャチャム署名 ■                         ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
"
}

fn print_result(title: &str, flavor: Flavor) {
    let (mark, label) = match flavor {
        Flavor::Concat => ("◎", "CONCAT (ctx || msg)"),
        Flavor::Aug => ("○", "AUG (ctx as aug)"),
        Flavor::Both => ("◇", "BOTH match (ambiguous)"),
        Flavor::None => ("×", "NO match (invalid?)"),
    };
    println!("  ▸ {title:<8} => {mark}  {label}");
}

struct DetectionReport {
    normal_flavor: Flavor,
    small_flavor: Flavor,
    normal_lib_ok: bool,
    small_lib_ok: bool,
}

fn build_detection_report() -> Result<DetectionReport, Box<dyn Error>> {
    // Deterministic seeds/messages for stable output
    let msg_normal = "昭和SIGN-ノーマル".as_bytes(); // "Showa SIGN - normal"
    let msg_small = "昭和SIGN-スモール".as_bytes(); // "Showa SIGN - small"

    // Normal (G1 pk, G2 sig)
    let (pk_n, sk_n) = BlsNormal::keypair(KeyGenOption::UseSeed(vec![7; 16]))
        .map_err(|err| format!("BLS normal keypair: {err}"))?;
    let sig_n =
        BlsNormal::sign(msg_normal, &sk_n).map_err(|err| format!("BLS normal sign: {err}"))?;
    let pk_n_bytes = pk_n.to_bytes();
    let normal_lib_ok = BlsNormal::verify(msg_normal, &sig_n, &pk_n).is_ok();
    let normal_flavor = detect_normal(msg_normal, &sig_n, &pk_n_bytes);

    // Small (G2 pk, G1 sig)
    let (pk_s, sk_s) = BlsSmall::keypair(KeyGenOption::UseSeed(vec![9; 24]))
        .map_err(|err| format!("BLS small keypair: {err}"))?;
    let sig_s = BlsSmall::sign(msg_small, &sk_s).map_err(|err| format!("BLS small sign: {err}"))?;
    let pk_small_bytes = pk_s.to_bytes();
    let small_lib_ok = BlsSmall::verify(msg_small, &sig_s, &pk_s).is_ok();
    let small_flavor = detect_small(msg_small, &sig_s, &pk_small_bytes);

    Ok(DetectionReport {
        normal_flavor,
        small_flavor,
        normal_lib_ok,
        small_lib_ok,
    })
}

fn validate_detection_report(report: &DetectionReport) -> Result<(), Box<dyn Error>> {
    if !report.normal_lib_ok {
        return Err("BLS normal generated signature failed library verification".into());
    }
    if !report.small_lib_ok {
        return Err("BLS small generated signature failed library verification".into());
    }
    validate_flavor("NORMAL", report.normal_flavor)?;
    validate_flavor("SMALL", report.small_flavor)?;
    Ok(())
}

fn validate_flavor(title: &str, flavor: Flavor) -> Result<(), Box<dyn Error>> {
    match flavor {
        Flavor::Concat => Ok(()),
        Flavor::Aug => Err(format!("{title} detected unsupported AUG BLS flavor").into()),
        Flavor::Both => Err(format!("{title} detected ambiguous BLS flavor").into()),
        Flavor::None => Err(format!("{title} detected no compatible BLS flavor").into()),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("{}", banner());
    println!(
        "  構成: bls-backend-blstrs = {}, bls-multi-pairing = {}",
        if cfg!(feature = "bls-backend-blstrs") {
            "ON"
        } else {
            "OFF"
        },
        if cfg!(feature = "bls-multi-pairing") {
            "ON"
        } else {
            "OFF"
        },
    );
    println!("  ───────────────────────────────────────────────────────────────");

    let report = build_detection_report()?;
    print_result("NORMAL", report.normal_flavor);
    print_result("SMALL", report.small_flavor);

    println!("  ───────────────────────────────────────────────────────────────");
    println!(
        "  [LIB  ] verify: NORMAL={}, SMALL={}",
        if report.normal_lib_ok { "OK" } else { "FAIL" },
        if report.small_lib_ok { "OK" } else { "FAIL" }
    );
    println!("  ヒント: CONCAT が有効なら Iroha の現行実装と一致しています。");
    println!("        AUG は代替ciphersuiteです (互換検査用)。");
    validate_detection_report(&report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_bls_variants_detect_concat_flavor() {
        let report = build_detection_report().expect("deterministic BLS report");

        assert!(report.normal_lib_ok);
        assert!(report.small_lib_ok);
        assert_eq!(report.normal_flavor, Flavor::Concat);
        assert_eq!(report.small_flavor, Flavor::Concat);
        validate_detection_report(&report).expect("CONCAT report must validate");
    }

    #[test]
    fn detector_validation_rejects_ambiguous_or_missing_flavors() {
        for flavor in [Flavor::Aug, Flavor::Both, Flavor::None] {
            let report = DetectionReport {
                normal_flavor: flavor,
                small_flavor: Flavor::Concat,
                normal_lib_ok: true,
                small_lib_ok: true,
            };

            let err = validate_detection_report(&report)
                .expect_err("non-CONCAT normal flavor must fail closed");
            assert!(
                err.to_string().contains("NORMAL"),
                "unexpected error: {err}"
            );
        }
    }

    #[test]
    fn detector_validation_rejects_library_verify_failure() {
        let report = DetectionReport {
            normal_flavor: Flavor::Concat,
            small_flavor: Flavor::Concat,
            normal_lib_ok: false,
            small_lib_ok: true,
        };
        let err =
            validate_detection_report(&report).expect_err("library verify failure must be fatal");

        assert!(
            err.to_string().contains("library verification"),
            "unexpected error: {err}"
        );
    }
}
