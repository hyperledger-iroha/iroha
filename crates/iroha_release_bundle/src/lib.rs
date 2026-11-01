//! Canonical native release-bundle format and retained executable custody.
//!
//! The release owner supplies the authenticated source and manifest anchors. Filesystem
//! admission and content authentication remain separate requirements; a parsed manifest
//! alone never grants authority to launch a program.

pub mod bundle;
pub mod execution;
pub mod manifest;
pub mod publish;

#[cfg(test)]
mod tests;

// TODO: Qualify the integrated publisher, continuous source keeper and retained process
// consumers with actual shipping artifacts on each native host; component tests are insufficient.
