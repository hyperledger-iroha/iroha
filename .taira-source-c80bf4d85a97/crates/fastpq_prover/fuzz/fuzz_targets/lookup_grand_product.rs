#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use fastpq_isi::poseidon;
use fastpq_prover::compute_lookup_grand_product;
use libfuzzer_sys::fuzz_target;

const GOLDILOCKS_MODULUS: u64 = 0xffff_ffff_0000_0001;
const MAX_ENTRIES: usize = 128;

#[derive(Debug)]
struct LookupInput {
    gamma: u64,
    entries: Vec<LookupEntry>,
}

#[derive(Debug)]
struct LookupEntry {
    selector: bool,
    witness: u64,
}

impl<'a> Arbitrary<'a> for LookupInput {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let len = usize::try_from(u.int_in_range(0..=MAX_ENTRIES as u32)?).expect("len fits usize");
        let mut entries = Vec::with_capacity(len);
        for _ in 0..len {
            entries.push(LookupEntry::arbitrary(u)?);
        }
        Ok(Self {
            gamma: u.arbitrary()?,
            entries,
        })
    }
}

impl<'a> Arbitrary<'a> for LookupEntry {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            selector: u.arbitrary()?,
            witness: u.arbitrary()?,
        })
    }
}

fn add_mod(a: u64, b: u64) -> u64 {
    let sum = a.wrapping_add(b);
    if sum >= GOLDILOCKS_MODULUS {
        sum - GOLDILOCKS_MODULUS
    } else {
        sum
    }
}

fn mul_mod(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    let reduced = product % u128::from(GOLDILOCKS_MODULUS);
    u64::try_from(reduced).expect("mod reduction fits u64")
}

fuzz_target!(|input: LookupInput| {
    let selectors: Vec<u64> = input
        .entries
        .iter()
        .map(|entry| if entry.selector { 1 } else { 0 })
        .collect();
    let witnesses: Vec<u64> = input.entries.iter().map(|entry| entry.witness).collect();
    let result = compute_lookup_grand_product(&selectors, &witnesses, input.gamma);
    let mut acc = 1u64;
    let mut running = Vec::with_capacity(witnesses.len());
    for (&selector, &witness) in selectors.iter().zip(witnesses.iter()) {
        if selector != 0 {
            acc = mul_mod(acc, add_mod(witness, input.gamma));
        }
        running.push(acc);
    }
    let expected = poseidon::hash_field_elements(&running);
    assert_eq!(result, expected, "lookup accumulator drift");
});
