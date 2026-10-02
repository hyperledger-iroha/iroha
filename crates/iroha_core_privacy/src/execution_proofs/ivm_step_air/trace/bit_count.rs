//! Shared zero-prefix equations over the common canonical Boolean source bits.
//!
//! Fetch constrains the leading-count selector to a Boolean. Routing the bits
//! forwards or backwards has degree two; a preceding prefix times the next
//! zero bit has degree three. Every prefix is uniquely Boolean by induction,
//! including on unrelated instructions and padding when this prefix owner is
//! selected. The public segment disables these equations only when multiply or
//! division owns the workspace as product digits. Private dispatch additionally
//! gives GCD its bounded radix-four workspace and disables the prefix owner; its
//! independent digit-range equations still constrain every shared cell.

use super::F;

pub(in super::super) const WIDTH: usize = 64;
pub(in super::super) const CONSTRAINTS: usize = WIDTH;

pub(in super::super) fn witness(bits: &[F], leading: bool) -> [F; WIDTH] {
    let mut previous = F::ONE;
    std::array::from_fn(|index| {
        let bit = bits[if leading { WIDTH - 1 - index } else { index }];
        previous = previous.mul(F::ONE.sub(bit));
        previous
    })
}

pub(in super::super) fn append_residues(out: &mut Vec<F>, prefixes: &[F], bits: &[F], leading: F) {
    let mut previous = F::ONE;
    for index in 0..WIDTH {
        let bit = F::ONE
            .sub(leading)
            .mul(bits[index])
            .add(leading.mul(bits[WIDTH - 1 - index]));
        out.push(prefixes[index].sub(previous.mul(F::ONE.sub(bit))));
        previous = prefixes[index];
    }
}
