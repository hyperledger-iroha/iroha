//! Exact Unit value/privacy and staged NODE/WORD debits at original clocks.

use super::*;
use packet::{AFTER, AFTER_TAG, BEFORE, BEFORE_TAG};

const BORROWS: usize = 8;
/// Two four-limb subtraction chains and the original memory-cell privacy mask.
pub(super) struct Validation([F; BORROWS + 16]);
impl Validation {
    pub(super) fn new(gas: [&Fields; 2], memory: &Fields) -> Self {
        let mut result = Self([F::ZERO; BORROWS + 16]);
        for (stage, cost) in [ivm::call_gas::NODE, ivm::call_gas::WORD]
            .into_iter()
            .enumerate()
        {
            let before = packet::half(&gas[stage].0, BEFORE, 0);
            let mut borrow = 0;
            for limb in 0..4 {
                let a = (before >> (16 * limb)) & 0xffff;
                let b = (cost >> (16 * limb)) & 0xffff;
                borrow = u64::from(a < b + borrow);
                result.0[4 * stage + limb] = F(borrow);
            }
        }
        for bit in 0..16 {
            result.0[BORROWS + bit] = F((memory.0[BEFORE_TAG].0 >> bit) & 1);
        }
        result
    }

    pub(super) fn append_residues(
        &self,
        out: &mut impl Sink,
        first: u32,
        gas: [&Fields; 2],
        memory: &Fields,
    ) {
        for (stage, cost) in [ivm::call_gas::NODE, ivm::call_gas::WORD]
            .into_iter()
            .enumerate()
        {
            let port = &gas[stage].0;
            header(
                out,
                port,
                packet::Space::Owner,
                33,
                first + 22 + stage as u32,
                true,
            );
            out.push(port[BEFORE_TAG]);
            out.push(port[AFTER_TAG]);
            for offset in [BEFORE, AFTER] {
                out.extend(port[offset + 4..offset + 8].iter().copied());
            }
            for limb in 0..4 {
                let borrow = self.0[4 * stage + limb];
                let incoming = if limb == 0 {
                    F::ZERO
                } else {
                    self.0[4 * stage + limb - 1]
                };
                out.push(super::super::super::bit(borrow));
                // Original history proves these packet limbs are u16 values.
                out.push(
                    port[BEFORE + limb]
                        .sub(F((cost >> (16 * limb)) & 0xffff))
                        .sub(incoming)
                        .sub(port[AFTER + limb])
                        .add(borrow.mul(F(1 << 16))),
                );
            }
            out.push(self.0[4 * stage + 3]);
        }
        let port = &memory.0;
        header(
            out,
            port,
            packet::Space::Memory,
            (ivm::Memory::HEAP_START / 16) as u32,
            first + 24,
            false,
        );
        for limb in 0..8 {
            out.push(port[AFTER + limb].sub(port[BEFORE + limb]));
        }
        out.push(port[AFTER_TAG].sub(port[BEFORE_TAG]));
        // Unit owns only the low eight bytes of the root's aligned result cell.
        out.extend(port[BEFORE..BEFORE + 4].iter().copied());
        let mut mask = F::ZERO;
        for (bit, value) in self.0[BORROWS..].iter().copied().enumerate() {
            out.push(super::super::super::bit(value));
            mask = mask.add(value.mul(F(1 << bit)));
            if bit < 8 {
                out.push(value);
            }
        }
        out.push(port[BEFORE_TAG].sub(mask));
    }
    fn clear(&mut self) {
        for field in &mut self.0 {
            field.zeroize_v1();
        }
    }
}
impl Drop for Validation {
    fn drop(&mut self) {
        self.clear();
    }
}

fn header(
    out: &mut impl Sink,
    port: &[F; packet::WIDTH],
    space: packet::Space,
    index: u32,
    clock: u32,
    write: bool,
) {
    for (field, expected) in [
        (packet::SPACE, F(space as u64)),
        (packet::VM, F::ZERO),
        (packet::GENERATION, F::ZERO),
        (packet::INDEX, F(u64::from(index))),
        (packet::KEY, F(u64::from(index) + ((space as u64) << 56))),
        (packet::CLOCK, F(u64::from(clock))),
        (packet::ENABLED, F::ONE),
        (packet::WRITE, F(u64::from(write))),
    ] {
        out.push(port[field].sub(expected));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_unit_validation_and_gas_equations_have_degree_two() {
        use crate::execution_proofs::stark::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1;
        let degree = measured_maximum_affine_degree_v1(
            [0x76; 32],
            [BORROWS + 16 + 3 * packet::WIDTH, 0, 0, 0, 0],
            8,
            2,
            |row, _, _, _, _| {
                let witness = Validation(row[..BORROWS + 16].try_into().unwrap());
                let fields: [Fields; 3] = core::array::from_fn(|index| {
                    let start = BORROWS + 16 + index * packet::WIDTH;
                    Fields(row[start..start + packet::WIDTH].try_into().unwrap())
                });
                let mut residues = Vec::new();
                witness.append_residues(
                    &mut residues,
                    first(),
                    [&fields[0], &fields[1]],
                    &fields[2],
                );
                Ok::<_, core::convert::Infallible>(residues)
            },
        );
        assert_eq!(degree, 2);
        let mut scratch = Validation([F::ONE; BORROWS + 16]);
        scratch.clear();
        assert!(scratch.0.iter().all(|field| *field == F::ZERO));
    }
}
