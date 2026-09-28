// Default and explicit push-registration budgets use the production limiter constructor.
#[cfg(feature = "push")]
#[tokio::test]
async fn push_registration_defaults_admit_ten_thousand_operations_with_one_bucket() {
    let config = iroha_config::parameters::actual::Push::default();
    assert_eq!(
        config.rate_per_minute.map(std::num::NonZeroU32::get),
        Some(600_000)
    );
    assert_eq!(config.burst.map(std::num::NonZeroU32::get), Some(100_000));
    let limiter = super::push_registration_rate_limiter(&config);
    let account = "push:solo-account";
    for request in 0..10_000 {
        assert!(limiter.allow(account).await, "solo registration {request}");
    }
    assert_eq!(limiter.bucket_count().await, 1);
    assert!(!limiter.allow_repeated(account, 100_001).await);
    assert_eq!(limiter.bucket_count().await, 1);
}

#[cfg(feature = "push")]
#[tokio::test]
async fn push_registration_explicit_small_budget_remains_bounded() {
    let config = iroha_config::parameters::actual::Push {
        rate_per_minute: std::num::NonZeroU32::new(1),
        burst: std::num::NonZeroU32::new(2),
        ..Default::default()
    };
    let limiter = super::push_registration_rate_limiter(&config);
    let account = "push:bounded-account";
    assert!(!limiter.allow_repeated(account, 3).await);
    assert!(limiter.allow_repeated(account, 2).await);
    assert!(!limiter.allow(account).await);
    assert_eq!(limiter.bucket_count().await, 1);
}
