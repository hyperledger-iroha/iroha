use super::*;
use crate::{
    gates::{flex_gate::threads::SinglePhaseCoreManager, GateChip},
    halo2_proofs::halo2curves::{bn256::Fr, ff::PrimeField},
};

#[test]
fn test_fix_permutation_against_test_vectors() {
    let mut pool = SinglePhaseCoreManager::new(true, Default::default());
    let gate = GateChip::<Fr>::default();
    let ctx = pool.main();

    // https://extgit.iaik.tugraz.at/krypto/hadeshash/-/blob/master/code/test_vectors.txt
    // poseidonperm_x5_254_3
    {
        const R_F: usize = 8;
        const R_P: usize = 57;
        const T: usize = 3;
        const RATE: usize = 2;

        let spec = OptimizedPoseidonSpec::<Fr, T, RATE>::new::<R_F, R_P, 0>();

        let mut state = PoseidonState::<Fr, T, RATE> {
            s: [0u64, 1, 2].map(|v| ctx.load_constant(Fr::from(v))),
        };
        let inputs = [Fr::zero(); RATE].iter().map(|f| ctx.load_constant(*f)).collect_vec();
        state.permutation(ctx, &gate, &inputs, None, &spec); // avoid padding
        let state_0 = state.s;
        let expected = [
            "7853200120776062878684798364095072458815029376092732009249414926327459813530",
            "7142104613055408817911962100316808866448378443474503659992478482890339429929",
            "6549537674122432311777789598043107870002137484850126429160507761192163713804",
        ];
        for (word, expected) in state_0.into_iter().zip(expected.iter()) {
            assert_eq!(word.value(), &Fr::from_str_vartime(expected).unwrap());
        }
    }

    // https://extgit.iaik.tugraz.at/krypto/hadeshash/-/blob/master/code/test_vectors.txt
    // poseidonperm_x5_254_5
    {
        const R_F: usize = 8;
        const R_P: usize = 60;
        const T: usize = 5;
        const RATE: usize = 4;

        let spec = OptimizedPoseidonSpec::<Fr, T, RATE>::new::<R_F, R_P, 0>();

        let mut state = PoseidonState::<Fr, T, RATE> {
            s: [0u64, 1, 2, 3, 4].map(|v| ctx.load_constant(Fr::from(v))),
        };
        let inputs = [Fr::zero(); RATE].iter().map(|f| ctx.load_constant(*f)).collect_vec();
        state.permutation(ctx, &gate, &inputs, None, &spec);
        let state_0 = state.s;
        let expected: [&str; 5] = [
            "18821383157269793795438455681495246036402687001665670618754263018637548127333",
            "7817711165059374331357136443537800893307845083525445872661165200086166013245",
            "16733335996448830230979566039396561240864200624113062088822991822580465420551",
            "6644334865470350789317807668685953492649391266180911382577082600917830417726",
            "3372108894677221197912083238087960099443657816445944159266857514496320565191",
        ];
        for (word, expected) in state_0.into_iter().zip(expected.iter()) {
            assert_eq!(word.value(), &Fr::from_str_vartime(expected).unwrap());
        }
    }
}

#[test]
fn test_var_permutation_against_test_vectors() {
    let mut pool = SinglePhaseCoreManager::new(true, Default::default());
    let gate = GateChip::<Fr>::default();
    let ctx = pool.main();

    // https://extgit.iaik.tugraz.at/krypto/hadeshash/-/blob/master/code/test_vectors.txt
    // poseidonperm_x5_254_3
    {
        const R_F: usize = 8;
        const R_P: usize = 57;
        const T: usize = 3;
        const RATE: usize = 2;

        let spec = OptimizedPoseidonSpec::<Fr, T, RATE>::new::<R_F, R_P, 0>();

        let mut state = PoseidonState::<Fr, T, RATE> {
            s: [0u64, 1, 2].map(|v| ctx.load_constant(Fr::from(v))),
        };
        let inputs = [Fr::zero(); RATE].iter().map(|f| ctx.load_constant(*f)).collect_vec();
        let len = ctx.load_constant(Fr::from(RATE as u64));
        state.permutation(ctx, &gate, &inputs, Some(len), &spec); // avoid padding
        let state_0 = state.s;
        let expected = [
            "7853200120776062878684798364095072458815029376092732009249414926327459813530",
            "7142104613055408817911962100316808866448378443474503659992478482890339429929",
            "6549537674122432311777789598043107870002137484850126429160507761192163713804",
        ];
        for (word, expected) in state_0.into_iter().zip(expected.iter()) {
            assert_eq!(word.value(), &Fr::from_str_vartime(expected).unwrap());
        }
    }

    // https://extgit.iaik.tugraz.at/krypto/hadeshash/-/blob/master/code/test_vectors.txt
    // poseidonperm_x5_254_5
    {
        const R_F: usize = 8;
        const R_P: usize = 60;
        const T: usize = 5;
        const RATE: usize = 4;

        let spec = OptimizedPoseidonSpec::<Fr, T, RATE>::new::<R_F, R_P, 0>();

        let mut state = PoseidonState::<Fr, T, RATE> {
            s: [0u64, 1, 2, 3, 4].map(|v| ctx.load_constant(Fr::from(v))),
        };
        let inputs = [Fr::zero(); RATE].iter().map(|f| ctx.load_constant(*f)).collect_vec();
        let len = ctx.load_constant(Fr::from(RATE as u64));
        state.permutation(ctx, &gate, &inputs, Some(len), &spec);
        let state_0 = state.s;
        let expected: [&str; 5] = [
            "18821383157269793795438455681495246036402687001665670618754263018637548127333",
            "7817711165059374331357136443537800893307845083525445872661165200086166013245",
            "16733335996448830230979566039396561240864200624113062088822991822580465420551",
            "6644334865470350789317807668685953492649391266180911382577082600917830417726",
            "3372108894677221197912083238087960099443657816445944159266857514496320565191",
        ];
        for (word, expected) in state_0.into_iter().zip(expected.iter()) {
            assert_eq!(word.value(), &Fr::from_str_vartime(expected).unwrap());
        }
    }
}
