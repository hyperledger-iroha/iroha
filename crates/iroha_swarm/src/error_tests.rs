//! Exact user-facing diagnostics for swarm, path and schema failures.

use crate::{Error, path, schema};

struct FailingWriter;

impl std::fmt::Write for FailingWriter {
    fn write_str(&mut self, _: &str) -> std::fmt::Result {
        Err(std::fmt::Error)
    }
}

fn assert_diagnostic(error: &impl std::error::Error, expected: &str) {
    assert_eq!(error.to_string(), expected);
    // Diagnostics render their complete messages without inheriting outer format flags.
    assert_eq!(format!("{error:*>180.3}"), expected);
    assert_eq!(format!("{error:#}"), expected);
    assert_eq!(format!("{error:+020}"), expected);
    // Wrapped diagnostics remain self-contained; these errors do not expose sources.
    assert!(error.source().is_none());
    assert!(std::fmt::write(&mut FailingWriter, format_args!("{error}")).is_err());
}

#[test]
fn swarm_error_messages() {
    let cases = [
        (
            Error::InvalidPeerCount { actual: 0 },
            "Peer count (0) must form an exact Sumeragi `3f + 1` committee (4..=31).",
        ),
        (
            Error::TargetFileIsADirectory,
            "Target file path points to a directory.",
        ),
        (Error::NoTargetDirectory, "Target directory not found."),
        (
            Error::InvalidPeerOverrideCount {
                expected: 4,
                actual: 7,
            },
            "Peer overrides count (7) does not match the requested peer count (4).",
        ),
        (
            Error::KeyGeneration("entropy {unavailable}\n次".into()),
            "Failed to generate cryptographic swarm material: entropy {unavailable}\n次",
        ),
        (
            Error::EmptyDevelopmentSeed,
            "Deterministic development mode requires a non-empty seed.",
        ),
        (
            Error::EmptyPreparedValidators,
            "A prepared validator bundle must contain at least one validator.",
        ),
        (
            Error::PreparedValidatorCountOverflow { actual: 65_536 },
            "Prepared validator count (65536) exceeds the supported u16 range.",
        ),
        (
            Error::InvalidPreparedValidatorPop {
                index: 3,
                reason: "invalid proof".into(),
            },
            "Prepared validator 3 has an invalid BLS proof of possession: invalid proof",
        ),
        (
            Error::InvalidPreparedTransportIdentity {
                index: 2,
                reason: "wrong algorithm".into(),
            },
            "Prepared validator 2 has an invalid SoraNet transport identity: wrong algorithm.",
        ),
        (
            Error::DuplicatePreparedTransportIdentity { index: 1 },
            "Prepared validator 1 repeats another validator's SoraNet transport identity.",
        ),
        (
            Error::InvalidPreparedRuntimeTarget {
                index: 9,
                target: "/outside".into(),
            },
            "Prepared validator 9 runtime file target `/outside` must be a normalized path below `/config/runtime`.",
        ),
        (
            Error::DuplicatePreparedRuntimeTarget {
                index: 8,
                target: "/config/runtime/a".into(),
            },
            "Prepared validator 8 repeats runtime file target `/config/runtime/a`.",
        ),
        (
            Error::OverlappingPreparedRuntimeTarget {
                index: 7,
                target: "/config/runtime/a/b".into(),
                other: "/config/runtime/a".into(),
            },
            "Prepared validator 7 runtime file target `/config/runtime/a/b` overlaps `/config/runtime/a`.",
        ),
        (
            Error::InvalidPreparedSecretTarget {
                index: 6,
                target: "relative`file".into(),
            },
            "Prepared validator 6 secret target `relative`file` must be one portable file below `/run/secrets`.",
        ),
        (
            Error::DuplicatePreparedSecretTarget {
                index: 5,
                target: "/run/secrets/key".into(),
            },
            "Prepared validator 5 repeats secret target `/run/secrets/key`.",
        ),
        (
            Error::InvalidPeerPorts {
                index: 4,
                p2p_port: 0,
                api_port: 65_535,
            },
            "Peer 4 must use distinct non-zero P2P/API ports, got 0/65535.",
        ),
        (
            Error::DuplicateHostPort {
                index: 3,
                port: 8080,
            },
            "Peer 3 reuses host port 8080.",
        ),
        (
            Error::InvalidPeerServiceName {
                index: 65_535,
                name: "invalid name".into(),
            },
            "Peer 65535 has invalid Compose service name `invalid name`.",
        ),
        (
            Error::DuplicatePeerServiceName {
                name: "peer0".into(),
            },
            "Compose service name `peer0` is used by more than one peer.",
        ),
        (
            Error::PathConversion(path::Error::AbsolutePath(std::io::Error::other(
                "path unavailable",
            ))),
            "Failed to construct an absolute path: path unavailable",
        ),
        (
            Error::PathConversion(path::Error::RelativePath),
            "Failed to construct a relative path.",
        ),
    ];
    for (error, expected) in cases {
        assert_diagnostic(&error, expected);
    }
}

#[test]
fn path_error_messages() {
    assert_diagnostic(
        &path::Error::AbsolutePath(std::io::Error::other("unavailable {cwd}\n次")),
        "Failed to construct an absolute path: unavailable {cwd}\n次",
    );
    assert_diagnostic(
        &path::Error::RelativePath,
        "Failed to construct a relative path.",
    );
}

#[test]
fn schema_error_messages() {
    assert_diagnostic(
        &schema::Error::BannerWrite(std::io::Error::other("writer closed")),
        "Could not write the banner: writer closed",
    );
    assert_diagnostic(
        &schema::Error::Yaml(norito::yaml::Error::Io(std::io::Error::other("disk full"))),
        "Could not serialize the schema: YAML IO error: disk full",
    );
    assert_diagnostic(
        &schema::Error::Yaml(norito::yaml::Error::Json(norito::Error::Message(
            "bad {schema}".into(),
        ))),
        "Could not serialize the schema: YAML JSON conversion error: bad {schema}",
    );
}
