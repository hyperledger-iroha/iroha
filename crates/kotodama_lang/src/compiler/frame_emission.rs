//! Reuse exact saved-register addresses and one equivalent return restoration.
//!
//! Every slot, frame size, callable role and memory-access order is unchanged.
//! Windows are used only when four large-offset accesses amortize even a new
//! indexed i64 literal. Return values are completely published before the
//! shared restoration; no source operation or checked trap is moved.

use super::*;

#[cfg(test)]
std::thread_local! {
    static RETAIN_SCALAR_FRAME_EMISSION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
pub(super) fn with_scalar_frame_emission<R>(body: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            RETAIN_SCALAR_FRAME_EMISSION.set(self.0);
        }
    }
    let _restore = Restore(RETAIN_SCALAR_FRAME_EMISSION.replace(true));
    body()
}
fn retain_scalar() -> bool {
    #[cfg(test)]
    if RETAIN_SCALAR_FRAME_EMISSION.get() {
        return true;
    }
    false
}

pub(super) fn emit_saved_registers(
    code: &mut Vec<u8>,
    fixups: &LiteralFixups,
    registers: &[u8],
    save_base: usize,
    restore: bool,
) -> Result<(), String> {
    let sp = regalloc::SP_REG as u8;
    // r27/r28 are compiler-reserved, never part of the allocation pool. Keep
    // an explicit fail-closed boundary if that ownership ever changes.
    if registers.iter().any(|reg| matches!(*reg, 26..=31)) {
        return Err("saved register aliases reserved frame-address scratch".to_owned());
    }
    let mut offsets = Vec::with_capacity(registers.len());
    for index in 0..registers.len() {
        let offset = index
            .checked_mul(8)
            .and_then(|offset| save_base.checked_add(offset))
            .ok_or("saved register slot overflow")?;
        i64::try_from(offset).map_err(|_| "saved register slot exceeds signed address space")?;
        offsets.push(offset);
    }
    let large = offsets
        .iter()
        .filter(|offset| **offset > WIDE_IMM_MAX as usize)
        .count();
    // At most ten allocated callee-preserved registers fit in one 256-byte
    // window. Four large accesses save at least 24 executable bytes, covering
    // a worst-case 16-byte new i64 directory/data entry as well.
    let windowed = !retain_scalar() && large >= 4 && registers.len() <= 10;
    // Preflight the complete window anchor before writing code or fixups.
    // Frame admission normally makes this tiny, but emission itself must not
    // leave a prefix when the signed-address ceiling is exceeded.
    if windowed {
        let first_large = offsets
            .iter()
            .copied()
            .find(|offset| *offset > WIDE_IMM_MAX as usize)
            .unwrap();
        i64::try_from(first_large)
            .ok()
            .and_then(|offset| offset.checked_sub(i64::from(WIDE_IMM_MIN)))
            .ok_or("saved register window anchor overflow")?;
    }
    let mut window = StackTableWindow::new(27);
    for (register, offset) in registers.iter().copied().zip(offsets) {
        let (base, relative) = if windowed {
            window.address(code, fixups, offset)?
        } else {
            (sp, offset as i64)
        };
        if restore {
            emit_load64(code, fixups, register, base, relative, Some(28))?;
        } else {
            emit_store64(
                code,
                fixups,
                base,
                register,
                relative,
                if windowed { 28 } else { 27 },
            )?;
        }
    }
    Ok(())
}

pub(super) fn shared_epilogue_label(
    function: &ir::Function,
    saved_registers: usize,
) -> Option<usize> {
    if retain_scalar() || saved_registers < 2 {
        return None;
    }
    let returns = function
        .blocks
        .iter()
        .filter(|block| {
            matches!(
                block.terminator,
                Terminator::Return(_) | Terminator::Return2(..) | Terminator::ReturnN(_)
            )
        })
        .count();
    if returns < 2 {
        return None;
    }
    // Two restores, SP adjustment and JALR are at least 16 bytes. One copy and
    // one 4-byte jump per return is strictly smaller for every count>=2.
    function
        .blocks
        .iter()
        .map(|block| block.label.0)
        .max()?
        .checked_add(1)
}

pub(super) fn emit_epilogue(
    code: &mut Vec<u8>,
    fixups: &LiteralFixups,
    registers: &[u8],
    save_base: usize,
    saves_return_address: bool,
    frame_bytes: usize,
) -> Result<(), String> {
    let sp = regalloc::SP_REG as u8;
    emit_saved_registers(code, fixups, registers, save_base, true)?;
    if saves_return_address {
        emit_load64(code, fixups, 1, sp, 0, Some(27))?;
    }
    emit_bounded_add(
        code,
        fixups,
        sp,
        sp,
        i64::try_from(frame_bytes).map_err(|_| "frame size exceeds signed address space")?,
        LITERAL_SHIFT_REG,
    )?;
    push_word(
        code,
        encoding::wide::encode_rr(instruction::wide::control::JALR, 0, 1, 0),
    );
    Ok(())
}

#[cfg(test)]
mod native_pairs;
#[cfg(test)]
mod tests;
