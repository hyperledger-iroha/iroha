//! Immutable simulated packets share encoding work, while every delivery still decodes
//! a canonical frame and admits its retained bytes to that recipient's original pool.

use std::{cell::OnceCell, ops::Deref, rc::Rc};

use crate::message::{CodecError, WireMessage};

/// One immutable simulated packet and its successful canonical encoding.
///
/// Cloned transport references share only encoding work. They do not share decoded
/// messages, signature checks, receiver admission, or consensus execution.
/// Rewriting an adversarial packet creates a separate carrier with a fresh encoding.
#[derive(Debug)]
pub struct SharedWire {
    message: WireMessage,
    encoded: OnceCell<Vec<u8>>,
}

impl SharedWire {
    /// Share an immutable outgoing packet for its queued deliveries.
    pub fn share(message: WireMessage) -> Rc<Self> {
        Rc::new(Self {
            message,
            encoded: OnceCell::new(),
        })
    }

    /// Read the unchanged packet for routing and adversarial inspection.
    pub fn message(&self) -> &WireMessage {
        &self.message
    }

    /// Encode this immutable packet once, retaining only a successful result.
    ///
    /// Each receiver must independently decode these bytes with its frame limit and
    /// admit the decoded owners to its own original pool before handling the message.
    /// Encoding failure leaves the cell empty so a later delivery retries it.
    ///
    /// # Errors
    /// Propagates the original canonical Norito encoding failure.
    pub fn canonical_bytes(&self) -> Result<&[u8], CodecError> {
        if self.encoded.get().is_none() {
            let bytes = self.message.encode()?;
            assert!(
                self.encoded.set(bytes).is_ok(),
                "single-threaded immutable packet encoding"
            );
        }
        Ok(self
            .encoded
            .get()
            .expect("successful packet encoding retained")
            .as_slice())
    }
}

impl Deref for SharedWire {
    type Target = WireMessage;

    fn deref(&self) -> &Self::Target {
        self.message()
    }
}

#[cfg(test)]
mod tests {
    use iroha_allocation::AllocationBudget;

    use super::*;
    use crate::{availability::RowBytes, message::PayloadChunk, types::Hash32};

    fn chunk(index: u32, row: Vec<u8>) -> WireMessage {
        WireMessage::PayloadChunk(PayloadChunk {
            instance: Hash32([1; 32]),
            height: 1,
            block_hash: Hash32([7; 32]),
            index,
            bytes: RowBytes::from_untrusted(row).unwrap(),
        })
    }

    #[test]
    fn shared_encoding_is_exact_and_adversarial_rewrites_have_separate_carriers() {
        let packet = SharedWire::share(chunk(0, vec![1, 2]));
        assert!(packet.encoded.get().is_none());
        assert_eq!(packet.instance(), &Hash32([1; 32]));
        let canonical = packet.message().encode().unwrap();
        assert_eq!(packet.canonical_bytes().unwrap(), canonical);
        let other_delivery = Rc::clone(&packet);
        let first = packet.canonical_bytes().unwrap();
        let second = other_delivery.canonical_bytes().unwrap();
        assert_eq!(first.as_ptr(), second.as_ptr());
        assert_eq!(first, second);

        let mut changed = packet.message().clone();
        let WireMessage::PayloadChunk(row) = &mut changed else {
            panic!("original payload chunk");
        };
        row.index = 1;
        let rewritten = SharedWire::share(changed);
        assert!(rewritten.encoded.get().is_none());
        assert_ne!(rewritten.canonical_bytes().unwrap(), first);
        assert_eq!(packet.canonical_bytes().unwrap(), canonical);
    }

    #[test]
    fn every_cached_delivery_still_checks_frame_limit_and_original_receiver_pool() {
        let packet = SharedWire::share(chunk(0, vec![1, 2]));
        let bytes = packet.canonical_bytes().unwrap();
        assert!(WireMessage::decode(bytes, bytes.len() - 1).is_err());
        let first_pool = AllocationBudget::new(1 << 20);
        let second_pool = AllocationBudget::new(1 << 20);
        let refused_pool = AllocationBudget::new(0);
        let mut first = WireMessage::decode(bytes, bytes.len()).unwrap();
        let mut second = WireMessage::decode(bytes, bytes.len()).unwrap();
        assert!(!first.owned_bytes_admitted_to(&first_pool));
        assert!(!second.owned_bytes_admitted_to(&second_pool));
        assert!(second.admit_owned_bytes(&refused_pool).is_err());
        assert_eq!(refused_pool.reserved_bytes(), 0);
        first.admit_owned_bytes(&first_pool).unwrap();
        assert!(first.owned_bytes_admitted_to(&first_pool));
        assert!(first.admit_owned_bytes(&second_pool).is_err());
        assert_eq!(second_pool.reserved_bytes(), 0);
        second.admit_owned_bytes(&second_pool).unwrap();
        assert!(second.owned_bytes_admitted_to(&second_pool));
        assert!(!second.owned_bytes_admitted_to(&first_pool));
        assert!(first_pool.reserved_bytes() > 0);
        assert!(second_pool.reserved_bytes() > 0);
        drop(first);
        assert_eq!(first_pool.reserved_bytes(), 0);
        assert!(second_pool.reserved_bytes() > 0);
        drop(second);
        assert_eq!(second_pool.reserved_bytes(), 0);
        assert_eq!(packet.canonical_bytes().unwrap(), bytes);
    }

    #[test]
    fn last_packet_reference_releases_cached_bytes_and_original_source_owners() {
        let source_pool = AllocationBudget::new(1 << 20);
        let mut message = chunk(0, vec![1, 2]);
        message.admit_owned_bytes(&source_pool).unwrap();
        assert!(source_pool.reserved_bytes() > 0);
        let packet = SharedWire::share(message);
        let _ = packet.canonical_bytes().unwrap();
        let weak = Rc::downgrade(&packet);
        let pending_delivery = Rc::clone(&packet);
        drop(packet);
        assert!(weak.upgrade().is_some());
        assert!(source_pool.reserved_bytes() > 0);
        drop(pending_delivery);
        assert!(weak.upgrade().is_none());
        assert_eq!(source_pool.reserved_bytes(), 0);
    }
}
