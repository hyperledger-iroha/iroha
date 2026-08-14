// Same-scope regression coverage extracted to keep the parent source budget bounded.
#[test]
fn policy_activation_is_permissioned_and_exactly_chained() {
    let operator = keypair(0x11);
    let authority = account(&operator);
    let state = state_with_accounts(&[&operator]);
    let mut block = state.block(block_header());
    let mut stx = block.transaction();
    let first = policy();
    let first_digest = first.digest().expect("digest first policy");
    SetSorafsOrderbookPolicy::new(first.clone())
        .execute(&authority, &mut stx)
        .expect("first policy activates");
    let stored = read_policy(stx.world())
        .expect("read policy")
        .expect("policy");
    assert_eq!(stored.policy_digest, first_digest);
    assert_eq!(stored.activated_at_unix, NOW);
    for invalid in {
        let mut gap = first.clone();
        gap.revision = 3;
        gap.predecessor_policy_digest = Some(first_digest);
        let mut branch = first.clone();
        branch.revision = 2;
        branch.predecessor_policy_digest = Some([0x44; 32]);
        let mut market_swap = first.clone();
        market_swap.revision = 2;
        market_swap.predecessor_policy_digest = Some(first_digest);
        market_swap.market_id = [0xB5; 32];
        [gap, branch, market_swap]
    } {
        assert!(
            SetSorafsOrderbookPolicy::new(invalid.clone())
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert_eq!(
            read_policy(stx.world())
                .expect("read unchanged policy")
                .expect("policy")
                .policy_digest,
            first_digest
        );
    }
    let mut second = first;
    second.revision = 2;
    second.predecessor_policy_digest = Some(first_digest);
    second.paused = true;
    SetSorafsOrderbookPolicy::new(second.clone())
        .execute(&authority, &mut stx)
        .expect("exact successor activates");
    assert_eq!(
        read_policy(stx.world())
            .expect("read successor")
            .expect("policy")
            .policy_digest,
        second.digest().expect("digest successor")
    );
}
