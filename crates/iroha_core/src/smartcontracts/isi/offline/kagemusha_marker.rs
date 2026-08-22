//! Deterministic, length-delimited Kagemusha marker derivation.

use iroha_crypto::Hash;

pub(super) fn v2_marker(domain: &str, components: &[&[u8]]) -> Hash {
    let mut preimage = Vec::with_capacity(
        domain.len()
            + components
                .iter()
                .map(|component| 8usize.saturating_add(component.len()))
                .sum::<usize>(),
    );
    preimage.extend_from_slice(domain.as_bytes());
    for component in components {
        preimage.extend_from_slice(
            &u64::try_from(component.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        preimage.extend_from_slice(component);
    }
    Hash::new(&preimage)
}
