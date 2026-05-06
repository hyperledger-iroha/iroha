//! This module contains trait implementations related to block queries
use std::{collections::BTreeSet, num::NonZeroUsize};

use eyre::Result;
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{BlockHeader, SignedBlock},
    query::{
        dsl::{CompoundPredicate, EvaluatePredicate},
        error::QueryExecutionFail,
        json::PredicateJson,
    },
};
use iroha_telemetry::metrics;
use nonzero_ext::nonzero;
use norito::json::Value;

use super::*;
use crate::{smartcontracts::ValidQuery, state::StateReadOnly};

fn block_height_from_value(value: &Value) -> Option<NonZeroUsize> {
    let height = usize::try_from(value.as_u64()?).ok()?;
    NonZeroUsize::new(height)
}

fn block_hash_from_value(value: &Value) -> Option<HashOf<BlockHeader>> {
    norito::json::from_value(value.clone()).ok()
}

fn intersect_block_candidate_heights(
    best: &mut Option<BTreeSet<NonZeroUsize>>,
    candidates: BTreeSet<NonZeroUsize>,
) {
    let Some(current) = best.take() else {
        *best = Some(candidates);
        return;
    };
    *best = Some(current.intersection(&candidates).copied().collect());
}

fn block_candidate_heights(
    predicate: &PredicateJson,
    state_ro: &impl StateReadOnly,
    is_height_field: impl Fn(&str) -> bool,
    is_hash_field: impl Fn(&str) -> bool,
) -> Option<BTreeSet<NonZeroUsize>> {
    let mut best = None;

    for cond in &predicate.equals {
        if is_height_field(&cond.field) {
            intersect_block_candidate_heights(
                &mut best,
                block_height_from_value(&cond.value).into_iter().collect(),
            );
            continue;
        }
        if is_hash_field(&cond.field) {
            intersect_block_candidate_heights(
                &mut best,
                block_hash_from_value(&cond.value)
                    .and_then(|hash| state_ro.kura().get_block_height_by_hash(hash))
                    .into_iter()
                    .collect(),
            );
        }
    }

    for cond in &predicate.r#in {
        if is_height_field(&cond.field) {
            intersect_block_candidate_heights(
                &mut best,
                cond.values
                    .iter()
                    .filter_map(block_height_from_value)
                    .collect(),
            );
            continue;
        }
        if is_hash_field(&cond.field) {
            intersect_block_candidate_heights(
                &mut best,
                cond.values
                    .iter()
                    .filter_map(block_hash_from_value)
                    .filter_map(|hash| state_ro.kura().get_block_height_by_hash(hash))
                    .collect(),
            );
        }
    }

    best
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

fn predicate_value_equals_hash(value: &Value, expected: HashOf<BlockHeader>) -> bool {
    value
        .as_str()
        .is_some_and(|raw| raw == expected.to_string())
        || block_hash_from_value(value).is_some_and(|hash| hash == expected)
}

fn predicate_values_contain_hash(values: &[Value], expected: HashOf<BlockHeader>) -> bool {
    values
        .iter()
        .any(|value| predicate_value_equals_hash(value, expected))
}

fn block_header_alias_equals(header: &BlockHeader, field: &str, value: &Value) -> Option<bool> {
    match field {
        "height" => Some(value.as_u64() == Some(header.height().get())),
        "hash" | "block_hash" => Some(predicate_value_equals_hash(value, header.hash())),
        _ => None,
    }
}

fn signed_block_alias_equals(block: &SignedBlock, field: &str, value: &Value) -> Option<bool> {
    match field {
        "height" | "header.height" | "payload.header.height" => {
            Some(value.as_u64() == Some(block.header().height().get()))
        }
        "hash" | "block_hash" | "header.hash" => {
            Some(predicate_value_equals_hash(value, block.hash()))
        }
        _ => None,
    }
}

fn block_header_alias_in(header: &BlockHeader, field: &str, values: &[Value]) -> Option<bool> {
    match field {
        "height" => Some(
            values
                .iter()
                .any(|value| value.as_u64() == Some(header.height().get())),
        ),
        "hash" | "block_hash" => Some(predicate_values_contain_hash(values, header.hash())),
        _ => None,
    }
}

fn signed_block_alias_in(block: &SignedBlock, field: &str, values: &[Value]) -> Option<bool> {
    match field {
        "height" | "header.height" | "payload.header.height" => Some(
            values
                .iter()
                .any(|value| value.as_u64() == Some(block.header().height().get())),
        ),
        "hash" | "block_hash" | "header.hash" => {
            Some(predicate_values_contain_hash(values, block.hash()))
        }
        _ => None,
    }
}

fn block_header_alias_exists(field: &str) -> bool {
    matches!(field, "height" | "hash" | "block_hash")
}

fn signed_block_alias_exists(field: &str) -> bool {
    matches!(
        field,
        "height"
            | "header.height"
            | "payload.header.height"
            | "hash"
            | "block_hash"
            | "header.hash"
    )
}

fn block_header_json_value<'a>(
    cache: &'a mut Option<Value>,
    header: &BlockHeader,
) -> Option<&'a Value> {
    if cache.is_none() {
        *cache = norito::json::to_value(header).ok();
    }
    cache.as_ref()
}

fn signed_block_json_value<'a>(
    cache: &'a mut Option<Value>,
    block: &SignedBlock,
) -> Option<&'a Value> {
    if cache.is_none() {
        *cache = norito::json::to_value(block).ok();
    }
    cache.as_ref()
}

