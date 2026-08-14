//! Adversarial signature, key, numeric, and confidential-input admission tests.
use once_cell::sync::OnceCell;
use pyo3::{
    Python,
    types::{PyBytes, PyDict},
};
use super::*;
fn ensure_python() {
    static INIT: OnceCell<()> = OnceCell::new();
    INIT.get_or_init(|| {
        Python::initialize();
    });
}
#[test]
fn sorafs_orderbook_owner_account_validation_enforces_v1_byte_ceiling() {
    ensure_python();
    assert!(
        validate_sorafs_orderbook_owner_account_py(&[0x45; ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1])
            .is_ok()
    );
    assert!(
        validate_sorafs_orderbook_owner_account_py(
            &[0x45; ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 + 1]
        )
        .is_err()
    );
}
fn py_err_message(err: pyo3::PyErr) -> String {
    ensure_python();
    Python::attach(|py| err.value(py).to_string())
}
#[test]
fn asset_numeric_scale_adapter_rejects_values_outside_numeric_v1() {
    assert_eq!(
        numeric_spec_from_optional_scale(Some(iroha_primitives::numeric::MAX_DECIMAL_SCALE))
            .expect("maximum Numeric V1 scale")
            .scale(),
        Some(iroha_primitives::numeric::MAX_DECIMAL_SCALE)
    );
    assert!(
        numeric_spec_from_optional_scale(Some(iroha_primitives::numeric::MAX_DECIMAL_SCALE + 1))
            .is_err(),
        "runtime-supplied scale 29 must be a Python error, never a panic"
    );
    assert_eq!(
        numeric_spec_from_optional_scale(None)
            .expect("unconstrained numeric specification")
            .scale(),
        None
    );
}
const MALFORMED_ED25519_PUBLIC_KEYS: [(&str, [u8; 32], &str); 3] = [
    ("all-zero", [0u8; 32], "all zero"),
    (
        "small-order",
        [
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ],
        "small-order",
    ),
    (
        "noncanonical",
        [
            0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0x7f,
        ],
        "non-canonical",
    ),
];
#[test]
fn checked_fallback_signature_from_bytes_rejects_empty_and_all_zero_payloads() {
    let empty = py_err_message(
        checked_signature_from_bytes_for_algorithm(&[], Algorithm::BlsNormal, "signature")
            .expect_err("empty signature must fail"),
    );
    assert!(
        empty.contains("signature is malformed")
            && empty.contains("signature payload must not be empty"),
        "unexpected empty-signature error: {empty}"
    );
    let all_zero = py_err_message(
        checked_signature_from_bytes_for_algorithm(&[0u8; 64], Algorithm::BlsNormal, "signature")
            .expect_err("all-zero signature must fail"),
    );
    assert!(
        all_zero.contains("signature is malformed")
            && all_zero.contains("signature payload must not be all zero"),
        "unexpected all-zero signature error: {all_zero}"
    );
    let accepted =
        checked_signature_from_bytes_for_algorithm(&[0x42; 64], Algorithm::BlsNormal, "signature")
            .expect("nonzero opaque signature material is admitted for backend verification");
    assert_eq!(accepted.payload(), &[0x42; 64]);
}
#[test]
fn checked_ed25519_signature_from_bytes_rejects_malformed_r_before_backend() {
    const SMALL_ORDER_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    let key_pair = KeyPair::try_from_seed(
        b"python-wallet-ed25519-signature-r-admission".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("derive checked Ed25519 wallet fixture keypair");
    let signature = Signature::try_new(
        key_pair.private_key(),
        b"python wallet Ed25519 signature admission",
    )
    .expect("checked wallet fixture signature");
    checked_signature_from_bytes_for_algorithm(
        signature.payload(),
        Algorithm::Ed25519,
        "signature",
    )
    .expect("valid Ed25519 signature material is admitted");
    for (label, replacement_r) in [
        ("small-order", SMALL_ORDER_R),
        ("noncanonical", NONCANONICAL_R),
    ] {
        let mut malformed = signature.payload().to_vec();
        malformed[..32].copy_from_slice(&replacement_r);
        let err = py_err_message(
            checked_signature_from_bytes_for_algorithm(&malformed, Algorithm::Ed25519, "signature")
                .expect_err("malformed Ed25519 R must fail admission"),
        );
        assert!(
            err.contains("signature is malformed"),
            "unexpected {label} R admission error: {err}"
        );
    }
}
#[test]
fn checked_mldsa_signature_from_bytes_rejects_malformed_lengths_before_backend() {
    let key_pair = KeyPair::try_from_seed(
        b"python-wallet-mldsa-signature-admission".to_vec(),
        Algorithm::MlDsa,
    )
    .expect("derive checked ML-DSA wallet fixture keypair");
    let signature = Signature::try_new(
        key_pair.private_key(),
        b"python wallet ML-DSA signature admission",
    )
    .expect("checked ML-DSA wallet fixture signature");
    checked_signature_from_bytes_for_algorithm(signature.payload(), Algorithm::MlDsa, "signature")
        .expect("valid ML-DSA signature material is admitted");
    let mut short = signature.payload().to_vec();
    short.pop();
    let mut overlong = signature.payload().to_vec();
    overlong.push(0x42);
    for (label, malformed) in [
        ("short", short),
        ("overlong", overlong),
        ("all-zero", vec![0_u8; signature.payload().len()]),
    ] {
        let err = py_err_message(
            checked_signature_from_bytes_for_algorithm(&malformed, Algorithm::MlDsa, "signature")
                .expect_err("malformed ML-DSA signature must fail admission"),
        );
        assert!(
            err.contains("signature is malformed"),
            "unexpected {label} ML-DSA signature admission error: {err}"
        );
    }
}
#[test]
fn parse_wallet_signature_rejects_malformed_ed25519_r_before_storage() {
    const SMALL_ORDER_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    let key_pair = KeyPair::try_from_seed(
        b"python-connect-wallet-ed25519-r-admission".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("derive checked Connect wallet fixture keypair");
    let signature = Signature::try_new(
        key_pair.private_key(),
        b"connect wallet signature admission",
    )
    .expect("checked Connect wallet fixture signature");
    ensure_python();
    Python::attach(|py| {
        let fields = PyDict::new(py);
        fields
            .set_item("signature", PyBytes::new(py, signature.payload()))
            .expect("set valid wallet signature");
        parse_wallet_signature(&fields).expect("valid wallet signature parses");
        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_R),
            ("noncanonical", NONCANONICAL_R),
        ] {
            let mut malformed = signature.payload().to_vec();
            malformed[..32].copy_from_slice(&replacement_r);
            fields
                .set_item("signature", PyBytes::new(py, &malformed))
                .expect("set malformed wallet signature");
            let err = match parse_wallet_signature(&fields) {
                Ok(_) => panic!("{label} Ed25519 R unexpectedly parsed"),
                Err(err) => err,
            };
            let message = err.value(py).to_string();
            assert!(
                message.contains("approve.signature is malformed"),
                "unexpected {label} R parser error: {message}"
            );
        }
    });
}
#[test]
fn verify_ed25519_rejects_malformed_signature_r_before_backend() {
    const SMALL_ORDER_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    let key_pair = KeyPair::try_from_seed(
        b"python-native-ed25519-signature-r-admission".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("derive checked Ed25519 fixture keypair");
    let (_, public_key) = public_key_to_bytes(key_pair.public_key(), "fixture public key")
        .expect("fixture public key bytes");
    let message = b"python native Ed25519 signature admission";
    let signature =
        Signature::try_new(key_pair.private_key(), message).expect("checked fixture signature");
    assert!(
        verify_py(
            Algorithm::Ed25519.as_static_str(),
            public_key,
            message,
            signature.payload(),
        )
        .expect("generic Ed25519 verification returns a bool"),
        "valid Ed25519 signature must verify through generic wrapper"
    );
    assert!(
        verify_ed25519_py(public_key, message, signature.payload())
            .expect("Ed25519 verification returns a bool"),
        "valid Ed25519 signature must verify through Ed25519 wrapper"
    );
    for (label, replacement_r) in [
        ("small-order", SMALL_ORDER_R),
        ("noncanonical", NONCANONICAL_R),
    ] {
        let mut malformed = signature.payload().to_vec();
        malformed[..32].copy_from_slice(&replacement_r);
        assert!(
            !verify_py(
                Algorithm::Ed25519.as_static_str(),
                public_key,
                message,
                &malformed,
            )
            .expect("generic Ed25519 verification returns a bool"),
            "{label} Ed25519 signature R must fail generic wrapper admission"
        );
        assert!(
            !verify_ed25519_py(public_key, message, &malformed)
                .expect("Ed25519 verification returns a bool"),
            "{label} Ed25519 signature R must fail Ed25519 wrapper admission"
        );
    }
}
#[test]
fn verify_rejects_malformed_mldsa_signature_lengths_before_backend() {
    let key_pair = KeyPair::try_from_seed(
        b"python-native-mldsa-signature-admission".to_vec(),
        Algorithm::MlDsa,
    )
    .expect("derive checked ML-DSA fixture keypair");
    let (_, public_key) = public_key_to_bytes(key_pair.public_key(), "fixture public key")
        .expect("fixture public key bytes");
    let message = b"python native ML-DSA signature admission";
    let signature = Signature::try_new(key_pair.private_key(), message)
        .expect("checked ML-DSA fixture signature");
    assert!(
        verify_py(
            Algorithm::MlDsa.as_static_str(),
            public_key,
            message,
            signature.payload(),
        )
        .expect("generic ML-DSA verification returns a bool"),
        "valid ML-DSA signature must verify through generic wrapper"
    );
    let mut short = signature.payload().to_vec();
    short.pop();
    let mut overlong = signature.payload().to_vec();
    overlong.push(0x42);
    for (label, malformed) in [
        ("short", short),
        ("overlong", overlong),
        ("all-zero", vec![0_u8; signature.payload().len()]),
    ] {
        assert!(
            !verify_py(
                Algorithm::MlDsa.as_static_str(),
                public_key,
                message,
                &malformed,
            )
            .expect("generic ML-DSA verification returns a bool"),
            "{label} ML-DSA signature must fail generic wrapper admission"
        );
    }
}
#[test]
fn verify_rejects_malformed_ed25519_public_key_material_before_backend() {
    let key_pair = KeyPair::try_from_seed(
        b"python-native-ed25519-public-key-admission".to_vec(),
        Algorithm::Ed25519,
    )
    .expect("derive checked Ed25519 fixture keypair");
    let message = b"python native Ed25519 public key admission";
    let signature =
        Signature::try_new(key_pair.private_key(), message).expect("checked fixture signature");
    for (label, public_key, expected_error) in MALFORMED_ED25519_PUBLIC_KEYS {
        let generic = py_err_message(
            verify_py(
                Algorithm::Ed25519.as_static_str(),
                &public_key,
                message,
                signature.payload(),
            )
            .expect_err("generic verifier must reject malformed Ed25519 public keys"),
        );
        assert!(
            generic.contains("failed to parse public key"),
            "unexpected generic verifier {label} public-key error: {generic}"
        );
        assert!(
            generic.contains(expected_error),
            "generic verifier {label} public-key error lost parser detail: {generic}"
        );
        let ed25519 = py_err_message(
            verify_ed25519_py(&public_key, message, signature.payload())
                .expect_err("Ed25519 verifier must reject malformed public keys"),
        );
        assert!(
            ed25519.contains("failed to parse public key"),
            "unexpected Ed25519 verifier {label} public-key error: {ed25519}"
        );
        assert!(
            ed25519.contains(expected_error),
            "Ed25519 verifier {label} public-key error lost parser detail: {ed25519}"
        );
    }
}
#[test]
fn python_confidential_transfer_input_requires_canonical_diversifier() {
    ensure_python();
    Python::attach(|py| {
        let input_with_diversifier = |key: Option<&str>| {
            let input = PyDict::new(py);
            input.set_item("amount", "7").expect("amount field");
            input
                .set_item("rho", PyBytes::new(py, &[0x51; 32]))
                .expect("rho field");
            if let Some(key) = key {
                input
                    .set_item(key, PyBytes::new(py, &[0x52; 32]))
                    .expect("diversifier field");
            }
            input
        };
        let parsed = parse_confidential_transfer_input_py(
            input_with_diversifier(Some("diversifier")).as_any(),
            0,
        )
        .expect("canonical diversifier accepted");
        assert_eq!(parsed.diversifier, [0x52; 32]);
        let missing =
            parse_confidential_transfer_input_py(input_with_diversifier(None).as_any(), 0)
                .expect_err("missing diversifier rejected");
        assert!(
            missing
                .value(py)
                .to_string()
                .contains("inputs[0].diversifier is required"),
            "unexpected missing-diversifier error: {}",
            missing.value(py)
        );
        for alias in ["diversifier_hex", "diversifierHex"] {
            let err = parse_confidential_transfer_input_py(
                input_with_diversifier(Some(alias)).as_any(),
                0,
            )
            .expect_err("retired diversifier alias rejected");
            assert!(
                err.value(py)
                    .to_string()
                    .contains("inputs[0].diversifier must use canonical diversifier"),
                "unexpected alias error for {alias}: {}",
                err.value(py)
            );
        }
    });
}
