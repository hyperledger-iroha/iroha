let online_peers_provider = {
    let response_limit = config
        .network
        .max_total_connections
        .or(config
            .network
            .lane_profile
            .derived_limits()
            .max_total_connections)
        .map_or(
            config.network.lane_profile.defaults().max_total_connections,
            std::num::NonZeroUsize::get,
        );
    iroha_torii::OnlinePeersProvider::new_with_response_limit(
        network.online_peers_receiver(),
        response_limit,
    )
};
