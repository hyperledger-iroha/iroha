// Shared std-only assertions for lifecycle source contracts.
fn source_region<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let Some((_, after_start)) = source.split_once(start) else {
        panic!("missing source-region start token `{start}`");
    };
    let Some((region, _)) = after_start.split_once(end) else {
        panic!("missing source-region end token `{end}`");
    };
    region
}

fn source_token_position(source: &str, token: &str) -> usize {
    let Some(position) = source.find(token) else {
        panic!("missing source token `{token}`");
    };
    position
}

fn assert_required_source_tokens(source: &str, tokens: &[&str]) {
    for token in tokens {
        assert!(
            source.contains(*token),
            "missing required source token `{token}`"
        );
    }
}

fn assert_forbidden_source_tokens(source: &str, tokens: &[&str]) {
    for token in tokens {
        assert!(
            !source.contains(*token),
            "found forbidden source token `{token}`"
        );
    }
}

fn assert_source_tokens_in_order(source: &str, tokens: &[&str]) {
    let mut previous = None;
    for token in tokens {
        let position = source_token_position(source, token);
        if let Some((previous_position, previous_token)) = previous {
            assert!(
                previous_position < position,
                "source token `{previous_token}` must precede `{token}`"
            );
        }
        previous = Some((position, *token));
    }
}

fn assert_source_token_count(source: &str, token: &str, expected: usize) {
    assert_eq!(
        source.matches(token).count(),
        expected,
        "unexpected count for source token `{token}`"
    );
}
