//! Explicit ordinary113 State consumer grammar. Inactive slots create no Mint authority.
//! Active ordinary MintFold must supply its distinct finalized-source and private opening
//! consumer; until that relation is installed, it fails before State graph construction.
use super::{
    KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1,
    ordinary_mint_circuit::ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1,
    ordinary_mint_public::ORDINARY_MINT_PUBLIC_PREFIX_V1,
};
use crate::kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, from_u128};
use halo2_base::{
    AssignedValue, Context,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};

#[path = "ordinary_state_mint_active_bindings.rs"]
mod active_bindings;
pub(super) use active_bindings::{
    OrdinaryMintStateBindingCellsV1, OrdinaryMintStateOpeningV1,
    constrain_ordinary_mint_state_bindings_v1,
};

pub(super) fn inactive_column<F: KagemushaPoseidonFieldV1>(
    history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Vec<F> {
    let mut column = vec![F::ZERO; ORDINARY_MINT_PUBLIC_PREFIX_V1];
    column.extend(
        history
            .chunks_exact(16)
            .map(|b| from_u128::<F>(u128::from_le_bytes(b.try_into().expect("exact history16")))),
    );
    column
}
pub(super) fn require_inactive_column<F: KagemushaPoseidonFieldV1>(
    column: &[Vec<F>],
    history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<(), String> {
    if column != [inactive_column(history)] {
        return Err("ordinary State inactive Mint113 column/history differs".into());
    }
    Ok(())
}
pub(super) fn constrain_inactive_column<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    column: &[AssignedValue<F>],
    mint: AssignedValue<F>,
) -> Result<(), String> {
    if column.len() != ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("ordinary State Mint113 column is truncated".into());
    }
    range.gate().assert_is_const(ctx, &mint, &F::ZERO);
    for cell in &column[..ORDINARY_MINT_PUBLIC_PREFIX_V1] {
        range.gate().assert_is_const(ctx, cell, &F::ZERO);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
    };
    fn both<F: KagemushaPoseidonFieldV1>() {
        let history = [37; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1];
        let good = inactive_column::<F>(&history);
        assert_eq!(good.len(), 113);
        assert!(require_inactive_column(&[good.clone()], &history).is_ok());
        let mut changed = good.clone();
        changed[0] = F::ONE;
        assert!(require_inactive_column(&[changed], &history).is_err());
        let mut changed = good.clone();
        changed[79] += F::ONE;
        assert!(require_inactive_column(&[changed], &history).is_err());
        assert!(require_inactive_column(&[good[..84].to_vec()], &history).is_err());
        for (offset, mint, pass) in [
            (None, 0, true),
            (Some(0), 0, false),
            (Some(78), 0, false),
            (None, 1, false),
        ] {
            let mut b = BaseCircuitBuilder::<F>::new(false)
                .use_k(10)
                .use_lookup_bits(9)
                .use_instance_columns(1);
            let range = b.range_chip();
            let ctx = b.main(0);
            let mut data = good.clone();
            if let Some(i) = offset {
                data[i] = F::ONE;
            }
            let assigned = data
                .into_iter()
                .map(|v| ctx.load_witness(v))
                .collect::<Vec<_>>();
            let enabled = ctx.load_witness(F::from(mint));
            constrain_inactive_column(ctx, &range, &assigned, enabled).unwrap();
            b.assigned_instances = vec![Vec::new()];
            b.calculate_params(Some(9));
            assert_eq!(
                MockProver::run(10, &b, vec![Vec::new()])
                    .unwrap()
                    .verify()
                    .is_ok(),
                pass
            );
        }
    }
    #[test]
    fn ordinary_inactive_mint113_has_explicit_neutral_semantics_and_rejects_money_selector() {
        both::<Fp>();
        both::<Fq>();
    }
}
