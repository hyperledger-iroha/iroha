//! Linear-scan register allocator and stack frame layout for Kotodama IR.
use std::collections::{HashMap, HashSet};

use super::ir::{BasicBlock, Function, Instr, Label, Temp, Terminator};

/// Result of register allocation for a function.
#[derive(Debug, PartialEq)]
pub struct Allocation {
    /// Mapping from IR temporaries to physical registers.
    pub regs: HashMap<Temp, usize>,
    /// Mapping from spilled temporaries to stack offsets.
    pub stack: HashMap<Temp, usize>,
    /// Total frame size in bytes (16-byte aligned).
    pub frame_size: usize,
}

/// Registers r10-r22 are used for argument passing.
pub const ARG_REGS: [usize; 13] = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22];
/// r10 also holds the first return value.
pub const RET_REG: usize = 10;
/// ABI limit for multi-value returns carried in r10..r17.
pub const MAX_RETURN_VALUES: usize = 8;
/// r31 acts as the stack pointer.
pub const SP_REG: usize = 31;
/// r30 may be used as a frame pointer.
pub const FP_REG: usize = 30;

// Pool of allocatable registers (see policy above)
const ALLOC_POOL: &[usize] = &[2, 3, 4, 5, 6, 7, 8, 9, 23, 24];

#[derive(Clone, Copy, Debug)]
struct Interval {
    temp: Temp,
    start: usize,
    end: usize,
}

/// Allocate registers for a function using a single-pass linear scan.
pub fn allocate(func: &Function) -> Allocation {
    let mut intervals: HashMap<Temp, Interval> = HashMap::new();
    let mut tuple_defs: HashMap<Temp, Vec<Temp>> = HashMap::new();
    let mut position: usize = 0;
    let block_count = func.blocks.len();
    let mut label_to_idx: HashMap<Label, usize> = HashMap::new();
    for (idx, block) in func.blocks.iter().enumerate() {
        label_to_idx.insert(block.label, idx);
    }
    let mut block_uses: Vec<HashSet<Temp>> = Vec::with_capacity(block_count);
    let mut block_defs: Vec<HashSet<Temp>> = Vec::with_capacity(block_count);
    let mut block_succs: Vec<Vec<usize>> = Vec::with_capacity(block_count);
    for block in &func.blocks {
        let (uses, defs) = block_uses_defs(block);
        block_uses.push(uses);
        block_defs.push(defs);
        block_succs.push(block_successors(block, &label_to_idx));
    }
    let (live_in, _live_out) = compute_liveness(&block_uses, &block_defs, &block_succs);
    let mut block_end_pos: Vec<usize> = Vec::with_capacity(block_count);

    for block in &func.blocks {
        for instr in &block.instrs {
            visit_instr_uses(instr, |temp| add_use(&mut intervals, temp, position));
            if let Some(dest) = dest_temp(instr) {
                add_def(&mut intervals, dest, position);
            }
            if let Instr::TuplePack { dest, items } = instr {
                tuple_defs.insert(*dest, items.clone());
            }
            if let Instr::MapLoadPair {
                dest_key, dest_val, ..
            } = instr
            {
                add_def(&mut intervals, *dest_key, position);
                add_def(&mut intervals, *dest_val, position);
            }
            if let Instr::CallMulti { dests, .. } = instr {
                for dest in dests {
                    add_def(&mut intervals, *dest, position);
                }
            }
            position = position.saturating_add(1);
        }
        visit_terminator_uses(&block.terminator, |temp| {
            add_use(&mut intervals, temp, position)
        });
        block_end_pos.push(position);
        position = position.saturating_add(1);
    }

    for (block_idx, succs) in block_succs.iter().enumerate() {
        for &succ in succs {
            if succ <= block_idx {
                let end_pos = block_end_pos[block_idx];
                for temp in &live_in[succ] {
                    intervals
                        .entry(*temp)
                        .and_modify(|iv| {
                            if iv.end < end_pos {
                                iv.end = end_pos;
                            }
                        })
                        .or_insert(Interval {
                            temp: *temp,
                            start: end_pos,
                            end: end_pos,
                        });
                }
            }
        }
    }

    extend_tuple_intervals(&mut intervals, &tuple_defs);

    let mut interval_list: Vec<Interval> = intervals.values().copied().collect();
    interval_list.sort_by_key(|iv| (iv.start, iv.temp.0));
    let debug_intervals = interval_list.clone();

    let mut allocation = Allocation {
        regs: HashMap::new(),
        stack: HashMap::new(),
        frame_size: 0,
    };
    let mut active: Vec<(usize, Temp, usize)> = Vec::new();
    let mut free_regs: Vec<usize> = ALLOC_POOL.to_vec();
    let mut next_slot: usize = 0;

    for interval in interval_list {
        expire_old_intervals(interval.start, &mut active, &mut free_regs);

        if let Some(reg) = free_regs.pop() {
            allocation.regs.insert(interval.temp, reg);
            active.push((interval.end, interval.temp, reg));
            active.sort_by_key(|(end, _, _)| *end);
            continue;
        }

        if let Some((idx, _)) = active
            .iter()
            .enumerate()
            .max_by_key(|(_, (end, _, _))| *end)
        {
            let (spill_end, spill_temp, spill_reg) = active[idx];
            if spill_end > interval.end {
                allocation.stack.entry(spill_temp).or_insert_with(|| {
                    let offset = next_slot;
                    next_slot += 8;
                    offset
                });
                allocation.regs.remove(&spill_temp);
                active.remove(idx);
                allocation.regs.insert(interval.temp, spill_reg);
                active.push((interval.end, interval.temp, spill_reg));
                active.sort_by_key(|(end, _, _)| *end);
                continue;
            }
        }

        allocation.stack.entry(interval.temp).or_insert_with(|| {
            let offset = next_slot;
            next_slot += 8;
            offset
        });
    }

    if !next_slot.is_multiple_of(16) {
        next_slot += 16 - (next_slot % 16);
    }
    allocation.frame_size = next_slot;
    if crate::dev_env::debug_regalloc_enabled() {
        eprintln!(
            "[regalloc] function {} frame {}",
            func.name, allocation.frame_size
        );
        for interval in debug_intervals {
            let reg = allocation.regs.get(&interval.temp).copied();
            let stack = allocation.stack.get(&interval.temp).copied();
            eprintln!(
                "  temp {:?} start {} end {} => reg {:?} stack {:?}",
                interval.temp, interval.start, interval.end, reg, stack
            );
        }
    }
    allocation
}

