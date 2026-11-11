//! The boundaries of the universal root's history (`specs/runtime_upgrades.md`, "Checkpoint
//! records"): read them, have a node sign its statement of the adoption anchor or of a startup
//! floor, and relay every validator's statements.
//!
//! A node derives the record from, or checks it against, its own certified history before it
//! signs, with its validator key, which never leaves it. The statements are the authority of a
//! record, so any account relays them; the kernel checks that they cover the current committee.

use std::{collections::BTreeSet, path::PathBuf};

use eyre::{Result, eyre};
use iroha::data_model::{
    isi::{
        InstructionBox,
        runtime_boundary::{RaiseRuntimeStartupFloorV1, RatifyRuntimeAdoptionAnchorV1},
    },
    runtime::{
        adoption::SignedAdoptionAttestationV1,
        checkpoint::{RuntimeAdoptionAnchorV1, SignedStartupFloorAttestationV1, StartupFloorV1},
        lifecycle::AdoptionRecordV1,
    },
};

use super::read_json;
use crate::{Run, RunContext};

/// Read the root's history boundaries, and collect and relay validators' statements of them.
#[derive(clap::Subcommand, Debug)]
pub enum BoundaryCommand {
    /// Show the root's checkpoint policy, adoption anchor, startup floors and history boundary,
    /// and the upgrade checkpoint and checkpoints this node holds
    List,
    /// Have the node derive the adoption record from its own store and print the record with its
    /// signed statement; save one per validator for `ratify-anchor`
    AnchorStatement,
    /// Ratify the adoption anchor with the statement of every validator of the current committee
    RatifyAnchor(RatifyAnchorArgs),
    /// Have the node state the startup floor at a checkpoint height it holds and print the floor
    /// with its signed statement; save one per validator for `raise-floor`
    FloorStatement(FloorStatementArgs),
    /// Raise the startup floor with the statement of every validator of the current committee
    RaiseFloor(RaiseFloorArgs),
}

impl Run for BoundaryCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            BoundaryCommand::List => {
                let client = context.client_from_config()?;
                let value = client.get_runtime_boundaries_json()?;
                context.print_data(&value)
            }
            BoundaryCommand::AnchorStatement => {
                let client = context.client_from_config()?;
                let value = client.post_runtime_attest_anchor_json()?;
                context.print_data(&value)
            }
            BoundaryCommand::RatifyAnchor(args) => args.run(context),
            BoundaryCommand::FloorStatement(args) => {
                let client = context.client_from_config()?;
                let value = client.post_runtime_attest_floor_json(args.height)?;
                context.print_data(&value)
            }
            BoundaryCommand::RaiseFloor(args) => args.run(context),
        }
    }
}

/// One node's statement of the adoption anchor, as `anchor-statement` prints it.
#[derive(norito::derive::JsonDeserialize)]
struct AnchorStatement {
    record: AdoptionRecordV1,
    attestation: SignedAdoptionAttestationV1,
}

/// One node's statement of a startup floor, as `floor-statement` prints it.
#[derive(norito::derive::JsonDeserialize)]
struct FloorStatement {
    floor: StartupFloorV1,
    attestation: SignedStartupFloorAttestationV1,
}

/// Arguments of `ratify-anchor`.
#[derive(clap::Args, Debug)]
pub struct RatifyAnchorArgs {
    /// Path to one node's `anchor-statement` output; repeat for every validator
    #[arg(long = "statement", value_name = "PATH", required = true)]
    statements: Vec<PathBuf>,
}

impl Run for RatifyAnchorArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let statements = self
            .statements
            .iter()
            .map(|path| read_json::<AnchorStatement>(path, "anchor statement"))
            .collect::<Result<Vec<_>>>()?;
        context.finish(vec![ratify_anchor(statements)?])
    }
}

/// The ratification of the one record every statement names, each statement verified for it
/// and from a distinct validator.
fn ratify_anchor(statements: Vec<AnchorStatement>) -> Result<InstructionBox> {
    let record = one_record(
        statements.iter().map(|statement| &statement.record),
        "adoption",
    )?;
    let mut validators = BTreeSet::new();
    let mut attestations = Vec::with_capacity(statements.len());
    for statement in statements {
        statement
            .attestation
            .verify_for(&record)
            .map_err(|error| eyre!("a statement does not verify for the record: {error}"))?;
        if !validators.insert(statement.attestation.attestation.validator.clone()) {
            return Err(eyre!("two statements come from the same validator"));
        }
        attestations.push(statement.attestation);
    }
    attestations.sort();
    Ok(RatifyRuntimeAdoptionAnchorV1 {
        anchor: RuntimeAdoptionAnchorV1 {
            record,
            attestations,
        },
    }
    .into())
}

/// Arguments of `floor-statement`.
#[derive(clap::Args, Debug)]
pub struct FloorStatementArgs {
    /// Height of the checkpoint to state as the startup floor
    #[arg(long, value_name = "HEIGHT")]
    height: u64,
}

/// Arguments of `raise-floor`.
#[derive(clap::Args, Debug)]
pub struct RaiseFloorArgs {
    /// Path to one node's `floor-statement` output; repeat for every validator
    #[arg(long = "statement", value_name = "PATH", required = true)]
    statements: Vec<PathBuf>,
}

impl Run for RaiseFloorArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let statements = self
            .statements
            .iter()
            .map(|path| read_json::<FloorStatement>(path, "floor statement"))
            .collect::<Result<Vec<_>>>()?;
        context.finish(vec![raise_floor(statements)?])
    }
}

