//! Strict ordered Ed25519 verification with caller-owned publication.

use super::{Ed25519BatchItem, SignatureScheme, verify_signature};

/// Borrowed fixed-geometry inputs shared by qualified native backends.
/// No validity-dependent compaction or intermediate result allocation is used.
#[cfg(any(feature = "cuda", all(target_os = "macos", feature = "metal"), test))]
#[derive(Clone, Copy)]
pub(crate) enum BatchInput<'a, 'message> {
    #[cfg(any(feature = "cuda", test))]
    Prepared {
        signatures: &'a [[u8; 64]],
        public_keys: &'a [[u8; 32]],
        hrams: &'a [[u8; 32]],
    },
    Items(&'a [Ed25519BatchItem<'message>]),
}

#[cfg(any(feature = "cuda", all(target_os = "macos", feature = "metal"), test))]
impl BatchInput<'_, '_> {
    pub(crate) fn checked_len(self) -> Option<usize> {
        match self {
            #[cfg(any(feature = "cuda", test))]
            Self::Prepared {
                signatures,
                public_keys,
                hrams,
            } if signatures.len() == public_keys.len() && signatures.len() == hrams.len() => {
                Some(signatures.len())
            }
            #[cfg(any(feature = "cuda", test))]
            Self::Prepared { .. } => None,
            Self::Items(items) => Some(items.len()),
        }
    }
    pub(crate) fn signature(self, index: usize) -> [u8; 64] {
        match self {
            #[cfg(any(feature = "cuda", test))]
            Self::Prepared { signatures, .. } => signatures[index],
            Self::Items(items) => items[index].signature,
        }
    }
    pub(crate) fn public_key(self, index: usize) -> [u8; 32] {
        match self {
            #[cfg(any(feature = "cuda", test))]
            Self::Prepared { public_keys, .. } => public_keys[index],
            Self::Items(items) => items[index].public_key,
        }
    }
    pub(crate) fn hram(self, index: usize) -> [u8; 32] {
        match self {
            #[cfg(any(feature = "cuda", test))]
            Self::Prepared { hrams, .. } => hrams[index],
            Self::Items(items) => {
                let item = &items[index];
                super::ed25519_challenge_scalar_bytes(
                    &item.signature,
                    &item.public_key,
                    item.message,
                )
            }
        }
    }
    pub(crate) fn structurally_valid(self, index: usize) -> bool {
        let signature = self.signature(index);
        !super::signature_bytes_are_all_zero(&signature)
            && !super::signature_has_invalid_ed25519_r(&signature)
            && !super::ed25519_public_key_bytes_are_invalid(&self.public_key(index))
    }
    /// Validate the entire native result before any caller storage is changed.
    pub(crate) fn publish(self, native: &[u8], destination: &mut [bool]) -> bool {
        if self.checked_len() != Some(destination.len())
            || native.len() != destination.len()
            || native.iter().any(|&byte| byte > 1)
        {
            return false;
        }
        for (index, (output, &native)) in destination.iter_mut().zip(native).enumerate() {
            *output = native == 1 && self.structurally_valid(index);
        }
        true
    }
}