fn extend_tuple_intervals(
    intervals: &mut HashMap<Temp, Interval>,
    tuple_defs: &HashMap<Temp, Vec<Temp>>,
) {
    fn extend_tuple_items(
        tuple: Temp,
        tuple_end: usize,
        intervals: &mut HashMap<Temp, Interval>,
        tuple_defs: &HashMap<Temp, Vec<Temp>>,
        visiting: &mut HashSet<Temp>,
    ) {
        if !visiting.insert(tuple) {
            return;
        }
        if let Some(items) = tuple_defs.get(&tuple) {
            for item in items {
                intervals
                    .entry(*item)
                    .and_modify(|iv| {
                        if iv.end < tuple_end {
                            iv.end = tuple_end;
                        }
                    })
                    .or_insert(Interval {
                        temp: *item,
                        start: tuple_end,
                        end: tuple_end,
                    });
                extend_tuple_items(*item, tuple_end, intervals, tuple_defs, visiting);
            }
        }
        visiting.remove(&tuple);
    }

    let mut visiting: HashSet<Temp> = HashSet::new();
    let tuples: Vec<(Temp, usize)> = tuple_defs
        .keys()
        .filter_map(|tuple| intervals.get(tuple).map(|iv| (*tuple, iv.end)))
        .collect();
    for (tuple, tuple_end) in tuples {
        extend_tuple_items(tuple, tuple_end, intervals, tuple_defs, &mut visiting);
    }
}

fn add_def(intervals: &mut HashMap<Temp, Interval>, temp: Temp, pos: usize) {
    intervals
        .entry(temp)
        .and_modify(|iv| {
            iv.start = iv.start.min(pos);
            iv.end = iv.end.max(pos);
        })
        .or_insert(Interval {
            temp,
            start: pos,
            end: pos,
        });
}

fn add_use(intervals: &mut HashMap<Temp, Interval>, temp: Temp, pos: usize) {
    intervals
        .entry(temp)
        .and_modify(|iv| iv.end = iv.end.max(pos))
        .or_insert(Interval {
            temp,
            start: pos,
            end: pos,
        });
}

