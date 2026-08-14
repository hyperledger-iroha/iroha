// Canonical active-lifecycle fixture shared by privacy ISI tests.
fn active_lifecycle() -> PrivacyProtocolLifecycleV1 {
    PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
        proposed_at_height: 1,
        activated_at_height: 2,
        state_since_height: 2,
    })
}
