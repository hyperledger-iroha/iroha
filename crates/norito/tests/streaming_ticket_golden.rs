//! Golden fixtures for Norito streaming ticket payloads.

use norito::streaming::{StreamingTicket, TicketCapabilities, TicketPolicy, TicketRevocation};

fn sample_ticket() -> StreamingTicket {
    let capabilities = TicketCapabilities::from_bits(
        TicketCapabilities::LIVE | TicketCapabilities::HDR | TicketCapabilities::SPATIAL_AUDIO,
    );
    let policy = TicketPolicy {
        max_relays: 4,
        allowed_regions: vec!["us".into(), "jp".into()],
        max_bandwidth_kbps: Some(15_000),
    };
    StreamingTicket {
        ticket_id: [0x44; 32],
        owner: "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE"
            .to_owned(),
        dsid: 7,
        lane_id: 5,
        settlement_bucket: 2_048,
        start_slot: 21_000,
        expire_slot: 24_000,
        prepaid_teu: 120_000,
        chunk_teu: 64,
        fanout_quota: 12,
        key_commitment: [0x55; 32],
        nonce: 42,
        contract_sig: [0x66; 64],
        commitment: [0x77; 32],
        nullifier: [0x88; 32],
        proof_id: [0x99; 32],
        issued_at: 1_701_234_567,
        expires_at: 1_701_834_567,
        policy: Some(policy),
        capabilities,
    }
}

fn sample_revocation() -> TicketRevocation {
    TicketRevocation {
        ticket_id: [0xAA; 32],
        nullifier: [0xBB; 32],
        reason_code: 17,
        revocation_signature: [0xCC; 64],
    }
}

const STREAMING_TICKET_HEX: &str = "4e52543000007a10ebf15999a4727a10ebf15999a47200a50200000000000099696c33ab058c7302000000000000000040014401440144014401440144014401440144014401440144014401440144014401440144014401440144014401440144014401440144014401440144014401445f5e736f726175e383ad314ee383a9684255643242e38384e383b2e3838869e383a4e3838be38384e3838c4b53e3838661e383aae383a1e383a251e383a972e383a16fe383aae3838a6ee382a6e383aa6251e382a6514ae3838b4c4a35485345080700000000000000010508000800000000000008085200000000000008c05d00000000000010c0d401000000000000000000000000000440000000020c004001550155015501550155015501550155015501550155015501550155015501550155015501550155015501550155015501550155015501550155015501550155082a00000000000000800101660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601660166016601664001770177017701770177017701770177017701770177017701770177017701770177017701770177017701770177017701770177017701770177017701770177400188018801880188018801880188018801880188018801880188018801880188018801880188018801880188018801880188018801880188018801880188018840019901990199019901990199019901990199019901990199019901990199019901990199019901990199019901990199019901990199019901990199019901990887c76665000000000847ef6f65000000001d011b0204001002000000000000000302757303026a70060104983a0000050419000000";

const TICKET_REVOCATION_HEX: &str = "4e5254300000621255907c11ad03621255907c11ad03000701000000000000f16065a33ac61199024001aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa01aa4001bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb01bb021100800101cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc01cc";

#[test]
fn streaming_ticket_golden() {
    let actual = norito::to_bytes(&sample_ticket()).expect("serialize streaming ticket");
    let expected = from_hex(STREAMING_TICKET_HEX);
    assert_eq!(
        actual,
        expected,
        "streaming ticket hex mismatch:\n{}",
        to_hex(&actual)
    );
}

#[test]
fn ticket_revocation_golden() {
    let actual = norito::to_bytes(&sample_revocation()).expect("serialize ticket revocation");
    let expected = from_hex(TICKET_REVOCATION_HEX);
    assert_eq!(
        actual,
        expected,
        "ticket revocation hex mismatch:\n{}",
        to_hex(&actual)
    );
}

fn from_hex(hex: &str) -> Vec<u8> {
    let clean: Vec<u8> = hex.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    clean
        .chunks_exact(2)
        .map(|chunk| (decode_nibble(chunk[0]) << 4) | decode_nibble(chunk[1]))
        .collect()
}

fn decode_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => 10 + byte - b'a',
        b'A'..=b'F' => 10 + byte - b'A',
        _ => panic!("invalid hex digit"),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
