use iroha_config::parameters::actual::Root as Config;
use iroha_core::{
    smartcontracts::isi::offline::VerifiedKagemushaV4RuntimeEffectiveConfigV1,
    sumeragi::GenesisV2Bootstrap,
};
use iroha_genesis::GenesisBlock;

/// Derive release-critical evidence from final config and frozen genesis.
pub(super) fn build_kagemusha_runtime_effective_config_projection_v1(
    config: &Config,
    genesis: &GenesisBlock,
    bootstrap: &GenesisV2Bootstrap,
) -> Result<VerifiedKagemushaV4RuntimeEffectiveConfigV1, String> {
    VerifiedKagemushaV4RuntimeEffectiveConfigV1::derive(config, genesis, bootstrap)
}
