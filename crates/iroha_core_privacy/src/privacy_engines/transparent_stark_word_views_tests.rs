//! Allocation-preserving transparent field/word slice views.

use super::*;

#[test]
fn transparent_word_views_preserve_extent_pointer_capacity_and_storage() {
    assert_eq!(size_of::<GoldilocksFieldV1>(), size_of::<u64>());
    assert_eq!(align_of::<GoldilocksFieldV1>(), align_of::<u64>());
    let mut fields = vec![
        GoldilocksFieldV1::ZERO,
        GoldilocksFieldV1::ONE,
        GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1 - 1),
    ];
    let original = (fields.as_ptr(), fields.len(), fields.capacity());
    let words = goldilocks_words_v1(&fields);
    assert_eq!(words.as_ptr(), original.0.cast::<u64>());
    assert_eq!(words, [0, 1, GOLDILOCKS_MODULUS_V1 - 1]);
    goldilocks_words_mut_v1(&mut fields)[1] = 19;
    assert_eq!(fields[1], GoldilocksFieldV1(19));
    assert_eq!((fields.as_ptr(), fields.len(), fields.capacity()), original);
    // A view is not an arithmetic validator. No representation validity is
    // lost even when malformed words are used by a refusal-path test.
    goldilocks_words_mut_v1(&mut fields)[2] = u64::MAX;
    assert_eq!(goldilocks_words_v1(&fields)[2], u64::MAX);
    assert!(GoldilocksFieldV1::canonical(fields[2].0).is_none());
    goldilocks_words_mut_v1(&mut fields).fill(0);
    assert!(fields.iter().all(|value| *value == GoldilocksFieldV1::ZERO));
}

#[test]
fn transparent_word_views_keep_empty_and_disjoint_borrows() {
    let mut empty = Vec::<GoldilocksFieldV1>::new();
    let original = empty.as_ptr();
    assert!(goldilocks_words_v1(&empty).is_empty());
    assert_eq!(goldilocks_words_v1(&empty).as_ptr(), original.cast::<u64>());
    assert!(goldilocks_words_mut_v1(&mut empty).is_empty());
    let mut fields = [GoldilocksFieldV1(3); 4];
    let (left, right) = fields.split_at_mut(2);
    let left_words = goldilocks_words_mut_v1(left);
    let right_words = goldilocks_words_mut_v1(right);
    left_words.copy_from_slice(&[5, 7]);
    right_words.copy_from_slice(&[11, 13]);
    assert_eq!(goldilocks_words_v1(&fields), [5, 7, 11, 13]);
}
