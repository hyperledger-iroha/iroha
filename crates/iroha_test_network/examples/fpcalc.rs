use iroha_core::sumeragi::consensus::{
    PERMISSIONED_TAG, compute_consensus_fingerprint_from_params,
};
use iroha_data_model::{
    block::consensus::ConsensusGenesisParams, parameter::system::Parameters, prelude::ChainId,
};

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn main() {
    let chain: ChainId = "00000000-0000-0000-0000-000000000000".parse().unwrap();
    let default_params = Parameters::default();
    let baseline = ConsensusGenesisParams {
        block_time_ms: 333,
        commit_time_ms: 667,
        min_finality_ms: 100,
        max_clock_drift_ms: 1000,
        collectors_k: 1,
        redundant_send_r: 1,
        block_max_transactions: 10,
        da_enabled: false,
        epoch_length_blocks: 0,
        bls_domain: "bls-iroha2:permissioned-sumeragi:v1".to_string(),
        npos: None,
    };
    let default_like = ConsensusGenesisParams {
        block_time_ms: default_params.sumeragi().block_time_ms(),
        commit_time_ms: default_params.sumeragi().commit_time_ms(),
        min_finality_ms: default_params.sumeragi().min_finality_ms(),
        max_clock_drift_ms: default_params.sumeragi().max_clock_drift_ms(),
        collectors_k: default_params.sumeragi().collectors_k(),
        redundant_send_r: default_params.sumeragi().collectors_redundant_send_r(),
        block_max_transactions: default_params.block().max_transactions().get(),
        da_enabled: default_params.sumeragi().da_enabled(),
        epoch_length_blocks: 0,
        bls_domain: "bls-iroha2:permissioned-sumeragi:v1".to_string(),
        npos: None,
    };

    let scenarios = [
        ("actual", baseline.clone()),
        ("default_params", default_like.clone()),
        (
            "actual_time_default_block_max",
            ConsensusGenesisParams {
                block_max_transactions: default_params.block().max_transactions().get(),
                ..baseline.clone()
            },
        ),
        (
            "default_time_actual_block_max",
            ConsensusGenesisParams {
                block_time_ms: default_params.sumeragi().block_time_ms(),
                commit_time_ms: default_params.sumeragi().commit_time_ms(),
                ..baseline
            },
        ),
    ];

    for (label, params) in scenarios {
        let fp = compute_consensus_fingerprint_from_params(&chain, &params, PERMISSIONED_TAG);
        println!("{label}: fp=0x{}", hex(&fp));
    }
}