fn expire_old_intervals(
    current_start: usize,
    active: &mut Vec<(usize, Temp, usize)>,
    free_regs: &mut Vec<usize>,
) {
    let mut idx = 0;
    while idx < active.len() {
        if active[idx].0 < current_start {
            free_regs.push(active[idx].2);
            active.remove(idx);
        } else {
            idx += 1;
        }
    }
}

fn block_uses_defs(block: &BasicBlock) -> (HashSet<Temp>, HashSet<Temp>) {
    let mut uses = HashSet::new();
    let mut defs = HashSet::new();
    for instr in &block.instrs {
        visit_instr_uses(instr, |temp| {
            if !defs.contains(&temp) {
                uses.insert(temp);
            }
        });
        if let Some(dest) = dest_temp(instr) {
            defs.insert(dest);
        }
        match instr {
            Instr::MapLoadPair {
                dest_key, dest_val, ..
            } => {
                defs.insert(*dest_key);
                defs.insert(*dest_val);
            }
            Instr::CallMulti { dests, .. } => {
                for dest in dests {
                    defs.insert(*dest);
                }
            }
            _ => {}
        }
    }
    visit_terminator_uses(&block.terminator, |temp| {
        if !defs.contains(&temp) {
            uses.insert(temp);
        }
    });
    (uses, defs)
}

fn block_successors(block: &BasicBlock, label_to_idx: &HashMap<Label, usize>) -> Vec<usize> {
    match block.terminator {
        Terminator::Jump(label) => label_to_idx.get(&label).copied().into_iter().collect(),
        Terminator::Branch {
            then_bb, else_bb, ..
        } => {
            let mut out = Vec::with_capacity(2);
            if let Some(idx) = label_to_idx.get(&then_bb).copied() {
                out.push(idx);
            }
            if let Some(idx) = label_to_idx.get(&else_bb).copied() {
                out.push(idx);
            }
            out
        }
        Terminator::Return(_) | Terminator::Return2(_, _) | Terminator::ReturnN(_) => Vec::new(),
    }
}

fn compute_liveness(
    block_uses: &[HashSet<Temp>],
    block_defs: &[HashSet<Temp>],
    block_succs: &[Vec<usize>],
) -> (Vec<HashSet<Temp>>, Vec<HashSet<Temp>>) {
    let block_count = block_uses.len();
    let mut live_in: Vec<HashSet<Temp>> = vec![HashSet::new(); block_count];
    let mut live_out: Vec<HashSet<Temp>> = vec![HashSet::new(); block_count];
    let mut changed = true;
    while changed {
        changed = false;
        for idx in (0..block_count).rev() {
            let mut out = HashSet::new();
            for &succ in &block_succs[idx] {
                out.extend(live_in[succ].iter().copied());
            }
            let mut in_set = block_uses[idx].clone();
            for temp in out.iter() {
                if !block_defs[idx].contains(temp) {
                    in_set.insert(*temp);
                }
            }
            if out != live_out[idx] || in_set != live_in[idx] {
                live_out[idx] = out;
                live_in[idx] = in_set;
                changed = true;
            }
        }
    }
    (live_in, live_out)
}

