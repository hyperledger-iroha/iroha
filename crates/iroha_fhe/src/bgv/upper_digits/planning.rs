//! Public resource preadmission and exact canonical shape replay.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct Preallocation {
    metadata: usize,
    work: u64,
    words: u64,
}
fn add(a: u64, b: u64) -> Result<u64, UpperDigitsError> {
    a.checked_add(b).ok_or(UpperDigitsError::SizeOverflow)
}
fn mul(a: u64, b: u64) -> Result<u64, UpperDigitsError> {
    a.checked_mul(b).ok_or(UpperDigitsError::SizeOverflow)
}
fn ceiling(required: usize, budget: usize, resource: &'static str) -> Result<(), UpperDigitsError> {
    if required > budget {
        Err(UpperDigitsError::Budget { resource })
    } else {
        Ok(())
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "preadmission accounts for the shared program, selected bank count and retention before any node allocation"
)]
pub(super) fn preallocation(
    profile: &BgvProfile,
    program: &UpperDigitsProgram,
    nodes: usize,
    operations: UpperDigitsOperations,
    banks: usize,
    depths: usize,
    retention: BankRetention,
    budget: UpperDigitsBudget,
) -> Result<Preallocation, UpperDigitsError> {
    // An admitted bound has at most W significant words. Natural operations
    // can retain their requested spare capacity: add requests W+1, product
    // requests 2W followed by degree scaling at most 2W+1. Division/clone
    // allocates from significant length. Charge that conservative requested
    // capacity, not merely the normalized length. Allocator over-allocation
    // beyond requested capacity remains outside this scoped element budget.
    let bound_words = sum(product(2, profile.noise_bound_words())?, 1)?;
    let bound_bytes = product(bound_words, size_of::<u64>())?;
    // Retained raw nodes, sorting/reference arrays, public shape simulation,
    // runtime arena and its two RnsPoly headers, and bound-word payloads. Public
    // shape simulation and request execution do not overlap; counting both is
    // conservative. Twenty further bound payloads cover planning temporaries.
    let each = sum(
        size_of::<Node>(),
        sum(
            3 * size_of::<usize>(),
            sum(
                size_of::<Option<CiphertextShape>>(),
                sum(
                    size_of::<Option<Ciphertext>>(),
                    sum(2 * size_of::<RnsPoly>(), product(2, bound_bytes)?)?,
                )?,
            )?,
        )?,
    )?;
    let geometry = program.plan().geometry();
    // Every completed bank gets a newly allocated output-container element,
    // two component headers, and a public bound. Incoming owner metadata is
    // already allocated by the caller and is explicitly outside this budget.
    // A streaming caller owns/charges its one scalar token and frontier; it
    // never creates this completed-bank container. Work uses the selected bank count.
    let completed = product(
        if retention == BankRetention::Collected {
            banks
        } else {
            0
        },
        sum(
            size_of::<Ciphertext>(),
            sum(2 * size_of::<RnsPoly>(), bound_bytes)?,
        )?,
    )?;
    let metadata = sum(
        completed,
        sum(
            sum(geometry.coefficient_bytes, geometry.program_bytes)?,
            sum(
                product(nodes, each)?,
                sum(
                    product(depths, size_of::<usize>())?,
                    sum(
                        product(profile.top_level() + 1, size_of::<u64>())?,
                        product(20, bound_bytes)?,
                    )?,
                )?,
            )?,
        )?,
    )?;
    ceiling(metadata, budget.metadata_bytes, "public metadata")?;
    let model = CostModel::new(profile);
    let top = profile.top_level();
    let products = mul(
        operations.ciphertext_multiplications,
        model.multiplication_units(top)?,
    )?;
    let plain = mul(
        operations.plaintext_multiplications,
        model.scalar_multiplication_units(top, 2)?,
    )?;
    let switches = if top == 0 {
        0
    } else {
        mul(
            mul(word(nodes)?, word(top)?)?,
            model.switch_down_units(top)?,
        )?
    };
    let work = mul(add(add(products, plain)?, switches)?, word(banks)?)?;
    if work > budget.canonical_arithmetic_units {
        return Err(UpperDigitsError::Budget {
            resource: "canonical arithmetic",
        });
    }
    let full = mul(word(profile.degree())?, word(top + 1)?)?;
    let component_ops = add(
        add(operations.ciphertext_additions, operations.negations)?,
        operations.exact_divisions,
    )?;
    let words = mul(
        add(
            mul(mul(component_ops, 2)?, full)?,
            mul(operations.plaintext_additions, full)?,
        )?,
        word(banks)?,
    )?;
    if words > budget.scalar_word_operations {
        return Err(UpperDigitsError::Budget {
            resource: "scalar words",
        });
    }
    Ok(Preallocation {
        metadata,
        work,
        words,
    })
}

