// Included from the world query implementation module to preserve its lexical scope.
fn proof_record_alias_values(record: &ProofRecord, field: &str) -> Vec<String> {
    match field {
        "id" => vec![record.id.to_string()],
        "backend" | "id.backend" => vec![record.id.backend.to_string()],
        "status" => vec![proof_status_label(record.status).to_owned()],
        _ => Vec::new(),
    }
}

fn predicate_value_at_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return None;
    }
    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }
        match current {
            Value::Object(map) => current = map.get(segment)?,
            _ => return None,
        }
    }
    Some(current)
}

fn predicate_value_equals_str(value: &Value, expected: &str) -> bool {
    matches!(value, Value::String(raw) if raw == expected)
}

fn predicate_values_contain_str(values: &[Value], expected: &str) -> bool {
    values
        .iter()
        .any(|value| matches!(value, Value::String(raw) if raw == expected))
}

fn proof_record_json_value<'a>(
    cache: &'a mut Option<Value>,
    record: &ProofRecord,
) -> Option<&'a Value> {
    if cache.is_none() {
        *cache = crate::smartcontracts::isi::query::ordinary_predicate_json_value(record);
    }
    cache.as_ref()
}

fn predicate_matches_proof_record(predicate: &PredicateJson, record: &ProofRecord) -> bool {
    let mut record_json = None;
    for cond in &predicate.equals {
        let aliases = proof_record_alias_values(record, &cond.field);
        if !aliases.is_empty() {
            if !aliases
                .iter()
                .any(|alias| predicate_value_equals_str(&cond.value, alias))
            {
                return false;
            }
            continue;
        }
        let Some(value) = proof_record_json_value(&mut record_json, record) else {
            continue;
        };
        let Some(actual) = predicate_value_at_path(value, &cond.field) else {
            return false;
        };
        if actual != &cond.value {
            return false;
        }
    }
    for cond in &predicate.r#in {
        let aliases = proof_record_alias_values(record, &cond.field);
        if !aliases.is_empty() {
            if !aliases
                .iter()
                .any(|alias| predicate_values_contain_str(&cond.values, alias))
            {
                return false;
            }
            continue;
        }
        let Some(value) = proof_record_json_value(&mut record_json, record) else {
            continue;
        };
        let Some(actual) = predicate_value_at_path(value, &cond.field) else {
            return false;
        };
        if !cond.values.iter().any(|candidate| candidate == actual) {
            return false;
        }
    }
    for field in &predicate.exists {
        if !proof_record_alias_values(record, field).is_empty() {
            continue;
        }
        let Some(value) = proof_record_json_value(&mut record_json, record) else {
            continue;
        };
        let Some(actual) = predicate_value_at_path(value, field) else {
            return false;
        };
        if actual.is_null() {
            return false;
        }
    }
    true
}

impl ValidQuery for iroha_data_model::query::proof::prelude::FindProofRecords {
    #[metrics(+"find_proof_records")]
    fn execute(
        self,
        filter: CompoundPredicate<iroha_data_model::proof::ProofRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = iroha_data_model::proof::ProofRecord>, Error> {
        let world = state_ro.world();
        let predicate_json = filter
            .json_payload()
            .and_then(iroha_data_model::query::json::predicate_json_candidate_plan_for_execution);
        if let Some(candidate_ids) = predicate_json
            .as_ref()
            .and_then(|predicate| proof_record_candidate_ids(predicate, world))
        {
            let iter: Box<dyn Iterator<Item = iroha_data_model::proof::ProofRecord> + '_> =
                Box::new(candidate_ids.into_iter().filter_map(move |proof_id| {
                    world
                        .proofs()
                        .get(&proof_id)
                        .filter(|record| {
                            if let Some(predicate) = predicate_json.as_ref() {
                                predicate_matches_proof_record(predicate, record)
                            } else {
                                filter.applies(*record)
                            }
                        })
                        .cloned()
                }));
            return Ok(iter);
        }
        let iter: Box<dyn Iterator<Item = iroha_data_model::proof::ProofRecord> + '_> =
            Box::new(world.proofs().iter().filter_map(move |(_, record)| {
                let matches = if let Some(predicate) = predicate_json.as_ref() {
                    predicate_matches_proof_record(predicate, record)
                } else {
                    filter.applies(record)
                };
                matches.then(|| record.clone())
            }));
        Ok(iter)
    }
}
impl ValidQuery for iroha_data_model::query::proof::prelude::FindProofRecordsByBackend {
    #[metrics(+"find_proof_records_by_backend")]
    #[allow(clippy::needless_collect)]
    fn execute(
        self,
        filter: CompoundPredicate<iroha_data_model::proof::ProofRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = iroha_data_model::proof::ProofRecord>, Error> {
        let backend = self.backend.to_string();
        let requested_backend = backend.clone();
        // Own the backend-range results because the returned iterator cannot borrow the
        // query's owned backend string after this function returns.
        let proofs = state_ro
            .world()
            .proofs_by_backend_iter(&backend)
            .map(|(_, rec)| rec)
            .filter(move |rec| {
                rec.id.backend.as_str() == requested_backend.as_str() && filter.applies(rec)
            })
            .cloned()
            .collect::<Vec<_>>();
        Ok(proofs.into_iter())
    }
}
impl ValidQuery for iroha_data_model::query::proof::prelude::FindProofRecordsByStatus {
    #[metrics(+"find_proof_records_by_status")]
    #[allow(clippy::needless_collect)]
    fn execute(
        self,
        filter: CompoundPredicate<iroha_data_model::proof::ProofRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = iroha_data_model::proof::ProofRecord>, Error> {
        let status = self.status;
        let requested_status = status;
        // Own the status-index results because the returned iterator cannot borrow the
        // query's owned status value after this function returns.
        let proofs = state_ro
            .world()
            .proofs_by_status_iter(&status)
            .map(|(_, rec)| rec)
            .filter(move |rec| rec.status == requested_status && filter.applies(rec))
            .cloned()
            .collect::<Vec<_>>();
        Ok(proofs.into_iter())
    }
}
