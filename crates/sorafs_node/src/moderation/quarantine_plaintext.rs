// Plaintext ownership and diagnostics for local quarantine decryption.

/// Own decrypted bytes until verification finishes or an authorized response takes them.
///
/// Drop wipes the initialized bytes with volatile stores and a compiler fence.
/// This does not erase other copies or application bytes explicitly transferred
/// into a successful response. Internal owners do not truncate this buffer.
pub(crate) struct ModerationQuarantinePlaintext(Vec<u8>);
impl ModerationQuarantinePlaintext {
    /// Borrow verified bytes without releasing their scrub owner.
    pub(crate) fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Transfer at the caller's explicit successful payload boundary.
    ///
    /// This operation grants no authorization by itself.
    pub(crate) fn into_authorized_payload(mut self) -> Vec<u8> {
        std::mem::take(&mut self.0)
    }
}
impl std::fmt::Debug for ModerationQuarantinePlaintext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModerationQuarantinePlaintext")
            .field("payload", &"<redacted>")
            .field("payload_len", &self.0.len())
            .finish()
    }
}
impl Drop for ModerationQuarantinePlaintext {
    fn drop(&mut self) {
        #[cfg(test)]
        let nonzero_before = self.0.iter().any(|byte| *byte != 0);
        zeroize_value_for_confidential_discard(self.0.as_mut_slice());
        #[cfg(test)]
        quarantine_plaintext_test_observer::record(&self.0, nonzero_before);
    }
}
impl std::fmt::Debug for ModerationQuarantineObjectPayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModerationQuarantineObjectPayload")
            .field("record", &self.record)
            .field("payload", &"<redacted>")
            .field("payload_len", &self.payload.len())
            .finish()
    }
}
impl std::fmt::Debug for ModerationQuarantineObjectRangePayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModerationQuarantineObjectRangePayload")
            .field("record", &self.record)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("payload", &"<redacted>")
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

#[cfg(test)]
pub(crate) mod quarantine_plaintext_test_observer {
    use std::cell::RefCell;

    /// Only non-secret observations of a live allocation before its destructor frees it.
    #[derive(Debug, PartialEq, Eq)]
    pub(crate) struct DropObservation {
        /// Initialized bytes inspected after the wipe and before deallocation.
        pub(crate) len: usize,
        /// Every inspected byte was zero after the wipe.
        pub(crate) all_zero: bool,
        /// The live buffer contained at least one nonzero byte before the wipe.
        pub(crate) nonzero_before: bool,
    }

    thread_local! {
        static OBSERVATIONS: RefCell<Option<Vec<DropObservation>>> = const { RefCell::new(None) };
    }

    pub(super) fn record(bytes: &[u8], nonzero_before: bool) {
        OBSERVATIONS.with(|observations| {
            if let Ok(mut observations) = observations.try_borrow_mut() {
                if let Some(observations) = observations.as_mut() {
                    observations.push(DropObservation {
                        len: bytes.len(),
                        all_zero: bytes.iter().all(|byte| *byte == 0),
                        nonzero_before,
                    });
                }
            }
        });
    }

    /// Observe synchronous real owner calls on this test thread, never freed memory.
    pub(crate) fn observe<T>(operation: impl FnOnce() -> T) -> (T, Vec<DropObservation>) {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                OBSERVATIONS.with(|observations| {
                    if let Ok(mut observations) = observations.try_borrow_mut() {
                        let _ = observations.take();
                    }
                });
            }
        }
        OBSERVATIONS.with(|observations| {
            let previous = observations.replace(Some(Vec::new()));
            assert!(previous.is_none(), "plaintext observations must not nest");
        });
        let _reset = Reset;
        let result = operation();
        let observations = OBSERVATIONS.with(|observations| {
            observations
                .borrow_mut()
                .take()
                .expect("active observation")
        });
        (result, observations)
    }
}