fn polynomial_bytes(profile: &BgvProfile, level: usize) -> Result<usize, UpperDigitsError> {
    product(product(profile.degree(), level + 1)?, size_of::<u64>())
}
fn live_bytes(
    profile: &BgvProfile,
    shapes: &[Option<CiphertextShape>],
) -> Result<usize, UpperDigitsError> {
    shapes.iter().flatten().try_fold(0, |total, s| {
        sum(
            total,
            product(s.components(), polynomial_bytes(profile, s.level())?)?,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay(
    profile: &BgvProfile,
    input: &CiphertextShape,
    nodes: &[Node],
    order: &[usize],
    initial_uses: &[usize],
    levels: &[usize],
    output_id: usize,
    mut operations: UpperDigitsOperations,
    banks: usize,
    retention: BankRetention,
    budget: UpperDigitsBudget,
    preflight: Preallocation,
) -> Result<(CiphertextShape, UpperDigitsOperations, UpperDigitsPayload), UpperDigitsError> {
    let mut shapes = reserve(nodes.len())?;
    shapes.resize(nodes.len(), None);
    let mut uses = reserve(nodes.len())?;
    uses.extend_from_slice(initial_uses);
    shapes[0] = Some(input.clone());
    let mut active_depth = 0;
    let mut inner_peak = live_bytes(profile, &shapes)?;
    let mut backend_peak = 0;
    let mut arithmetic = 0_u64;
    let mut words = 0_u64;
    let model = CostModel::new(profile);
    for &index in order {
        let node = nodes[index];
        if matches!(node.operation, Operation::Input) {
            if index != 0 {
                return Err(UpperDigitsError::Program);
            }
            continue;
        }
        if node.depth != active_depth {
            let target = *levels
                .get(node.depth.checked_sub(1).ok_or(UpperDigitsError::Program)?)
                .ok_or(UpperDigitsError::Levels)?;
            for id in 0..shapes.len() {
                while shapes[id].as_ref().is_some_and(|s| s.level() > target) {
                    let old = shapes[id].as_ref().ok_or(UpperDigitsError::Program)?;
                    let level = old.level();
                    let next = profile.plan_switch_down(old)?;
                    inner_peak = inner_peak.max(sum(
                        live_bytes(profile, &shapes)?,
                        product(2, polynomial_bytes(profile, next.level())?)?,
                    )?);
                    arithmetic = add(arithmetic, model.switch_down_units(level)?)?;
                    operations.modulus_switches = add(operations.modulus_switches, 1)?;
                    shapes[id] = Some(next);
                }
                if shapes[id].as_ref().is_some_and(|s| s.level() != target) {
                    return Err(UpperDigitsError::Levels);
                }
            }
            active_depth = node.depth;
        }
        let (args, len) = node.operation.arguments();
        let first = shapes
            .get(args[0])
            .and_then(Option::as_ref)
            .ok_or(UpperDigitsError::Program)?;
        let level = first.level();
        let one = polynomial_bytes(profile, level)?;
        let other = |id: usize| {
            shapes
                .get(id)
                .and_then(Option::as_ref)
                .ok_or(UpperDigitsError::Program)
        };
        let mut extra = product(2, one)?;
        let result = match node.operation {
            Operation::Input => return Err(UpperDigitsError::Program),
            Operation::Multiply(_, b) => {
                arithmetic = add(arithmetic, model.multiplication_units(level)?)?;
                extra = product(5, one)?;
                // Tensor internally holds up to three further main-basis
                // polynomials beyond its eventual three-component result.
                // Shared key-switch scratch has its independent exact owner.
                backend_peak = backend_peak.max(product(3, one)?).max(
                    usize::try_from(model.key_switch_scratch_words(level)?)
                        .map_err(|_| UpperDigitsError::SizeOverflow)?
                        .checked_mul(8)
                        .ok_or(UpperDigitsError::SizeOverflow)?,
                );
                profile.plan_multiply(first, other(b)?)?
            }
            Operation::Add(_, b) | Operation::Subtract(_, b) => {
                words = add(
                    words,
                    mul(2, mul(word(profile.degree())?, word(level + 1)?)?)?,
                )?;
                profile.plan_add(first, other(b)?)?
            }
            Operation::MultiplyPlain(_, c) => {
                arithmetic = add(arithmetic, model.scalar_multiplication_units(level, 2)?)?;
                let scalar = first.plaintext_precision().fixed().canonicalize_i64(c);
                profile.plan_mul_public_scalar(first, scalar)?
            }
            Operation::AddPlain(_, c) => {
                words = add(words, mul(word(profile.degree())?, word(level + 1)?)?)?;
                backend_peak = backend_peak.max(one);
                profile.plan_add_plain(
                    first,
                    PlaintextBound {
                        linf: c.unsigned_abs(),
                        l1: c.unsigned_abs(),
                    },
                )?
            }
            Operation::Negate(_) => {
                words = add(
                    words,
                    mul(2, mul(word(profile.degree())?, word(level + 1)?)?)?,
                )?;
                first.clone()
            }
            Operation::DivideProven(parent) => {
                if uses[parent] != 1
                    || !matches!(nodes[parent].operation, Operation::Subtract(..))
                    || nodes[parent].precision != node.precision + 1
                {
                    return Err(UpperDigitsError::Program);
                }
                extra = 0;
                words = add(
                    words,
                    mul(2, mul(word(profile.degree())?, word(level + 1)?)?)?,
                )?;
                let precision = profile.plaintext_precision(node.precision)?;
                let bound = first
                    .bound()
                    .div_floor_u64(precision.prime())
                    .ok_or(UpperDigitsError::Program)?;
                profile.plan_claimed(precision, level, 2, bound)?
            }
        };
        if result.plaintext_precision().exponent() != node.precision {
            return Err(UpperDigitsError::Program);
        }
        inner_peak = inner_peak.max(sum(live_bytes(profile, &shapes)?, extra)?);
        shapes[index] = Some(result);
        for &arg in &args[..len] {
            uses[arg] = uses[arg].checked_sub(1).ok_or(UpperDigitsError::Program)?;
            if uses[arg] == 0 {
                shapes[arg] = None;
            }
        }
        if uses[index] == 0 {
            shapes[index] = None;
        }
    }
    let output = shapes
        .get_mut(output_id)
        .and_then(Option::take)
        .ok_or(UpperDigitsError::Program)?;
    if shapes.iter().any(Option::is_some) {
        return Err(UpperDigitsError::Program);
    }
    let input_bytes = product(2, polynomial_bytes(profile, input.level())?)?;
    let output_bytes = product(2, polynomial_bytes(profile, output.level())?)?;
    let owned = if retention == BankRetention::Streaming {
        inner_peak
    } else {
        (0..banks).try_fold(0_usize, |peak, done| {
            Ok::<_, UpperDigitsError>(peak.max(sum(
                sum(
                    product(banks - done - 1, input_bytes)?,
                    product(done, output_bytes)?,
                )?,
                inner_peak,
            )?))
        })?
    };
    let plaintext = if operations.plaintext_additions == 0 {
        0
    } else {
        product(profile.degree(), size_of::<u64>())?
    };
    ceiling(owned, budget.owned_residue_bytes, "owned residues")?;
    ceiling(plaintext, budget.plaintext_bytes, "public plaintext")?;
    ceiling(backend_peak, budget.backend_scratch_bytes, "kernel scratch")?;
    arithmetic = mul(arithmetic, word(banks)?)?;
    words = mul(words, word(banks)?)?;
    if arithmetic > preflight.work || words > preflight.words {
        return Err(UpperDigitsError::Program);
    }
    let payload = UpperDigitsPayload {
        metadata_bytes: preflight.metadata,
        owned_residue_bytes: owned,
        single_bank_residue_bytes: inner_peak,
        plaintext_bytes: plaintext,
        backend_scratch_bytes: backend_peak,
        preallocation_arithmetic_units: preflight.work,
        preallocation_scalar_word_operations: preflight.words,
        canonical_arithmetic_units: arithmetic,
        scalar_word_operations: words,
    };
    Ok((output, operations, payload))
}