fn predicate_matches_block_header(predicate: &PredicateJson, header: &BlockHeader) -> bool {
    let mut header_json = None;

    for cond in &predicate.equals {
        if let Some(matches) = block_header_alias_equals(header, &cond.field, &cond.value) {
            if !matches {
                return false;
            }
            continue;
        }
        let Some(value) = block_header_json_value(&mut header_json, header) else {
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
        if let Some(matches) = block_header_alias_in(header, &cond.field, &cond.values) {
            if !matches {
                return false;
            }
            continue;
        }
        let Some(value) = block_header_json_value(&mut header_json, header) else {
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
        if block_header_alias_exists(field) {
            continue;
        }
        let Some(value) = block_header_json_value(&mut header_json, header) else {
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

fn predicate_matches_signed_block(predicate: &PredicateJson, block: &SignedBlock) -> bool {
    let mut block_json = None;

    for cond in &predicate.equals {
        if let Some(matches) = signed_block_alias_equals(block, &cond.field, &cond.value) {
            if !matches {
                return false;
            }
            continue;
        }
        let Some(value) = signed_block_json_value(&mut block_json, block) else {
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
        if let Some(matches) = signed_block_alias_in(block, &cond.field, &cond.values) {
            if !matches {
                return false;
            }
            continue;
        }
        let Some(value) = signed_block_json_value(&mut block_json, block) else {
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
        if signed_block_alias_exists(field) {
            continue;
        }
        let Some(value) = signed_block_json_value(&mut block_json, block) else {
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

impl ValidQuery for FindBlocks {
    #[metrics(+"find_blocks")]
    fn execute(
        self,
        filter: CompoundPredicate<SignedBlock>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = Self::Item>, QueryExecutionFail> {
        let predicate_json = filter
            .json_payload()
            .and_then(|raw| norito::json::from_str::<PredicateJson>(raw).ok());
        if let Some(candidate_heights) = predicate_json.as_ref().and_then(|predicate| {
            block_candidate_heights(
                predicate,
                state_ro,
                |field| matches!(field, "height" | "header.height" | "payload.header.height"),
                |field| matches!(field, "hash" | "block_hash" | "header.hash"),
            )
        }) {
            let iter: Box<dyn Iterator<Item = SignedBlock> + '_> = Box::new(
                candidate_heights
                    .into_iter()
                    .rev()
                    .filter_map(move |height| {
                        state_ro
                            .kura()
                            .get_block(height)
                            .filter(|block| {
                                predicate_json.as_ref().map_or_else(
                                    || filter.applies(block.as_ref()),
                                    |predicate| predicate_matches_signed_block(predicate, block),
                                )
                            })
                            .map(|block| block.as_ref().clone())
                    }),
            );
            return Ok(iter);
        }

        let iter: Box<dyn Iterator<Item = SignedBlock> + '_> = Box::new(
            state_ro
                .all_blocks(nonzero!(1_usize))
                .rev()
                .filter(move |block| {
                    predicate_json.as_ref().map_or_else(
                        || filter.applies(block),
                        |predicate| predicate_matches_signed_block(predicate, block),
                    )
                })
                .map(|block| (*block).clone()),
        );
        Ok(iter)
    }
}

impl ValidQuery for FindBlockHeaders {
    #[metrics(+"find_block_headers")]
    fn execute(
        self,
        filter: CompoundPredicate<BlockHeader>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = Self::Item>, QueryExecutionFail> {
        let predicate_json = filter
            .json_payload()
            .and_then(|raw| norito::json::from_str::<PredicateJson>(raw).ok());
        if let Some(candidate_heights) = predicate_json.as_ref().and_then(|predicate| {
            block_candidate_heights(
                predicate,
                state_ro,
                |field| field == "height",
                |field| matches!(field, "hash" | "block_hash"),
            )
        }) {
            let iter: Box<dyn Iterator<Item = BlockHeader> + '_> = Box::new(
                candidate_heights
                    .into_iter()
                    .rev()
                    .filter_map(move |height| {
                        let header = state_ro.kura().get_block(height)?.header();
                        let matches = predicate_json.as_ref().map_or_else(
                            || filter.applies(&header),
                            |predicate| predicate_matches_block_header(predicate, &header),
                        );
                        matches.then_some(header)
                    }),
            );
            return Ok(iter);
        }

        let iter: Box<dyn Iterator<Item = BlockHeader> + '_> = Box::new(
            state_ro
                .all_blocks(nonzero!(1_usize))
                .rev()
                .filter_map(move |block| {
                    let header = block.header();
                    let matches = predicate_json.as_ref().map_or_else(
                        || filter.applies(&header),
                        |predicate| predicate_matches_block_header(predicate, &header),
                    );
                    matches.then_some(header)
                }),
        );
        Ok(iter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_candidate_heights_are_intersected() {
        let mut candidates = None;
        intersect_block_candidate_heights(
            &mut candidates,
            BTreeSet::from([nonzero!(1_usize), nonzero!(2_usize)]),
        );
        intersect_block_candidate_heights(
            &mut candidates,
            BTreeSet::from([nonzero!(2_usize), nonzero!(3_usize)]),
        );

        assert_eq!(candidates, Some(BTreeSet::from([nonzero!(2_usize)])));
    }
}
