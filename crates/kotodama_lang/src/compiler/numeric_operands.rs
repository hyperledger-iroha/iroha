//! Test-only same-compiler baseline for redundant numeric operand publication.

std::thread_local! {
    static RETAIN_PUBLICATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn retain_publication() -> bool {
    RETAIN_PUBLICATION.get()
}

pub(super) fn with_publication<R>(body: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            RETAIN_PUBLICATION.set(self.0);
        }
    }
    let _restore = Restore(RETAIN_PUBLICATION.replace(true));
    // The former sequential publication emitter required conservative homes.
    crate::regalloc::with_conservative_host_operands(body)
}
