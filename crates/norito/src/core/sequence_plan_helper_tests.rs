// Included inside the sequence-plan helper module to preserve test scope.

mod tests {
    use super::{
        AbiSpan, BinarySequenceLayout, HelperOutcome, RC_NO_SPACE, RC_UNAVAILABLE, call_helper,
        load_sequence_plan_library, sequence_plan_helper_self_test,
    };

    unsafe extern "C" fn mismatched_helper(
        _input_ptr: *const u8,
        input_len: usize,
        _flags: u8,
        _layout_kind: u32,
        out_spans: *mut AbiSpan,
        out_capacity: usize,
        out_count: *mut usize,
        out_used: *mut usize,
    ) -> i32 {
        unsafe {
            *out_count = 1;
            *out_used = input_len;
        }
        if out_capacity == 0 {
            return RC_NO_SPACE;
        }
        unsafe {
            *out_spans = AbiSpan { start: 0, end: 0 };
        }
        0
    }

    unsafe extern "C" fn backend_error_helper(
        _input_ptr: *const u8,
        _input_len: usize,
        _flags: u8,
        _layout_kind: u32,
        _out_spans: *mut AbiSpan,
        _out_capacity: usize,
        _out_count: *mut usize,
        _out_used: *mut usize,
    ) -> i32 {
        4
    }

    unsafe extern "C" fn unavailable_helper(
        _input_ptr: *const u8,
        _input_len: usize,
        _flags: u8,
        _layout_kind: u32,
        _out_spans: *mut AbiSpan,
        _out_capacity: usize,
        _out_count: *mut usize,
        _out_used: *mut usize,
    ) -> i32 {
        RC_UNAVAILABLE
    }

    #[test]
    fn sequence_plan_helper_self_test_rejects_mismatched_helper() {
        assert!(!sequence_plan_helper_self_test(mismatched_helper));
    }

    #[test]
    fn sequence_plan_loads_required_cuda_helper_when_requested() {
        let lib = unsafe { load_sequence_plan_library() };
        if std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some() {
            assert!(
                lib.is_some(),
                "JSONSTAGE1_CUDA_REQUIRE requires the CUDA sequence-plan helper to load and pass self-test"
            );
        } else if lib.is_none() {
            eprintln!("sequence-plan CUDA helper unavailable; skipping required-helper assertion");
        }
    }

    #[test]
    fn helper_backend_errors_are_distinguished_from_bad_input() {
        let bytes = super::make_unpacked_case(super::super::header_flags::COMPACT_LEN);
        let outcome = unsafe {
            call_helper(
                backend_error_helper,
                &bytes,
                super::super::header_flags::COMPACT_LEN,
                BinarySequenceLayout::LengthPrefixed,
            )
        };

        assert!(matches!(outcome, HelperOutcome::BackendFailure));
    }

    #[test]
    fn helper_unavailable_is_a_fallback_not_backend_failure() {
        let bytes = super::make_unpacked_case(super::super::header_flags::COMPACT_LEN);
        let outcome = unsafe {
            call_helper(
                unavailable_helper,
                &bytes,
                super::super::header_flags::COMPACT_LEN,
                BinarySequenceLayout::LengthPrefixed,
            )
        };

        assert!(matches!(outcome, HelperOutcome::BackendUnavailable));
    }
}
