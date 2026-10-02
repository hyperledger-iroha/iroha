//! Original Name structure and forced OID-census uniqueness constraints.
//!
//! TODO: the full original Name value census and UTF8/PrintableString state
//! machine remain required before complete parser equivalence or activation.
use super::*;

const NODE_CLASS: usize = CALENDAR_COLUMNS + 10;
const CLASS_ROLES: [u64; 5] = [
    ZkX509Rfc5280GrammarRoleV1::NameRdn as u64,
    ZkX509Rfc5280GrammarRoleV1::NameAttribute as u64,
    ZkX509Rfc5280GrammarRoleV1::CertificateIssuer as u64,
    ZkX509Rfc5280GrammarRoleV1::CertificateSubject as u64,
    ZkX509Rfc5280GrammarRoleV1::CrlIssuer as u64,
];
const NODE_NONEMPTY_INVERSE: usize = NODE_CLASS + 2 * CLASS_ROLES.len();
pub(super) const NODE_PREFIX_END: usize = NODE_NONEMPTY_INVERSE + 1;
const NAME: usize = BASE_C;
const NAME_INVERSE: usize = BASE_D;
const NAME_FIRST: usize = BASE_E;
const NAME_ID: usize = BASE_F;
const INSTANCE_REMAINDER: usize = BASE_SMALL_BITS;
const KEY_GAP: usize = INSTANCE_REMAINDER + 6;
pub(super) const RESIDUES: usize = 54;