fn visit_instr_uses<F: FnMut(Temp)>(instr: &Instr, mut f: F) {
    use Instr::*;
    match instr {
        Const { .. }
        | StringConst { .. }
        | LoadVar { .. }
        | MapNew { .. }
        | CreateNftsForAllUsers
        | SubscriptionBill
        | SubscriptionRecordUsage
        | DataRef { .. }
        | GetAuthority { .. }
        | GetTriggerEvent { .. }
        | TransferBatchBegin
        | TransferBatchEnd => {}
        Binary { left, right, .. } => {
            f(*left);
            f(*right);
        }
        Unary { operand, .. } => f(*operand),
        NumericFromInt { value, .. } | NumericToInt { value, .. } => f(*value),
        NumericBinary { left, right, .. } | NumericCompare { left, right, .. } => {
            f(*left);
            f(*right);
        }
        Min { a, b, .. } | Max { a, b, .. } | Gcd { a, b, .. } | Mean { a, b, .. } => {
            f(*a);
            f(*b);
        }
        DivCeil { num, denom, .. } => {
            f(*num);
            f(*denom);
        }
        Abs { src, .. } => f(*src),
        Isqrt { src, .. } => f(*src),
        Poseidon2 { a, b, .. } => {
            f(*a);
            f(*b);
        }
        Poseidon6 { args, .. } => {
            for temp in args {
                f(*temp);
            }
        }
        Pubkgen { src, .. } => f(*src),
        Valcom { value, blind, .. } => {
            f(*value);
            f(*blind);
        }
        RegisterAsset {
            name,
            symbol,
            quantity,
            mintable,
        } => {
            f(*name);
            f(*symbol);
            f(*quantity);
            f(*mintable);
        }
        CreateNewAsset {
            name,
            symbol,
            quantity,
            account,
            mintable,
        } => {
            f(*name);
            f(*symbol);
            f(*quantity);
            f(*account);
            f(*mintable);
        }
        TransferAsset {
            from,
            to,
            asset,
            amount,
        } => {
            f(*from);
            f(*to);
            f(*asset);
            f(*amount);
        }
        MintAsset {
            account,
            asset,
            amount,
        }
        | BurnAsset {
            account,
            asset,
            amount,
        } => {
            f(*account);
            f(*asset);
            f(*amount);
        }
        AssertEq { left, right } => {
            f(*left);
            f(*right);
        }
        Assert { cond } => f(*cond),
        AbortIf { cond } => f(*cond),
        Info { msg } => f(*msg),
        PointerFromString { src, .. } => f(*src),
        MapGet { map, key, .. } => {
            f(*map);
            f(*key);
        }
        MapLoadPair { map, .. } => f(*map),
        MapSet { map, key, value } => {
            f(*map);
            f(*key);
            f(*value);
        }
        Load64Imm { base, .. } => f(*base),
        TuplePack { items, .. } => {
            for temp in items {
                f(*temp);
            }
        }
        TupleGet { tuple, .. } => f(*tuple),
        Copy { src, .. } => f(*src),
        SetExecutionDepth { value } => f(*value),
        SetVl { value } => f(*value),
        Call { args, .. } | CallMulti { args, .. } => {
            for arg in args {
                f(*arg);
            }
        }
        SetAccountDetail {
            account,
            key,
            value,
        } => {
            f(*account);
            f(*key);
            f(*value);
        }
        CreateNft { nft, owner } => {
            f(*nft);
            f(*owner);
        }
        SetNftData { nft, json } => {
            f(*nft);
            f(*json);
        }
        BurnNft { nft } => f(*nft),
        TransferNft { from, nft, to } => {
            f(*from);
            f(*nft);
            f(*to);
        }
        RegisterDomain { domain } | UnregisterDomain { domain } => f(*domain),
        TransferDomain { domain, to } => {
            f(*domain);
            f(*to);
        }
        RegisterAccount { account } | UnregisterAccount { account } => f(*account),
        GrantPermission { account, token } | RevokePermission { account, token } => {
            f(*account);
            f(*token);
        }
        GrantRole { account, name } | RevokeRole { account, name } => {
            f(*account);
            f(*name);
        }
        UnregisterAsset { asset } => f(*asset),
        RegisterPeer { json } | UnregisterPeer { json } | CreateTrigger { json } => f(*json),
        CreateRole { name, json } => {
            f(*name);
            f(*json);
        }
        RemoveTrigger { name } | DeleteRole { name } => f(*name),
        SetTriggerEnabled { name, enabled } => {
            f(*name);
            f(*enabled);
        }
        Instr::Sm3Hash { message, .. }
        | Instr::Sha256Hash { message, .. }
        | Instr::Sha3Hash { message, .. } => f(*message),
        Instr::Sm2Verify {
            message,
            signature,
            public_key,
            distid,
            ..
        } => {
            f(*message);
            f(*signature);
            f(*public_key);
            if let Some(d) = distid {
                f(*d);
            }
        }
        Instr::VerifySignature {
            message,
            signature,
            public_key,
            scheme,
            ..
        } => {
            f(*message);
            f(*signature);
            f(*public_key);
            f(*scheme);
        }
        Instr::Sm4GcmSeal {
            key,
            nonce,
            aad,
            plaintext,
            ..
        } => {
            f(*key);
            f(*nonce);
            f(*aad);
            f(*plaintext);
        }
        Instr::Sm4GcmOpen {
            key,
            nonce,
            aad,
            ciphertext_and_tag,
            ..
        } => {
            f(*key);
            f(*nonce);
            f(*aad);
            f(*ciphertext_and_tag);
        }
        Instr::Sm4CcmSeal {
            key,
            nonce,
            aad,
            plaintext,
            tag_len,
            ..
        } => {
            f(*key);
            f(*nonce);
            f(*aad);
            f(*plaintext);
            if let Some(t) = tag_len {
                f(*t);
            }
        }
        Instr::Sm4CcmOpen {
            key,
            nonce,
            aad,
            ciphertext_and_tag,
            tag_len,
            ..
        } => {
            f(*key);
            f(*nonce);
            f(*aad);
            f(*ciphertext_and_tag);
            if let Some(t) = tag_len {
                f(*t);
            }
        }
        Instr::ZkVerify { payload, .. }
        | Instr::VendorExecuteInstruction { payload }
        | Instr::VendorExecuteQuery { payload, .. } => f(*payload),
        StateGet { path, .. } => f(*path),
        StateSet { path, value } => {
            f(*path);
            f(*value);
        }
        StateDel { path } => f(*path),
        DecodeInt { blob, .. } | JsonDecode { blob, .. } | NameDecode { blob, .. } => f(*blob),
        TlvLen { value, .. } => f(*value),
        JsonGetInt { json, key, .. }
        | JsonGetNumeric { json, key, .. }
        | JsonGetJson { json, key, .. }
        | JsonGetName { json, key, .. }
        | JsonGetAccountId { json, key, .. }
        | JsonGetNftId { json, key, .. }
        | JsonGetBlobHex { json, key, .. } => {
            f(*json);
            f(*key);
        }
        SchemaDecode { schema, blob, .. } => {
            f(*schema);
            f(*blob);
        }
        EncodeInt { value, .. } | PointerToNorito { value, .. } => f(*value),
        PointerFromNorito { blob, .. } => f(*blob),
        PathMapKey { base, key, .. } => {
            f(*base);
            f(*key);
        }
        PathMapKeyNorito { base, key_blob, .. } => {
            f(*base);
            f(*key_blob);
        }
        JsonEncode { json, .. } => f(*json),
        SchemaEncode { schema, json, .. } => {
            f(*schema);
            f(*json);
        }
        SchemaInfo { schema, .. } => f(*schema),
        BuildSubmitBallotInline {
            election_id,
            ciphertext,
            nullifier,
            backend,
            proof,
            vk,
            ..
        } => {
            f(*election_id);
            f(*ciphertext);
            f(*nullifier);
            f(*backend);
            f(*proof);
            f(*vk);
        }
        BuildUnshieldInline {
            asset,
            to,
            amount,
            inputs,
            backend,
            proof,
            vk,
            ..
        } => {
            f(*asset);
            f(*to);
            f(*amount);
            f(*inputs);
            f(*backend);
            f(*proof);
            f(*vk);
        }
        PointerEq { left, right, .. } => {
            f(*left);
            f(*right);
        }
        VrfVerify {
            input,
            public_key,
            proof,
            variant,
            ..
        } => {
            f(*input);
            f(*public_key);
            f(*proof);
            f(*variant);
        }
        VrfVerifyBatch { batch, .. } => f(*batch),
        AxtBegin { descriptor } => f(*descriptor),
        AxtTouch { dsid, manifest } => {
            f(*dsid);
            if let Some(m) = manifest {
                f(*m);
            }
        }
        VerifyDsProof { dsid, proof } => {
            f(*dsid);
            if let Some(p) = proof {
                f(*p);
            }
        }
        UseAssetHandle {
            handle,
            intent,
            proof,
        } => {
            f(*handle);
            f(*intent);
            if let Some(p) = proof {
                f(*p);
            }
        }
        AxtCommit => {}
    }
}

