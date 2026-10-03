//! Independent canonical SHA3 adapters for generic toy-tree topology tests.
//!
//! These graphs are deliberately smaller/larger than the proof geometry. They
//! are not a proof protocol, proof decoder, or synthetic execution-relation test.
use super::super::compact_sha3::{BodyFields, Context};
use crate::Result;
use fastpq_isi::keccak256::Sha3Digest256V1 as Digest;
pub(super) fn leaf(context: &Context, round: u8, index: u32, bytes: &[u8]) -> Result<Digest> {
    context
        .hash_frame(&context.frame(1, 3, round, 0, index, 32, BodyFields::One(bytes)))
        .map_err(|_| super::shape("canonical SHA3 reference leaf failed"))
}
pub(super) fn parent(
    context: &Context,
    round: u8,
    level: u32,
    index: u32,
    left: Digest,
    right: Digest,
) -> Result<Digest> {
    context
        .hash_frame(&context.frame(
            2,
            3,
            round,
            level,
            index,
            32,
            BodyFields::Two(left.as_bytes(), right.as_bytes()),
        ))
        .map_err(|_| super::shape("canonical SHA3 reference parent failed"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independently_encoded_sha3_tree_matches_every_leaf_and_root() {
        let context = Context::new(b"complete public context without a private witness").unwrap();
        let expected = [
            "a4978555e5154cc725098513117df50be99ba4c6a6f2c57b3053a68295f678ba",
            "db7b0cf4c7b767c30d7dd4dd240794ed5e706b1e86a997469bfe421497a015bc",
            "c32cad07fb35e41b354bf814ce8b7f91a6b676afae5ff4b05ff0405fe1555b55",
            "cd0fcf96587a683a917dbbbe1b5c12bb2507360d96c4deccbdeb70933235ceb3",
            "0c0903e147b753b743c8e77d9ee9301c84a95571ee20dc936989ba32f9ed248e",
            "b4c9c43cf5b091ed589a39810e445a7c48954dc2100d5aaf2db619aec2ab89db",
            "df63a0e93d665bf2758881d3cb68b1d396c3e38fa52a55fdf70ea2c7c9cce1c4",
            "c6b0a2de4c969d13c2c0300163fe96be69730ad9b6f6a61fae3bd2b16e2e75da",
        ];
        let mut levels = vec![
            (0..8_u32)
                .map(|index| {
                    let mut payload = [0; 64];
                    payload[..8].copy_from_slice(&u64::from(index + 1).to_le_bytes());
                    let actual = leaf(&context, 3, index, &payload).unwrap();
                    assert_eq!(hex::encode(actual.into_bytes()), expected[index as usize]);
                    actual
                })
                .collect::<Vec<_>>(),
        ];
        while levels.last().unwrap().len() > 1 {
            let next = levels
                .last()
                .unwrap()
                .chunks_exact(2)
                .enumerate()
                .map(|(index, children)| {
                    parent(
                        &context,
                        3,
                        u32::try_from(levels.len()).expect("bounded reference tree depth"),
                        u32::try_from(index).expect("bounded reference tree index"),
                        children[0],
                        children[1],
                    )
                    .unwrap()
                })
                .collect();
            levels.push(next);
        }
        assert_eq!(
            hex::encode(levels.last().unwrap()[0].into_bytes()),
            "12b46c57525a90481660afabc7fb6fb483534243090541ddd2503fc5ec9d0cc5"
        );
    }
}