pub(super) fn residues<A: PolynomialAirFieldV1>(
    row: &ZkX509Rfc5280StarkBaseRowV1<A>,
    next: &ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
) -> [A; RESIDUES] {
    let node = fixed[ZkX509Rfc5280StarkFamilyV1::SourceNode as usize];
    let fixed_byte = fixed[ZkX509Rfc5280StarkFamilyV1::FixedByte as usize];
    let mut output = [A::ZERO; RESIDUES];
    let mut index = 0;
    let mut push = |r| {
        output[index] = r;
        index += 1;
    };
    for (slot, role) in CLASS_ROLES.into_iter().enumerate() {
        let flag = row[NODE_CLASS + 2 * slot];
        let inverse = row[NODE_CLASS + 2 * slot + 1];
        let delta = row[BASE_ROLE].sub(A::from_base(F(role)));
        push(node.mul(flag).mul(flag.sub(A::ONE)));
        push(node.mul(flag).mul(delta));
        push(node.mul(delta.mul(inverse).sub(row[BASE_ACTIVE]).add(flag)));
    }
    let bounded_ordinal = row[NODE_CLASS].add(row[NODE_CLASS + 2]);
    for bit in 2..16 {
        push(
            node.mul(bounded_ordinal)
                .mul(row[GRAMMAR_CHILD_ORDINAL_BITS + bit]),
        );
    }
    let nonempty = row[NODE_CLASS]
        .add(row[NODE_CLASS + 4])
        .add(row[NODE_CLASS + 6])
        .add(row[NODE_CLASS + 8]);
    push(
        node.mul(nonempty)
            .mul(row[BASE_D].mul(row[NODE_NONEMPTY_INVERSE]).sub(A::ONE)),
    );
    push(
        node.mul(row[BASE_ACTIVE].sub(nonempty))
            .mul(row[NODE_NONEMPTY_INVERSE]),
    );
    let name = row[NAME];
    let first = row[NAME_FIRST];
    let role_delta = row[BASE_ROLE].sub(A::from_base(F(9)));
    push(fixed_byte.mul(name).mul(name.sub(A::ONE)));
    push(fixed_byte.mul(name).mul(role_delta));
    push(
        fixed_byte.mul(
            role_delta
                .mul(row[NAME_INVERSE])
                .sub(row[BASE_ACTIVE])
                .add(name),
        ),
    );
    push(fixed_byte.mul(first.sub(name.mul(row[BASE_IS_WRITE]))));
    push(fixed_byte.mul(row[NAME_ID]).mul(row[NAME_ID].sub(A::ONE)));
    let remainder = (0..6).fold(A::ZERO, |sum, bit| {
        sum.add(row[INSTANCE_REMAINDER + bit].mul_base(F(1 << bit)))
    });
    push(
        fixed_byte.mul(name).mul(
            row[BASE_H]
                .sub(row[NAME_ID].mul_base(F(1024)))
                .sub(remainder),
        ),
    );
    for bit in 0..6 {
        let bit = row[INSTANCE_REMAINDER + bit];
        push(fixed_byte.mul(name).mul(bit).mul(bit.sub(A::ONE)));
    }
    for bit in 0..5 {
        let bit = row[KEY_GAP + bit];
        push(fixed_byte.mul(first).mul(bit).mul(bit.sub(A::ONE)));
    }
    let gap = (0..5).fold(A::ZERO, |sum, bit| {
        sum.add(row[KEY_GAP + bit].mul_base(F(1 << bit)))
    });
    let key = row[BASE_DOCUMENT]
        .mul_base(F(8))
        .add(row[NAME_ID].mul_base(F(4)))
        .add(row[BASE_ENDPOINT_ROLE])
        .add(A::ONE);
    push(fixed_byte.mul(first).mul(row[BASE_STATE_AFTER].sub(key)));
    push(
        fixed_byte.mul(first).mul(
            row[BASE_STATE_AFTER]
                .sub(row[BASE_STATE_BEFORE])
                .sub(A::ONE)
                .sub(gap),
        ),
    );
    push(
        fixed_byte
            .mul(A::ONE.sub(first))
            .mul(row[BASE_STATE_AFTER].sub(row[BASE_STATE_BEFORE])),
    );
    push(
        fixed_byte
            .mul(fixed[FIX_ACTIVATION_CONTINUE])
            .mul(next[BASE_ACTIVE])
            .mul(next[BASE_STATE_BEFORE].sub(row[BASE_STATE_AFTER])),
    );
    push(
        fixed_byte
            .mul(fixed[FIX_EXPECTED])
            .mul(row[BASE_STATE_BEFORE]),
    );
    push(
        fixed_byte
            .mul(row[BASE_ACTIVE])
            .mul(A::ONE.sub(row[BASE_STRICT]))
            .mul(next[BASE_H].sub(row[BASE_H])),
    );
    assert_eq!(index, RESIDUES);
    output
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn populate_source_node(
    row: &mut ZkX509Rfc5280StarkBaseRowV1,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    for (slot, role) in CLASS_ROLES.into_iter().enumerate() {
        let delta = row[BASE_ROLE].sub(F(role));
        row[NODE_CLASS + 2 * slot] = F(u64::from(delta == F::ZERO));
        row[NODE_CLASS + 2 * slot + 1] = delta.inv().unwrap_or(F::ZERO);
    }
    if [0, 2, 3, 4]
        .into_iter()
        .any(|slot| row[NODE_CLASS + 2 * slot] == F::ONE)
    {
        row[NODE_NONEMPTY_INVERSE] = row[BASE_D]
            .inv()
            .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
    }
    if (row[NODE_CLASS] == F::ONE || row[NODE_CLASS + 2] == F::ONE) && row[BASE_CHILD].0 >= 4 {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    Ok(())
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn populate_fixed_byte(
    row: &mut ZkX509Rfc5280StarkBaseRowV1,
    previous: &mut u64,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let name = row[BASE_ROLE] == F(9);
    let delta = row[BASE_ROLE].sub(F(9));
    row[NAME] = F(u64::from(name));
    row[NAME_INVERSE] = delta.inv().unwrap_or(F::ZERO);
    row[NAME_FIRST] = row[NAME].mul(row[BASE_IS_WRITE]);
    row[BASE_STATE_BEFORE] = F(*previous);
    if name {
        let partition = row[BASE_H].0 / 1024;
        let remainder = row[BASE_H].0 % 1024;
        if partition > 1 || remainder >= 64 {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        row[NAME_ID] = F(partition);
        for bit in 0..6 {
            row[INSTANCE_REMAINDER + bit] = F((remainder >> bit) & 1);
        }
        if row[NAME_FIRST] == F::ONE {
            let key = row[BASE_DOCUMENT].0 * 8 + partition * 4 + row[BASE_ENDPOINT_ROLE].0 + 1;
            let gap = key
                .checked_sub(*previous)
                .and_then(|difference| difference.checked_sub(1))
                .filter(|gap| *gap < 32)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
            for bit in 0..5 {
                row[KEY_GAP + bit] = F((gap >> bit) & 1);
            }
            *previous = key;
        }
    }
    row[BASE_STATE_AFTER] = F(*previous);
    Ok(())
}
