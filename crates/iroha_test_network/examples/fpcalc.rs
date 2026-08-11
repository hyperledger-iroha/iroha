use core::num::NonZeroU64;

use iroha_core::sumeragi::consensus::compute_consensus_parameters_fingerprint;
use iroha_data_model::{
    block::{
        consensus::{ConsensusGenesisModeParams, ConsensusGenesisParams},
        consensus_v2::{PROTOCOL_VERSION, SumeragiV2GenesisContextParameters},
    },
    parameter::system::Parameters,
};

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn main() {
    let default_params = Parameters::default();
    let baseline = ConsensusGenesisParams {
        block_cadence_ms: NonZeroU64::new(333).expect("non-zero cadence"),
        block_max_transactions: NonZeroU64::new(10).expect("non-zero block bound"),
        mode: ConsensusGenesisModeParams::Permissioned,
        protocol_version: u32::from(PROTOCOL_VERSION),
        v2_context: SumeragiV2GenesisContextParameters::recommended(),
    };
    let default_like = ConsensusGenesisParams {
        block_cadence_ms: default_params.sumeragi().block_cadence_ms(),
        block_max_transactions: default_params.block().max_transactions(),
        mode: ConsensusGenesisModeParams::Permissioned,
        protocol_version: u32::from(PROTOCOL_VERSION),
        v2_context: SumeragiV2GenesisContextParameters::recommended(),
    };

    let scenarios = [
        ("actual", baseline.clone()),
        ("default_params", default_like.clone()),
        (
            "actual_cadence_default_block_max",
            ConsensusGenesisParams {
                block_max_transactions: default_params.block().max_transactions(),
                ..baseline.clone()
            },
        ),
        (
            "default_cadence_actual_block_max",
            ConsensusGenesisParams {
                block_cadence_ms: default_params.sumeragi().block_cadence_ms(),
                ..baseline
            },
        ),
    ];

    for (label, params) in scenarios {
        let fp = compute_consensus_parameters_fingerprint(&params)
            .expect("scenario must use canonical consensus params");
        println!("{label}: fp=0x{}", hex(&fp));
    }
}
