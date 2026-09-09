//! Observe the existing bounded-encoder sentinel without invoking its serializer.

pub(crate) fn check_rejected_sentinel() {
    super::tests::bounded_encoder_rejects_exact_oversize_before_serialization();
}
