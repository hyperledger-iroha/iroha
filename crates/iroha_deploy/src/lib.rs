//! Deployment engine for Iroha networks and dataspaces.
//!
//! One engine backs `iroha network` and `iroha dataspace`: it reads a
//! hand-written definition, compares it with observed host and chain facts, and
//! converges hosts with idempotent actions. The design and its guarantees are
//! specified in `specs/network_deployment.md`.
//!
//! This first phase provides [`definition`]: typed, validated parsers for the
//! network definition (`networks/<name>.toml`) and the dataspace definition
//! (`dataspaces/<name>.toml`), read through `iroha_config_base` so that every
//! error names its file and key and unknown keys are rejected.
//!
//! [`verify`] holds the verification gates. So far it has [`verify::finality`],
//! the light finality verifier behind gate G5. It is anchored in an
//! authenticated signed genesis or a stored `SumeragiFinalityCheckpoint`,
//! verifies every contiguous certified successor with the data model's native
//! verifier, including each epoch handoff, and requires fresh challenge-bound
//! attestations from `2f + 1` members of the authenticated committee.
//!
//! Later phases add planning and the decision hash, the converge executor and
//! its journal, the local, container and SSH drivers, and the remaining
//! verification gates.
// TODO: P2 adds `plan`, `converge`, the local driver, the HTTP finality source
// and gates G0-G8/G11; P3 adds the SSH driver and edge renderers
// (specs/network_deployment.md §13).

pub mod definition;
pub mod verify;
