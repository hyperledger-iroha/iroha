//! Exact existing-account DATA frame shared with the genuine Core E1 contract.

const DOMAIN: &[u8] = b"BPNG.KAGEMUSHA.WALLET.ENROLLMENT.ACCOUNT.V1\0";

pub(super) fn frame(
    network: &[u8; 32],
    transcript: &[u8],
    owner: &[u8; 32],
    payment: &[u8],
    issued: u64,
    expires: u64,
) -> Option<Vec<u8>> {
    if network.iter().all(|v| *v == 0)
        || owner.iter().all(|v| *v == 0)
        || transcript.len() != 194
        || transcript[..2] != 1_u16.to_le_bytes()
        || transcript[2..]
            .chunks_exact(32)
            .any(|digest| digest.iter().all(|v| *v == 0))
        || payment.len() != 65
        || payment[0] != 4
        || issued == 0
        || !expires
            .checked_sub(issued)
            .is_some_and(|n| (1..=600_000).contains(&n))
    {
        return None;
    }
    let mut out = Vec::with_capacity(385);
    out.extend_from_slice(DOMAIN);
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(network);
    out.extend_from_slice(transcript);
    out.extend_from_slice(owner);
    out.extend_from_slice(payment);
    out.extend_from_slice(&issued.to_le_bytes());
    out.extend_from_slice(&expires.to_le_bytes());
    (out.len() == 385).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn originals() -> (Vec<u8>, Vec<u8>) {
        let mut transcript = 1_u16.to_le_bytes().to_vec();
        for v in 1..=6 {
            transcript.extend_from_slice(&[v; 32]);
        }
        let mut payment = vec![7; 65];
        payment[0] = 4;
        (transcript, payment)
    }
    #[test]
    fn exact_core_domain_field_order_and_original_dates() {
        let (challenge, key) = originals();
        let out = frame(&[9; 32], &challenge, &[8; 32], &key, 0x10203, 0x40506).unwrap();
        assert_eq!(out.len(), 385);
        assert_eq!(&out[..DOMAIN.len()], DOMAIN);
        let mut at = DOMAIN.len();
        assert_eq!(&out[at..at + 2], &1_u16.to_le_bytes());
        at += 2;
        assert_eq!(&out[at..at + 32], &[9; 32]);
        at += 32;
        assert_eq!(&out[at..at + 194], challenge);
        at += 194;
        assert_eq!(&out[at..at + 32], &[8; 32]);
        at += 32;
        assert_eq!(&out[at..at + 65], key);
        at += 65;
        assert_eq!(&out[at..at + 8], &0x10203_u64.to_le_bytes());
        at += 8;
        assert_eq!(&out[at..], &0x40506_u64.to_le_bytes());
    }
    #[test]
    fn no_projection_or_zero_binding_can_form_a_frame() {
        let (mut challenge, key) = originals();
        assert!(frame(&[0; 32], &challenge, &[8; 32], &key, 1, 2).is_none());
        assert!(frame(&[9; 32], &challenge, &[0; 32], &key, 1, 2).is_none());
        challenge[2..34].fill(0);
        assert!(frame(&[9; 32], &challenge, &[8; 32], &key, 1, 2).is_none());
        assert!(frame(&[9; 32], &challenge[..193], &[8; 32], &key, 1, 2).is_none());
    }
    #[test]
    fn substituted_key_or_dates_changes_signed_original() {
        let (challenge, mut key) = originals();
        let original = frame(&[9; 32], &challenge, &[8; 32], &key, 1, 2).unwrap();
        key[1] ^= 1;
        assert_ne!(
            frame(&[9; 32], &challenge, &[8; 32], &key, 1, 2).unwrap(),
            original
        );
        assert_ne!(
            frame(&[9; 32], &challenge, &[8; 32], &key, 2, 3).unwrap(),
            original
        );
        assert!(frame(&[9; 32], &challenge, &[8; 32], &key, 1, 600_002).is_none());
    }
}
