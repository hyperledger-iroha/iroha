#[test]
fn staged_mint_helper_keeps_state_map_base_literals_after_call_propagation() {
    let src = r#"
seiyaku StagedMintRequest {
  state int MintRequestNextSequence;
  state StateMap<Name, int> MintRequestSequenceById;
  state StateMap<int, int> MintRequestSequences;
  state StateMap<int, Name> MintRequestRequestIds;
  state StateMap<int, Name> MintRequestFiIds;
  state StateMap<int, AccountId> MintRequestFiAuthorities;
  state StateMap<int, AccountId> MintRequestToAccounts;
  state StateMap<int, int> MintRequestAmounts;
  state StateMap<int, Json> MintRequestRequestedBy;
  state StateMap<int, int> MintRequestStates;
  state StateMap<int, int> MintRequestCreatedAt;
  state StateMap<int, int> MintRequestExpiresAt;
  state StateMap<int, int> MintRequestFinalizedAt;
  state StateMap<int, int> MintRequestCanceledAt;

  hajimari() { MintRequestNextSequence = 0; }

  fn update_record(int sequence,
                   Name request_id,
                   Name fi_id,
                   AccountId fi_multisig_account_id,
                   AccountId to_account_id,
                   int amount_i64,
                   Json requested_by_actor_id,
                   int state_code,
                   int created_at_ms,
                   int expires_at_ms,
                   int finalized_at_ms,
                   int canceled_at_ms) {
    MintRequestSequences[sequence] = sequence;
    MintRequestRequestIds[sequence] = request_id;
    MintRequestFiIds[sequence] = fi_id;
    MintRequestFiAuthorities[sequence] = fi_multisig_account_id;
    MintRequestToAccounts[sequence] = to_account_id;
    MintRequestAmounts[sequence] = amount_i64;
    MintRequestRequestedBy[sequence] = requested_by_actor_id;
    MintRequestStates[sequence] = state_code;
    MintRequestCreatedAt[sequence] = created_at_ms;
    MintRequestExpiresAt[sequence] = expires_at_ms;
    MintRequestFinalizedAt[sequence] = finalized_at_ms;
    MintRequestCanceledAt[sequence] = canceled_at_ms;
  }

  fn run() {
    let ev = context::trigger_event();
    let action_key = Name::parse("action");
    let request_id_key = Name::parse("request_id");
    let fi_id_key = Name::parse("fi_id");
    let to_account_id_key = Name::parse("to_account_id");
    let amount_i64_key = Name::parse("amount_i64");
    let requested_by_actor_id_key = Name::parse("requested_by_actor_id");
    let created_at_ms_key = Name::parse("created_at_ms");
    let expires_at_ms_key = Name::parse("expires_at_ms");

    let action = ev.get_name(action_key).unwrap_or(Name::parse("missing"));
    if (action == Name::parse("create")) {
      let request_id = ev.get_name(request_id_key).unwrap_or(Name::parse("missing"));
      let sequence = MintRequestNextSequence + 1;
      let fi_id = ev.get_name(fi_id_key).unwrap_or(Name::parse("missing"));
      let to_account_id = ev.get_account_id(to_account_id_key).unwrap_or(context::authority());
      let amount_i64 = ev.get_int(amount_i64_key).unwrap_or(0);
      let requested_by_actor_id = ev.get_json(requested_by_actor_id_key).unwrap_or(Json::parse("{}"));
      let created_at_ms = ev.get_int(created_at_ms_key).unwrap_or(0);
      let expires_at_ms = ev.get_int(expires_at_ms_key).unwrap_or(0);
      update_record(
        sequence: sequence,
        request_id: request_id,
        fi_id: fi_id,
        fi_multisig_account_id: to_account_id,
        to_account_id: to_account_id,
        amount_i64: amount_i64,
        requested_by_actor_id: requested_by_actor_id,
        state_code: 0,
        created_at_ms: created_at_ms,
        expires_at_ms: expires_at_ms,
        finalized_at_ms: 0,
        canceled_at_ms: 0,
      );
    }
  }
}
"#;

    let program = parse(src).expect("parse");
    let typed = analyze(&program).expect("analyze");
    let ir_prog = ir::lower_with_cap(&typed, usize::from(COLLECTION_ITERATION_CAP)).expect("lower");
    let typed_functions: Vec<_> = typed
        .items
        .iter()
        .map(|item| match item {
            crate::semantic::TypedItem::Function(func) => func,
        })
        .collect();

    let mut string_map: HashMap<(usize, ir::Temp), String> = HashMap::new();
    let mut string_literal_temps: HashSet<(usize, ir::Temp)> = HashSet::new();
    let mut dataref_kind_map: HashMap<(usize, ir::Temp), ir::DataRefKind> = HashMap::new();
    let mut int_const_map: HashMap<(usize, ir::Temp), i64> = HashMap::new();
    let mut param_temp_map: HashMap<(usize, usize), ir::Temp> = HashMap::new();
    let multiply_defined_dests = super::multiply_defined_temps(&ir_prog);

    use crate::ast::UnaryOp;
    use crate::ir::DataRefKind as DRK;
    for (func_idx, func) in ir_prog.functions.iter().enumerate() {
        for bb in &func.blocks {
            for instr in &bb.instrs {
                if let ir::Instr::Binary { dest, .. } = instr {
                    int_const_map.remove(&(func_idx, *dest));
                }
                if let ir::Instr::Copy { dest, src } = instr {
                    if dest != src {
                        let dest_key = (func_idx, *dest);
                        string_map.remove(&dest_key);
                        dataref_kind_map.remove(&dest_key);
                        int_const_map.remove(&dest_key);
                        string_literal_temps.remove(&dest_key);
                        if !multiply_defined_dests.contains(&dest_key) {
                            if let Some(val) = string_map.get(&(func_idx, *src)).cloned() {
                                string_map.insert(dest_key, val);
                            }
                            if let Some(kind) = dataref_kind_map.get(&(func_idx, *src)).copied() {
                                dataref_kind_map.insert(dest_key, kind);
                            }
                            if let Some(val) = int_const_map.get(&(func_idx, *src)).copied() {
                                int_const_map.insert(dest_key, val);
                            }
                            if string_literal_temps.contains(&(func_idx, *src)) {
                                string_literal_temps.insert(dest_key);
                            }
                        }
                    }
                    continue;
                }
                if let ir::Instr::StringConst { dest, value } = instr {
                    string_map.insert((func_idx, *dest), value.clone());
                    string_literal_temps.insert((func_idx, *dest));
                    dataref_kind_map.insert((func_idx, *dest), DRK::Blob);
                }
                if let ir::Instr::PointerFromString { dest, kind, src } = instr
                    && let Some(s) = string_map.get(&(func_idx, *src)).cloned()
                {
                    string_map.insert((func_idx, *dest), s);
                    dataref_kind_map.insert((func_idx, *dest), *kind);
                }
                if let ir::Instr::Const { dest, value } = instr {
                    int_const_map.insert((func_idx, *dest), *value);
                }
                if let ir::Instr::Unary {
                    dest,
                    op: UnaryOp::Neg,
                    operand,
                } = instr
                    && let Some(value) = int_const_map.get(&(func_idx, *operand)).copied()
                    && let Some(neg) = value.checked_neg()
                {
                    int_const_map.insert((func_idx, *dest), neg);
                }
                if let ir::Instr::DataRef { dest, kind, value } = instr {
                    string_map.insert((func_idx, *dest), value.clone());
                    dataref_kind_map.insert((func_idx, *dest), *kind);
                }
                if let ir::Instr::PointerFromNorito { dest, kind, .. } = instr {
                    dataref_kind_map.insert((func_idx, *dest), *kind);
                }
                if let ir::Instr::StateGet { dest, .. }
                | ir::Instr::StateKeys { dest, .. }
                | ir::Instr::StateMapKeyAt { dest, .. } = instr
                {
                    dataref_kind_map.insert((func_idx, *dest), DRK::NoritoBytes);
                }
                if let ir::Instr::PointerToNorito { dest, value } = instr {
                    dataref_kind_map.insert((func_idx, *dest), DRK::NoritoBytes);
                    let literal_kind = dataref_kind_map.get(&(func_idx, *value)).copied();
                    let literal_raw = string_map.get(&(func_idx, *value)).cloned();
                    if let (Some(kind), Some(raw)) = (literal_kind, literal_raw)
                        && let Some(tlv_bytes) = super::encode_pointer_tlv_bytes(kind, &raw)
                    {
                        let hex = hex::encode(tlv_bytes);
                        string_map.insert((func_idx, *dest), format!("0x{hex}"));
                    }
                }
                if let ir::Instr::ActorAccount { dest, .. } = instr {
                    dataref_kind_map.insert((func_idx, *dest), DRK::Account);
                }
                if let ir::Instr::ActorPublicKey { dest, .. } | ir::Instr::ActorSign { dest, .. } =
                    instr
                {
                    dataref_kind_map.insert((func_idx, *dest), DRK::Blob);
                }
                if let ir::Instr::LoadVar { dest, name } = instr
                    && let Some(param_idx) = func.params.iter().position(|p| p == name)
                {
                    param_temp_map.entry((func_idx, param_idx)).or_insert(*dest);
                }
                crate::regalloc::visit_instr_defs(instr, |dest| {
                    let key = (func_idx, dest);
                    if multiply_defined_dests.contains(&key) {
                        string_map.remove(&key);
                        dataref_kind_map.remove(&key);
                        int_const_map.remove(&key);
                        string_literal_temps.remove(&key);
                    }
                });
            }
        }
    }

    let fn_index_by_name: HashMap<String, usize> = typed_functions
        .iter()
        .enumerate()
        .map(|(idx, func)| (func.name.clone(), idx))
        .collect();
    let mut literal_param_conflicts: HashSet<(usize, ir::Temp)> = HashSet::new();
    for (caller_idx, func) in ir_prog.functions.iter().enumerate() {
        for bb in &func.blocks {
            for instr in &bb.instrs {
                if let Some((name, args)) = match instr {
                    ir::Instr::Call { callee, args, .. }
                    | ir::Instr::CallMulti { callee, args, .. } => {
                        Some((callee.as_str(), args.as_slice()))
                    }
                    _ => None,
                } && let Some(&callee_idx) = fn_index_by_name.get(name)
                {
                    let callee = &ir_prog.functions[callee_idx];
                    let count = usize::min(args.len(), callee.params.len());
                    for (i, &arg_temp) in args.iter().take(count).enumerate() {
                        let Some(&param_temp) = param_temp_map.get(&(callee_idx, i)) else {
                            continue;
                        };
                        let param_key = (callee_idx, param_temp);
                        if literal_param_conflicts.contains(&param_key) {
                            continue;
                        }
                        let arg_has_literal = string_literal_temps
                            .contains(&(caller_idx, arg_temp))
                            || dataref_kind_map.contains_key(&(caller_idx, arg_temp));
                        let Some(value) = string_map.get(&(caller_idx, arg_temp)).cloned() else {
                            if string_map.contains_key(&param_key) {
                                string_map.remove(&param_key);
                                string_literal_temps.remove(&param_key);
                                dataref_kind_map.remove(&param_key);
                                literal_param_conflicts.insert(param_key);
                            }
                            continue;
                        };
                        if !arg_has_literal {
                            if string_map.contains_key(&param_key) {
                                string_map.remove(&param_key);
                                string_literal_temps.remove(&param_key);
                                dataref_kind_map.remove(&param_key);
                                literal_param_conflicts.insert(param_key);
                            }
                            continue;
                        }
                        if let Some(existing) = string_map.get(&param_key) {
                            if existing != &value {
                                string_map.remove(&param_key);
                                string_literal_temps.remove(&param_key);
                                dataref_kind_map.remove(&param_key);
                                literal_param_conflicts.insert(param_key);
                                continue;
                            }
                        } else {
                            string_map.insert(param_key, value);
                        }
                        if string_literal_temps.contains(&(caller_idx, arg_temp)) {
                            string_literal_temps.insert(param_key);
                        }
                        if let Some(kind) = dataref_kind_map.get(&(caller_idx, arg_temp)).copied() {
                            dataref_kind_map.insert(param_key, kind);
                        }
                    }
                }
            }
        }
    }

    let update_record_idx = ir_prog
        .functions
        .iter()
        .position(|func| func.name == "update_record")
        .expect("update_record index");
    let update_record = &ir_prog.functions[update_record_idx];
    let mut bases = Vec::new();
    for bb in &update_record.blocks {
        for instr in &bb.instrs {
            if let ir::Instr::PathMapKeyNorito { base, .. } = instr {
                bases.push(
                    string_map
                        .get(&(update_record_idx, *base))
                        .cloned()
                        .expect("PathMapKey base should be a literal name"),
                );
            }
        }
    }

    assert_eq!(
        bases,
        vec![
            "MintRequestSequences",
            "MintRequestRequestIds",
            "MintRequestFiIds",
            "MintRequestFiAuthorities",
            "MintRequestToAccounts",
            "MintRequestAmounts",
            "MintRequestRequestedBy",
            "MintRequestStates",
            "MintRequestCreatedAt",
            "MintRequestExpiresAt",
            "MintRequestFinalizedAt",
            "MintRequestCanceledAt",
        ]
    );
}