fn visit_terminator_uses<F: FnMut(Temp)>(term: &Terminator, mut f: F) {
    match term {
        Terminator::Return(Some(temp)) => f(*temp),
        Terminator::Return2(t0, t1) => {
            f(*t0);
            f(*t1);
        }
        Terminator::ReturnN(vals) => {
            for temp in vals {
                f(*temp);
            }
        }
        Terminator::Branch { cond, .. } => f(*cond),
        Terminator::Return(None) | Terminator::Jump(_) => {}
    }
}

fn dest_temp(instr: &Instr) -> Option<Temp> {
    match instr {
        Instr::PointerEq { dest, .. }
        | Instr::Const { dest, .. }
        | Instr::StringConst { dest, .. }
        | Instr::DataRef { dest, .. }
        | Instr::Binary { dest, .. }
        | Instr::Unary { dest, .. }
        | Instr::Min { dest, .. }
        | Instr::Max { dest, .. }
        | Instr::Abs { dest, .. }
        | Instr::DivCeil { dest, .. }
        | Instr::Gcd { dest, .. }
        | Instr::Mean { dest, .. }
        | Instr::Isqrt { dest, .. }
        | Instr::LoadVar { dest, .. }
        | Instr::Poseidon2 { dest, .. }
        | Instr::Poseidon6 { dest, .. }
        | Instr::Pubkgen { dest, .. }
        | Instr::Valcom { dest, .. }
        | Instr::MapNew { dest }
        | Instr::GetAuthority { dest }
        | Instr::GetTriggerEvent { dest }
        | Instr::Copy { dest, .. }
        | Instr::PointerFromString { dest, .. }
        | Instr::PointerToNorito { dest, .. }
        | Instr::PointerFromNorito { dest, .. }
        | Instr::Load64Imm { dest, .. }
        | Instr::StateGet { dest, .. }
        | Instr::NumericFromInt { dest, .. }
        | Instr::NumericToInt { dest, .. }
        | Instr::NumericBinary { dest, .. }
        | Instr::NumericCompare { dest, .. } => Some(*dest),
        Instr::SchemaInfo { dest, .. } => Some(*dest),
        Instr::Sm3Hash { dest, .. }
        | Instr::Sha256Hash { dest, .. }
        | Instr::Sha3Hash { dest, .. } => Some(*dest),
        Instr::Sm2Verify { dest, .. } => Some(*dest),
        Instr::VerifySignature { dest, .. } => Some(*dest),
        Instr::Sm4GcmSeal { dest, .. } => Some(*dest),
        Instr::Sm4GcmOpen { dest, .. } => Some(*dest),
        Instr::Sm4CcmSeal { dest, .. } => Some(*dest),
        Instr::Sm4CcmOpen { dest, .. } => Some(*dest),
        Instr::VrfVerify { dest, .. } => Some(*dest),
        Instr::VrfVerifyBatch { dest, .. } => Some(*dest),
        Instr::MapGet { dest, .. } => Some(*dest),
        Instr::DecodeInt { dest, .. } => Some(*dest),
        Instr::TlvLen { dest, .. } => Some(*dest),
        Instr::EncodeInt { dest, .. } => Some(*dest),
        Instr::PathMapKey { dest, .. } => Some(*dest),
        Instr::PathMapKeyNorito { dest, .. } => Some(*dest),
        Instr::JsonEncode { dest, .. } => Some(*dest),
        Instr::JsonDecode { dest, .. } => Some(*dest),
        Instr::JsonGetInt { dest, .. }
        | Instr::JsonGetNumeric { dest, .. }
        | Instr::JsonGetJson { dest, .. }
        | Instr::JsonGetName { dest, .. }
        | Instr::JsonGetAccountId { dest, .. }
        | Instr::JsonGetNftId { dest, .. }
        | Instr::JsonGetBlobHex { dest, .. } => Some(*dest),
        Instr::NameDecode { dest, .. } => Some(*dest),
        Instr::SchemaEncode { dest, .. } => Some(*dest),
        Instr::SchemaDecode { dest, .. } => Some(*dest),
        Instr::TuplePack { dest, .. } => Some(*dest),
        Instr::TupleGet { dest, .. } => Some(*dest),
        Instr::BuildSubmitBallotInline { dest, .. } => Some(*dest),
        Instr::BuildUnshieldInline { dest, .. } => Some(*dest),
        Instr::VendorExecuteQuery { dest, .. } => Some(*dest),
        Instr::Call { dest, .. } => dest.as_ref().copied(),
        Instr::GrantPermission { .. }
        | Instr::RevokePermission { .. }
        | Instr::RegisterAsset { .. }
        | Instr::CreateNewAsset { .. }
        | Instr::TransferAsset { .. }
        | Instr::MintAsset { .. }
        | Instr::BurnAsset { .. }
        | Instr::CreateNft { .. }
        | Instr::TransferNft { .. }
        | Instr::CreateNftsForAllUsers
        | Instr::SetExecutionDepth { .. }
        | Instr::SetVl { .. }
        | Instr::SetAccountDetail { .. }
        | Instr::RegisterDomain { .. }
        | Instr::RegisterAccount { .. }
        | Instr::UnregisterDomain { .. }
        | Instr::UnregisterAsset { .. }
        | Instr::UnregisterAccount { .. }
        | Instr::RegisterPeer { .. }
        | Instr::UnregisterPeer { .. }
        | Instr::CreateTrigger { .. }
        | Instr::RemoveTrigger { .. }
        | Instr::SetTriggerEnabled { .. }
        | Instr::CreateRole { .. }
        | Instr::DeleteRole { .. }
        | Instr::GrantRole { .. }
        | Instr::RevokeRole { .. }
        | Instr::ZkVerify { .. }
        | Instr::VendorExecuteInstruction { .. }
        | Instr::SubscriptionBill
        | Instr::SubscriptionRecordUsage
        | Instr::AssertEq { .. }
        | Instr::Assert { .. }
        | Instr::AbortIf { .. }
        | Instr::Info { .. }
        | Instr::MapSet { .. }
        | Instr::SetNftData { .. }
        | Instr::BurnNft { .. }
        | Instr::TransferDomain { .. }
        | Instr::StateSet { .. }
        | Instr::StateDel { .. }
        | Instr::AxtBegin { .. }
        | Instr::AxtTouch { .. }
        | Instr::VerifyDsProof { .. }
        | Instr::UseAssetHandle { .. }
        | Instr::AxtCommit
        | Instr::TransferBatchBegin
        | Instr::TransferBatchEnd => None,
        Instr::CallMulti { .. } | Instr::MapLoadPair { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{self, BasicBlock, Instr, Terminator};

    #[test]
    fn reuse_registers_when_intervals_do_not_overlap() {
        let mut blocks = Vec::new();
        let mut instrs = Vec::new();
        for i in 0..35 {
            instrs.push(Instr::Const {
                dest: Temp(i),
                value: i as i64,
            });
        }
        blocks.push(BasicBlock {
            label: ir::Label(0),
            instrs,
            terminator: Terminator::Return(None),
        });
        let func = Function {
            name: "f".into(),
            params: vec![],
            blocks,
            entry: ir::Label(0),
        };
        let alloc = allocate(&func);
        assert!(alloc.stack.is_empty());
        assert_eq!(alloc.frame_size, 0);
        for &reg in alloc.regs.values() {
            assert!(ALLOC_POOL.contains(&reg));
        }
    }

    #[test]
    fn spills_when_live_set_exceeds_pool() {
        let live = ALLOC_POOL.len() + 4;
        let mut blocks = Vec::new();
        let mut instrs = Vec::new();
        for i in 0..live {
            instrs.push(Instr::Const {
                dest: Temp(i),
                value: i as i64,
            });
        }
        instrs.push(Instr::TuplePack {
            dest: Temp(live),
            items: (0..live).map(Temp).collect(),
        });
        blocks.push(BasicBlock {
            label: ir::Label(0),
            instrs,
            terminator: Terminator::Return(None),
        });
        let func = Function {
            name: "g".into(),
            params: vec![],
            blocks,
            entry: ir::Label(0),
        };
        let alloc = allocate(&func);
        assert!(
            !alloc.stack.is_empty(),
            "expected spills when live set exceeds pool"
        );
        assert!(alloc.frame_size > 0);
        assert_eq!(alloc.frame_size % 16, 0);
    }

    #[test]
    fn deterministic_allocation_for_equal_start_intervals() {
        let dest0 = Temp(0);
        let dest1 = Temp(1);
        let instrs = vec![Instr::CallMulti {
            callee: "f".into(),
            args: Vec::new(),
            dests: vec![dest0, dest1],
        }];
        let block = BasicBlock {
            label: ir::Label(0),
            instrs,
            terminator: Terminator::Return(None),
        };
        let func = Function {
            name: "f".into(),
            params: vec![],
            blocks: vec![block],
            entry: ir::Label(0),
        };
        let alloc = allocate(&func);
        let expected_first = *ALLOC_POOL.last().expect("alloc pool");
        let expected_second = ALLOC_POOL[ALLOC_POOL.len() - 2];
        assert_eq!(alloc.regs.get(&dest0), Some(&expected_first));
        assert_eq!(alloc.regs.get(&dest1), Some(&expected_second));
    }
}