/// The raise to the one floor every statement names, each statement verified for it and from a
/// distinct validator.
fn raise_floor(statements: Vec<FloorStatement>) -> Result<InstructionBox> {
    let floor = one_record(statements.iter().map(|statement| &statement.floor), "floor")?;
    let mut validators = BTreeSet::new();
    let mut attestations = Vec::with_capacity(statements.len());
    for statement in statements {
        statement
            .attestation
            .verify_for(&floor)
            .map_err(|error| eyre!("a statement does not verify for the floor: {error}"))?;
        if !validators.insert(statement.attestation.attestation.validator.clone()) {
            return Err(eyre!("two statements come from the same validator"));
        }
        attestations.push(statement.attestation);
    }
    attestations.sort();
    Ok(RaiseRuntimeStartupFloorV1 {
        floor,
        attestations,
    }
    .into())
}

/// The one record all statements name: nodes whose stores disagree derive different records,
/// and their statements cannot be relayed together.
fn one_record<'a, T: Clone + PartialEq + 'a>(
    mut records: impl Iterator<Item = &'a T>,
    what: &str,
) -> Result<T> {
    let first = records
        .next()
        .ok_or_else(|| eyre!("no {what} statement was given"))?
        .clone();
    if records.any(|record| *record != first) {
        return Err(eyre!(
            "the statements name different {what} records: the validators' stores disagree"
        ));
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use iroha::{
        crypto::{Algorithm, Hash, HashOf, KeyPair},
        data_model::{
            NetworkId,
            block::BlockHeader,
            runtime::{
                checkpoint::RuntimeStartupFloorV1, definition::SelectionId,
                lifecycle::AdoptionRecordV1,
            },
        },
    };

    use super::*;

    fn keys() -> Vec<KeyPair> {
        (1..=4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
            .collect()
    }

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([7; 32]),
        ))
    }

    fn record() -> AdoptionRecordV1 {
        AdoptionRecordV1 {
            network: network(),
            height: 5524,
            block_hash: [1; 32],
            predecessor_world_root: [2; 32],
            imported_state_root: [3; 32],
            committee: [4; 32],
            selection: SelectionId([5; 32]),
            authorization: [0; 32],
        }
    }

    fn anchor_statement(key: &KeyPair, record: AdoptionRecordV1) -> AnchorStatement {
        AnchorStatement {
            attestation: RuntimeAdoptionAnchorV1::statement(&record, key.public_key().clone())
                .unwrap()
                .sign(key.private_key())
                .unwrap(),
            record,
        }
    }

    fn floor() -> StartupFloorV1 {
        StartupFloorV1 {
            network: network(),
            height: 6000,
            block_hash: [6; 32],
            committed_ms: 24_000_000,
        }
    }

    fn floor_statement(key: &KeyPair, floor: StartupFloorV1) -> FloorStatement {
        FloorStatement {
            attestation: RuntimeStartupFloorV1::statement(&floor, key.public_key().clone())
                .unwrap()
                .sign(key.private_key())
                .unwrap(),
            floor,
        }
    }

    #[test]
    fn the_statements_of_one_record_relay_as_its_ratification() {
        let keys = keys();
        let statements: Vec<_> = keys
            .iter()
            .rev()
            .map(|key| anchor_statement(key, record()))
            .collect();
        let mut attestations: Vec<_> = statements
            .iter()
            .map(|statement| statement.attestation.clone())
            .collect();
        attestations.sort();
        let expected = InstructionBox::from(RatifyRuntimeAdoptionAnchorV1 {
            anchor: RuntimeAdoptionAnchorV1 {
                record: record(),
                attestations,
            },
        });
        assert_eq!(
            ratify_anchor(statements).expect("one record, distinct validators"),
            expected,
            "every statement, in canonical order"
        );
    }

    #[test]
    fn statements_of_different_records_or_from_one_validator_are_refused() {
        let keys = keys();
        let mut other = record();
        other.imported_state_root = [9; 32];
        let disagreeing = vec![
            anchor_statement(&keys[0], record()),
            anchor_statement(&keys[1], other),
        ];
        assert!(ratify_anchor(disagreeing).is_err());
        let repeated = vec![
            anchor_statement(&keys[0], record()),
            anchor_statement(&keys[0], record()),
        ];
        assert!(ratify_anchor(repeated).is_err());
        // A statement signed for another record does not verify for the one it is filed with.
        let mut forged = anchor_statement(&keys[2], record());
        forged.record.committee = [8; 32];
        assert!(ratify_anchor(vec![forged]).is_err());
        assert!(ratify_anchor(Vec::new()).is_err());
    }

    #[test]
    fn floor_statements_relay_as_one_raise() {
        let keys = keys();
        let statements: Vec<_> = keys
            .iter()
            .map(|key| floor_statement(key, floor()))
            .collect();
        let mut attestations: Vec<_> = statements
            .iter()
            .map(|statement| statement.attestation.clone())
            .collect();
        attestations.sort();
        assert_eq!(
            raise_floor(statements).expect("one floor"),
            InstructionBox::from(RaiseRuntimeStartupFloorV1 {
                floor: floor(),
                attestations,
            })
        );
        let mut later = floor();
        later.height += 1;
        assert!(
            raise_floor(vec![
                floor_statement(&keys[0], floor()),
                floor_statement(&keys[1], later),
            ])
            .is_err()
        );
    }
}
