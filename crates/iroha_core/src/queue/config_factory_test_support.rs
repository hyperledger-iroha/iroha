// Shared queue test configuration factory.

fn config_factory() -> Config {
    Config {
        transaction_time_to_live: Duration::from_secs(100),
        capacity: 100.try_into().unwrap(),
        ..Config::default()
    }
}
