fn default_oracle() -> iroha_config::parameters::actual::Oracle {
    iroha_config::parameters::actual::Oracle {
        history_depth: iroha_config::parameters::defaults::oracle::history_depth(),
        economics: iroha_config::parameters::actual::OracleEconomics {
            reward_asset: iroha_config::parameters::defaults::oracle::reward_asset(),
            reward_pool: iroha_config::parameters::defaults::oracle::reward_pool(),
            reward_amount: iroha_config::parameters::defaults::oracle::reward_amount(),
            slash_asset: iroha_config::parameters::defaults::oracle::slash_asset(),
            slash_receiver: iroha_config::parameters::defaults::oracle::slash_receiver(),
            slash_outlier_amount: iroha_config::parameters::defaults::oracle::slash_outlier_amount(
            ),
            slash_error_amount: iroha_config::parameters::defaults::oracle::slash_error_amount(),
            slash_no_show_amount: iroha_config::parameters::defaults::oracle::slash_no_show_amount(
            ),
            dispute_bond_asset: iroha_config::parameters::defaults::oracle::dispute_bond_asset(),
            dispute_bond_amount: iroha_config::parameters::defaults::oracle::dispute_bond_amount(),
            dispute_reward_amount:
                iroha_config::parameters::defaults::oracle::dispute_reward_amount(),
            frivolous_slash_amount:
                iroha_config::parameters::defaults::oracle::frivolous_slash_amount(),
        },
        governance: iroha_config::parameters::actual::OracleGovernance {
            intake_sla_blocks: iroha_config::parameters::defaults::oracle::intake_sla_blocks(),
            rules_sla_blocks: iroha_config::parameters::defaults::oracle::rules_sla_blocks(),
            cop_sla_blocks: iroha_config::parameters::defaults::oracle::cop_sla_blocks(),
            technical_sla_blocks: iroha_config::parameters::defaults::oracle::technical_sla_blocks(
            ),
            policy_jury_sla_blocks:
                iroha_config::parameters::defaults::oracle::policy_jury_sla_blocks(),
            enact_sla_blocks: iroha_config::parameters::defaults::oracle::enact_sla_blocks(),
            intake_min_votes: iroha_config::parameters::defaults::oracle::intake_min_votes(),
            rules_min_votes: iroha_config::parameters::defaults::oracle::rules_min_votes(),
            cop_min_votes: iroha_config::parameters::actual::OracleChangeThresholds {
                low: iroha_config::parameters::defaults::oracle::cop_low_votes(),
                medium: iroha_config::parameters::defaults::oracle::cop_medium_votes(),
                high: iroha_config::parameters::defaults::oracle::cop_high_votes(),
            },
            technical_min_votes: iroha_config::parameters::defaults::oracle::technical_min_votes(),
            policy_jury_min_votes: iroha_config::parameters::actual::OracleChangeThresholds {
                low: iroha_config::parameters::defaults::oracle::policy_jury_low_votes(),
                medium: iroha_config::parameters::defaults::oracle::policy_jury_medium_votes(),
                high: iroha_config::parameters::defaults::oracle::policy_jury_high_votes(),
            },
        },
        twitter_binding: iroha_config::parameters::actual::OracleTwitterBinding {
            feed_id: iroha_config::parameters::defaults::oracle::twitter_binding_feed_id(),
            pepper_id: iroha_config::parameters::defaults::oracle::twitter_binding_pepper_id(),
            max_ttl_ms: iroha_config::parameters::defaults::oracle::twitter_binding_max_ttl_ms(),
            min_ttl_ms: iroha_config::parameters::defaults::oracle::twitter_binding_min_ttl_ms(),
            min_update_spacing_ms:
                iroha_config::parameters::defaults::oracle::twitter_binding_min_update_spacing_ms(),
        },
    }
}
