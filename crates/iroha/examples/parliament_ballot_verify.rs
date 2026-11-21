//! Verify a closed Parliament ballot against every certified block from the configured genesis.
//!
//! Run `cargo run --release -p iroha --example parliament_ballot_verify -- CLIENT.toml
//! CLOSED_BOARD.nrt SEAL_HEIGHT SEAL_POSITION [OTHER_AUTHENTICATED_BOARD.nrt ...]`.
//! The client configuration must independently pin the original network identity. The closed
//! board and seal coordinates are claims checked against certified execution. Other scopes'
//! supplied contexts and the underlying voter eligibility are independently authenticated inputs.

use std::{fs::File, io::Read as _, num::NonZeroU64, path::Path};

use eyre::{Result, WrapErr, bail, ensure, eyre};
use iroha::{
    client::Client,
    config::Config,
    data_model::{
        NetworkId,
        block::{consensus::SumeragiRootScope, decode_framed_signed_block},
        governance::{
            GovernanceAnchorV1,
            anonymous_ballot::{
                BALLOT_BOARD_RECORD_MAX_BYTES_V1, ParliamentBallotBoardRecordV1,
                SealedCredentialListBytesV1, SealedCredentialListPublishedV1,
            },
        },
        sumeragi_finality::{SumeragiFinalityProof, SumeragiFinalityVerifier},
    },
};
use iroha_parliament_ballot::{
    board::BallotBoardV1,
    corpus::ClosureAnchorsV1,
    finalized_interval::verify_finalized_ballot_interval_v1,
    replay::{FullVerificationRequestV1, FullVerificationV1, ReplaySourceErrorV1},
};

fn read_board(path: &Path) -> Result<ParliamentBallotBoardRecordV1> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(u64::try_from(BALLOT_BOARD_RECORD_MAX_BYTES_V1)? + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= BALLOT_BOARD_RECORD_MAX_BYTES_V1,
        "board exceeds its wire bound"
    );
    Ok(norito::decode_canonical(&bytes)?)
}

fn request(record: ParliamentBallotBoardRecordV1) -> Result<FullVerificationRequestV1> {
    let board = BallotBoardV1::restore(record.clone())?;
    let closure = record
        .closure
        .ok_or_else(|| eyre!("board has no closure record"))?;
    let seal = SealedCredentialListPublishedV1 {
        body_context: *board.context().body_context(),
        body_instance_id: record.body_instance_id,
        roster_root: record.roster_root,
        list_digest: *board.context().list_digest(),
        registration_mapping_digest: record.registration_mapping_digest,
        sealed_list_bytes: SealedCredentialListBytesV1::from_list(&record.context.sealed_list()?)?,
    };
    Ok(FullVerificationRequestV1 {
        context: board.context().clone(),
        seal,
        other_scopes: Vec::new(),
        proposal_content_id: record.proposal_content_id,
        anchors: ClosureAnchorsV1 {
            opening: closure.opening_anchor,
            casting_clock_height: closure.casting_clock_height,
            casting_clock_timestamp_ms: closure.casting_clock_timestamp_ms,
            closure: closure.closure_anchor,
        },
        closure_record: closure,
        certified_result_root: None,
    })
}

fn pinned_genesis(
    network: &NetworkId,
    chain_label: &str,
    proof: &SumeragiFinalityProof,
) -> Result<SumeragiFinalityVerifier> {
    let block = decode_framed_signed_block(&proof.block_wire)?;
    ensure!(
        NetworkId::from_genesis_hash(block.hash()) == *network,
        "genesis differs from the configured network identity"
    );
    let mut verifier = SumeragiFinalityVerifier::new(&block, chain_label, proof.committee.clone())?;
    verifier.verify(proof)?;
    Ok(verifier)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [config_path, board_path, height, position, other_boards @ ..] = args.as_slice() else {
        bail!(
            "usage: parliament_ballot_verify CLIENT.toml CLOSED_BOARD.nrt SEAL_HEIGHT SEAL_POSITION [OTHER_AUTHENTICATED_BOARD.nrt ...]"
        );
    };
    let config = Config::load_file(config_path)?;
    let chain_label = config.chain.to_string();
    let mut builder = Client::builder(config).api_token(None);
    builder.headers.clear();
    let client = builder.build()?;
    let record = read_board(Path::new(board_path))?;
    let seal = GovernanceAnchorV1 {
        height: height
            .to_str()
            .ok_or_else(|| eyre!("invalid seal height"))?
            .parse()?,
        position: position
            .to_str()
            .ok_or_else(|| eyre!("invalid seal position"))?
            .parse()?,
        timestamp_ms: record.sealed_timestamp_ms,
    };
    let mut request = request(record)?;
    for path in other_boards {
        let other = BallotBoardV1::restore(read_board(Path::new(path))?)?;
        request.other_scopes.push(other.context().clone());
    }
    let proof = client.get_sumeragi_finality_proof(NonZeroU64::MIN, SumeragiRootScope::Global)?;
    let verifier = pinned_genesis(client.network_id(), &chain_label, &proof)?;
    let result = verify_finalized_ballot_interval_v1(verifier, &request, seal, |height| {
        let height = NonZeroU64::new(height).ok_or(ReplaySourceErrorV1::Missing { height })?;
        client
            .get_sumeragi_finality_proof(height, SumeragiRootScope::Global)
            .wrap_err_with(|| format!("retrieving certified height {height}"))
            .map_err(|e| ReplaySourceErrorV1::Unavailable {
                reason: format!("{e:#}"),
            })
    })?;
    match result {
        FullVerificationV1::IndependentFullVerification(report) => {
            println!(
                "Verified {} interval blocks and {} ballots. Accepted {}. Outcome {:?}. Result root {}.",
                report.blocks,
                report.ballots_verified,
                report.accepted,
                report.closure_record.outcome,
                hex::encode(report.result_root.as_bytes())
            );
            Ok(())
        }
        other => bail!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::data_model::testing::native_finality::NativeFinalityFixture;

    #[test]
    fn open_board_cannot_be_verified_as_closed() {
        let board = read_board(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/governance/anonymous_ballot_http_v1/board.nrt"),
        )
        .unwrap();
        assert!(
            request(board)
                .unwrap_err()
                .to_string()
                .contains("no closure record")
        );
    }

    #[test]
    fn genesis_must_match_the_independently_pinned_network_and_chain() {
        let native = NativeFinalityFixture::new();
        let foreign = NativeFinalityFixture::start_at("other", 8_201);
        let proof = native.genesis_proof();
        assert!(pinned_genesis(&foreign.network_id(), native.chain_id(), proof).is_err());
        // The signed genesis can be reused under a different human-readable label. The first
        // successor's original quorum certificate binds the selected consensus instance.
        let wrong_label = pinned_genesis(&native.network_id(), foreign.chain_id(), proof);
        if let Ok(mut verifier) = wrong_label {
            assert!(verifier.verify(native.latest()).is_err());
        }
        let mut verifier = pinned_genesis(&native.network_id(), native.chain_id(), proof).unwrap();
        assert_eq!(verifier.authenticated_height(), Some(1));
        verifier.verify(native.latest()).unwrap();
        assert_eq!(verifier.authenticated_height(), Some(2));
    }
}
