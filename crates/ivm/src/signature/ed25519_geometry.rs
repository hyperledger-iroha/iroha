//! Bounded public message geometry; caller bytes never become calibration inputs.

use super::Ed25519BatchItem;

pub(crate) const MIN_ITEMS: usize = 16;
pub(crate) const MAX_ITEMS: usize = 512;
pub(crate) const MAX_MESSAGE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_TOTAL_BYTES: usize = 512 * 1024;

/// Borrow only the lengths of the immutable caller-owned batch.
#[derive(Clone, Copy)]
pub(crate) struct MessageGeometry<'a, 'message> {
    items: &'a [Ed25519BatchItem<'message>],
    bytes: usize,
}
impl<'a, 'message> MessageGeometry<'a, 'message> {
    pub(crate) fn new(items: &'a [Ed25519BatchItem<'message>]) -> Option<Self> {
        if !(MIN_ITEMS..=MAX_ITEMS).contains(&items.len()) {
            return None;
        }
        let mut bytes = 0_usize;
        for item in items {
            if item.message.len() > MAX_MESSAGE_BYTES {
                return None;
            }
            bytes = bytes.checked_add(item.message.len())?;
            if bytes > MAX_TOTAL_BYTES {
                return None;
            }
        }
        Some(Self { items, bytes })
    }
    pub(crate) fn len(self) -> usize {
        self.items.len()
    }
    pub(crate) fn message_len(self, index: usize) -> usize {
        self.items[index].message.len()
    }
    pub(crate) fn total_bytes(self) -> usize {
        self.bytes
    }
}

/// Inline fixed-capacity identity, covered by its original physical owner's
/// allocation. No heap, message digest, signature, key or actual message is kept.
#[derive(Debug)]
pub(crate) struct GeometryKey {
    count: usize,
    lengths: [u32; MAX_ITEMS],
}
impl GeometryKey {
    pub(crate) fn capture(geometry: MessageGeometry<'_, '_>) -> Self {
        let mut key = Self {
            count: geometry.len(),
            lengths: [0; MAX_ITEMS],
        };
        for (index, slot) in key.lengths[..key.count].iter_mut().enumerate() {
            *slot = geometry.message_len(index) as u32;
        }
        key
    }
    pub(crate) fn matches(&self, geometry: MessageGeometry<'_, '_>) -> bool {
        self.count == geometry.len()
            && self.lengths[..self.count]
                .iter()
                .enumerate()
                .all(|(index, &len)| len as usize == geometry.message_len(index))
    }
}

#[cfg(test)]
mod tests;
