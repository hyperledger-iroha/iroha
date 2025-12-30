//! Seeded fuzz-ish regression for adaptive bare codec encode/decode.

use norito::{
    codec::{decode_adaptive, encode_with_header_flags},
    core::{
        DecodeFlagsGuard, {self},
    },
};
use rand::{Rng, SeedableRng, rngs::StdRng};

type SampleEntry = (u16, Vec<u8>, Vec<Vec<u8>>, Option<String>);
type Sample = Vec<SampleEntry>;

fn random_sample(rng: &mut StdRng) -> Sample {
    let len = rng.random_range(0..=8);
    let mut entries = Vec::with_capacity(len);
    for _ in 0..len {
        let tag = rng.random::<u16>();
        let bytes_len = rng.random_range(0..=32);
        let mut bytes = Vec::with_capacity(bytes_len);
        for _ in 0..bytes_len {
            bytes.push(rng.random());
        }

        let nested_len = rng.random_range(0..=4);
        let mut nested = Vec::with_capacity(nested_len);
        for _ in 0..nested_len {
            let inner_len = rng.random_range(0..=24);
            let mut inner = Vec::with_capacity(inner_len);
            for _ in 0..inner_len {
                inner.push(rng.random());
            }
            nested.push(inner);
        }

        let text = if rng.random_bool(0.5) {
            let str_len = rng.random_range(0..=16);
            let s: String = (0..str_len)
                .map(|_| (rng.random_range(b'a'..=b'z')) as char)
                .collect();
            Some(s)
        } else {
            None
        };

        entries.push((tag, bytes, nested, text));
    }
    entries
}

#[test]
fn adaptive_codec_roundtrips_seeded() {
    let mut rng = StdRng::seed_from_u64(0xC0DEC0DE);
    for _ in 0..64 {
        let sample = random_sample(&mut rng);
        let (payload, flags) = encode_with_header_flags(&sample);

        let decoded: Sample = decode_adaptive(&payload).expect("decode adaptive");
        assert_eq!(decoded, sample, "decode without pre-set flags");

        {
            let _guard = DecodeFlagsGuard::enter(flags);
            let decoded_again: Sample =
                decode_adaptive(&payload).expect("decode adaptive with preconfigured flags");
            assert_eq!(decoded_again, sample, "decode with pre-set flags");
        }

        core::reset_decode_state();
    }
}
