//! Runtime support for active-only compiler-owned Kotodama sums.

use ivm_abi::sum::SUM_WORD_BYTES_V1;
pub use ivm_abi::sum::SumLayoutV1;

use crate::{IVM, VMError};

fn layout_error() -> VMError {
    VMError::DecodeError
}

/// Allocate one active-only `Option` or `Result` value.
///
/// The complete larger-branch capacity is reserved once, but only the selected
/// branch words are written. Validation occurs before allocation, so malformed
/// values cannot partially advance or mutate the VM heap.
pub fn allocate_words(
    vm: &mut IVM,
    layout: SumLayoutV1,
    tag: u64,
    active_payload: &[u64],
) -> Result<u64, VMError> {
    let actual = u64::try_from(active_payload.len()).map_err(|_| layout_error())?;
    layout
        .validate_active_width(tag, actual)
        .map_err(|_| layout_error())?;
    let bytes = layout.allocation_bytes().map_err(|_| layout_error())?;
    let base = vm.alloc_heap(bytes)?;
    vm.store_u64(base, tag)?;
    for (index, word) in active_payload.iter().copied().enumerate() {
        let offset = u64::try_from(index)
            .map_err(|_| layout_error())?
            .checked_add(1)
            .and_then(|word_index| word_index.checked_mul(SUM_WORD_BYTES_V1))
            .ok_or_else(layout_error)?;
        let address = base.checked_add(offset).ok_or_else(layout_error)?;
        vm.store_u64(address, word)?;
    }
    Ok(base)
}

/// Validate and read only the selected payload of one compiler-owned sum.
///
/// Reserved words beyond the active branch must remain canonical zero, so an
/// inactive branch can never smuggle a placeholder payload across a boundary.
pub fn read_words(vm: &IVM, base: u64, layout: SumLayoutV1) -> Result<(bool, Vec<u64>), VMError> {
    if !base.is_multiple_of(SUM_WORD_BYTES_V1) {
        return Err(layout_error());
    }
    let bytes = layout.allocation_bytes().map_err(|_| layout_error())?;
    vm.ensure_owned_heap_range(base, bytes)?;
    let raw_tag = vm.load_u64(base)?;
    let active_words = layout.active_words(raw_tag).map_err(|_| layout_error())?;
    let mut payload =
        Vec::with_capacity(usize::try_from(active_words).map_err(|_| layout_error())?);
    for index in 0..active_words {
        let offset = index
            .checked_add(1)
            .and_then(|word_index| word_index.checked_mul(SUM_WORD_BYTES_V1))
            .ok_or_else(layout_error)?;
        let address = base.checked_add(offset).ok_or_else(layout_error)?;
        payload.push(vm.load_u64(address)?);
    }
    for index in active_words..layout.payload_capacity_words() {
        let offset = index
            .checked_add(1)
            .and_then(|word_index| word_index.checked_mul(SUM_WORD_BYTES_V1))
            .ok_or_else(layout_error)?;
        let address = base.checked_add(offset).ok_or_else(layout_error)?;
        if vm.load_u64(address)? != 0 {
            return Err(layout_error());
        }
    }
    Ok((raw_tag == 1, payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;

    #[test]
    fn option_none_and_some_materialize_only_the_active_payload() {
        let mut vm = IVM::new(0);
        let layout = SumLayoutV1::option(2).expect("Option layout");

        let none = allocate_words(&mut vm, layout, 0, &[]).expect("none");
        assert_eq!(none, Memory::HEAP_START);
        assert_eq!(read_words(&vm, none, layout), Ok((false, vec![])));

        let some = allocate_words(&mut vm, layout, 1, &[7, 9]).expect("some");
        assert_eq!(read_words(&vm, some, layout), Ok((true, vec![7, 9])));
    }

    #[test]
    fn result_branches_use_their_own_exact_width() {
        let mut vm = IVM::new(0);
        let layout = SumLayoutV1::try_new(1, 3).expect("Result layout");
        let err = allocate_words(&mut vm, layout, 0, &[44]).expect("err");
        assert_eq!(read_words(&vm, err, layout), Ok((false, vec![44])));
        let ok = allocate_words(&mut vm, layout, 1, &[1, 2, 3]).expect("ok");
        assert_eq!(read_words(&vm, ok, layout), Ok((true, vec![1, 2, 3])));
    }

    #[test]
    fn malformed_values_and_forged_handles_fail_closed() {
        let mut vm = IVM::new(0);
        let layout = SumLayoutV1::try_new(1, 2).expect("layout");
        assert_eq!(
            allocate_words(&mut vm, layout, 1, &[1]),
            Err(VMError::DecodeError)
        );
        assert_eq!(
            allocate_words(&mut vm, layout, 3, &[1]),
            Err(VMError::DecodeError)
        );

        let value = allocate_words(&mut vm, layout, 0, &[8]).expect("valid sum");
        vm.store_u64(value + 16, 9)
            .expect("forge inactive reserved payload");
        assert_eq!(read_words(&vm, value, layout), Err(VMError::DecodeError));
        vm.store_u64(value + 16, 0)
            .expect("restore inactive reserved payload");
        vm.store_u64(value, 2).expect("forge tag");
        assert_eq!(read_words(&vm, value, layout), Err(VMError::DecodeError));
        assert_eq!(
            read_words(&vm, Memory::HEAP_START + 4096, layout),
            Err(VMError::DecodeError)
        );
        assert_eq!(
            read_words(&vm, value + 1, layout),
            Err(VMError::DecodeError)
        );
    }
}