pub(crate) fn cpu_batch_into(items: &[Ed25519BatchItem<'_>], destination: &mut [bool]) {
    for (item, output) in items.iter().zip(destination) {
        *output = verify_signature(
            SignatureScheme::Ed25519,
            item.message,
            &item.signature,
            &item.public_key,
        );
    }
}

fn with_fallback(
    items: &[Ed25519BatchItem<'_>],
    destination: &mut [bool],
    attempt: impl FnOnce(&mut [bool]) -> bool,
) -> bool {
    if items.len() != destination.len() {
        return false;
    }
    if items.is_empty() || attempt(destination) {
        return true;
    }
    // Every element is recomputed from the original input, even if a refusing
    // backend happened to touch some destination slots before returning.
    cpu_batch_into(items, destination);
    true
}

/// Verify an ordered batch into already funded caller storage. A mismatched
/// destination is unchanged and returns `false`; otherwise all entries are
/// initialized, with malformed or invalid signatures marked `false`.
/// Backend refusal recomputes the complete batch from the original inputs.
pub fn verify_ed25519_batch_items_into(
    items: &[Ed25519BatchItem<'_>],
    destination: &mut [bool],
) -> bool {
    with_fallback(items, destination, |destination| {
        #[cfg(all(target_os = "macos", feature = "metal"))]
        if crate::vector::metal_ed25519_auto_into(items, destination) {
            return true;
        }
        #[cfg(feature = "cuda")]
        if let Some(bytes) = items.len().checked_mul(129)
            && u32::try_from(items.len()).is_ok()
            && crate::vector::gpu_launch_eligible(bytes)
            && crate::cuda::ed25519_items_cuda_into(items, destination)
        {
            return true;
        }
        let _ = destination;
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn default_item_is_inert_and_shared_cpu_traversal_initializes_every_slot() {
        let items = [Ed25519BatchItem::default(); 2];
        assert!(items.iter().all(|item| item.message.is_empty()));
        let mut output = [true; 2];
        cpu_batch_into(&items, &mut output);
        assert_eq!(output, [false; 2]);
    }

    #[test]
    fn malformed_destination_and_empty_batch_do_not_attempt_acceleration() {
        assert!(with_fallback(&[], &mut [], |_| panic!("empty attempt")));
        let mut output = [true];
        assert!(!with_fallback(&[], &mut output, |_| panic!(
            "shape attempt"
        )));
        assert_eq!(output, [true]);
    }

    #[test]
    fn refusal_recomputes_every_slot_from_original_items() {
        let key = SigningKey::from_bytes(&[0x47; 32]);
        let message = b"strict original batch";
        let item = Ed25519BatchItem {
            message,
            signature: key.sign(message).to_bytes(),
            public_key: key.verifying_key().to_bytes(),
        };
        let mut bad = item;
        bad.message = b"different";
        let items = [item, bad];
        let mut output = [false, true];
        assert!(with_fallback(&items, &mut output, |output| {
            output.copy_from_slice(&[false, true]);
            false
        }));
        assert_eq!(output, [true, false]);
    }

    #[test]
    fn generated_inputs_preserve_order_and_share_challenge_relation() {
        let key = SigningKey::from_bytes(&[0x48; 32]);
        let message = b"generated input";
        let item = Ed25519BatchItem {
            message,
            signature: key.sign(message).to_bytes(),
            public_key: key.verifying_key().to_bytes(),
        };
        let items = [item];
        let input = BatchInput::Items(&items);
        let signatures = [input.signature(0)];
        let keys = [input.public_key(0)];
        let hrams = [input.hram(0)];
        let prepared = BatchInput::Prepared {
            signatures: &signatures,
            public_keys: &keys,
            hrams: &hrams,
        };
        assert_eq!(input.checked_len(), Some(1));
        assert_eq!(input.signature(0), prepared.signature(0));
        assert_eq!(input.public_key(0), prepared.public_key(0));
        assert_eq!(input.hram(0), prepared.hram(0));
        assert!(input.structurally_valid(0));
        let malformed = BatchInput::Prepared {
            signatures: &signatures,
            public_keys: &[],
            hrams: &hrams,
        };
        assert_eq!(malformed.checked_len(), None);
    }

    #[test]
    fn result_validation_precedes_publication_and_strict_checks_reject_weak_material() {
        let signatures = [[0; 64], [1; 64]];
        let keys = [[0; 32]; 2];
        let hrams = [[0; 32]; 2];
        let input = BatchInput::Prepared {
            signatures: &signatures,
            public_keys: &keys,
            hrams: &hrams,
        };
        let mut output = [true; 2];
        assert!(!input.publish(&[0, 2], &mut output));
        assert_eq!(output, [true; 2]);
        assert!(!input.publish(&[0], &mut output));
        assert_eq!(output, [true; 2]);
        assert!(input.publish(&[1, 1], &mut output));
        assert_eq!(output, [false; 2]);
    }
}
