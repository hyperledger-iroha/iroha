//! Closed q77 chain shape controls migrated from the retired hash owner.
use super::*;

#[test]
fn whole_pending_chain_rejects_every_wrong_length_and_the_terminal_round() {
    let context = Context::new(b"chain shape rejection").unwrap();
    let root = Digest::from_bytes([0xff; 32]);
    for ordinal in 1..10 {
        let round = Round::new(ordinal).unwrap();
        let raw = vec![0; round.tape_bytes()];
        assert!(context.chain(round, &raw, root).is_ok());
        for length in [0, round.tape_bytes() - 1, round.tape_bytes() + 1] {
            assert!(matches!(
                context.chain(round, &vec![0; length], root),
                Err(BindingError::Phase)
            ));
        }
    }
    let terminal = Round::new(10).unwrap();
    assert!(matches!(
        context.chain(terminal, &vec![0; terminal.tape_bytes()], root),
        Err(BindingError::Phase)
    ));
}
