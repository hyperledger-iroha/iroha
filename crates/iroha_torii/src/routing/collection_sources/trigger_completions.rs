//! Trigger completions persisted in committed execution outputs, as a history
//! collection (`specs/torii/collection_queries.md`).
//!
//! One row per completion record of a Network, Pipeline or Time execution output,
//! newest first by `(block_height, output_index, completion_index)`. Blocks are read
//! through the checkpoint-authenticated carrier reader under the request's query owner
//! and the page's history work and byte budget. A page that fills up inside a block
//! continues after its last examined completion; a page whose budget ends between
//! blocks continues strictly below the last block it examined completely, so a sparse
//! stretch returns short or empty pages with advancing cursors instead of failing.
use super::*;
use iroha_data_model::{
    block::execution_output::InvocationCompletionV1,
    events::trigger_completed::TriggerCompletedOutcome,
};

/// Collection endpoint label of `GET /v1/triggers/completed`.
pub(super) const ENDPOINT: &str = "/v1/triggers/completed";

/// How a page's walk ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PageEnd {
    /// Every block of the requested range was examined.
    Exhausted,
    /// The page holds `limit` rows and another matching row exists.
    Full,
    /// The history budget, or the query owner once a block was complete, refused more work.
    BudgetSpent,
}

/// Where the next page starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Continuation {
    /// Strictly after this examined completion.
    After((u64, u64, u64)),
    /// Strictly below this completely examined block.
    Below(u64),
}

/// The continuation of a page that stopped early: after the last examined completion
/// when it lies below every complete block, otherwise below the last complete block.
/// `None` means the page made no progress and cannot name a continuation.
fn continuation(
    completed_block: Option<u64>,
    last_examined: Option<(u64, u64, u64)>,
) -> Option<Continuation> {
    match (completed_block, last_examined) {
        (Some(block), Some(row)) if row.0 < block => Some(Continuation::After(row)),
        (Some(block), _) => Some(Continuation::Below(block)),
        (None, Some(row)) => Some(Continuation::After(row)),
        (None, None) => None,
    }
}

/// The row of one completion record.
fn completion_row(
    (block_height, output_index, completion_index): (u64, u64, u64),
    entrypoint_index: Option<u64>,
    trigger_execution_hash: &str,
    completion: &InvocationCompletionV1,
) -> Map {
    let (outcome, message) = match &completion.outcome {
        TriggerCompletedOutcome::Success => ("Success", Value::Null),
        TriggerCompletedOutcome::Failure(message) => ("Failure", Value::from(message.clone())),
    };
    let mut row = Map::new();
    row.insert("block_height".into(), Value::from(block_height));
    row.insert("output_index".into(), Value::from(output_index));
    row.insert("completion_index".into(), Value::from(completion_index));
    row.insert(
        "trigger_id".into(),
        Value::from(completion.trigger_id.to_string()),
    );
    row.insert(
        "trigger_execution_hash".into(),
        Value::from(trigger_execution_hash),
    );
    row.insert(
        "entrypoint_index".into(),
        entrypoint_index.map_or(Value::Null, Value::from),
    );
    row.insert(
        "step_index".into(),
        Value::from(u64::from(completion.callback_index)),
    );
    row.insert("outcome".into(), Value::from(outcome));
    row.insert("message".into(), message);
    row
}

/// Visit the completion rows of one authenticated block in descending
/// `(output_index, completion_index)` order, skipping coordinates at or above `after`.
/// Stops when `visit` breaks and reports whether the block was examined completely.
///
/// # Errors
/// Rejects an output whose execution call cannot be resolved from the block.
pub(crate) fn visit_block_rows(
    block: &iroha_data_model::block::SignedBlock,
    block_height: u64,
    after: Option<(u64, u64, u64)>,
    mut visit: impl FnMut((u64, u64, u64), Map) -> std::ops::ControlFlow<()>,
) -> Result<bool> {
    for (output_index, output) in block.execution_outputs().iter().enumerate().rev() {
        let output_index = output_index as u64;
        if after.is_some_and(|(height, index, _)| (block_height, output_index) > (height, index)) {
            continue;
        }
        // A governance block-start row runs no trigger: it has neither an execution
        // call nor a completion.
        let completions = output.completions();
        if completions.is_empty() {
            continue;
        }
        let call_hash = output
            .execution_call_hash(block.hash(), block)
            .map_err(conversion_error)?
            .to_string();
        let entrypoint_index = output.network_index().map(u64::from);
        for (completion_index, completion) in completions.iter().enumerate().rev() {
            let coordinate = (block_height, output_index, completion_index as u64);
            if after.is_some_and(|after| coordinate >= after) {
                continue;
            }
            let row = completion_row(coordinate, entrypoint_index, &call_hash, completion);
            if visit(coordinate, row).is_break() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// Read the carrier at `height`, or `None` when the page must end before it: its
/// history work or byte budget is spent or, once the page has completely examined a
/// block (`page_can_end`), its query owner refused the read. The owner refusal still
/// marks the owner, so the successful partial page keeps the original capacity signal.
/// A refusal of the page's first block, and every permanent refusal, is the request's
/// own failure.
fn read_block(
    state: &CoreState,
    height: u64,
    budget: &mut HistoryReadBudget,
    page_can_end: bool,
) -> Result<Option<SharedSignedBlock>> {
    use iroha_core::execution_attempt::ExecutionAttemptError;
    use iroha_data_model::query::error::QueryExecutionFail;
    let height = usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(history_capacity_error)?;
    let owner = crate::history_producer::HistoryProducerOwner::current()?;
    let read_budget = owner.canonical_history_budget();
    // Genesis is authenticated by its own certified successor; every later block by
    // the State/checkpoint ancestry, exactly as other HTTP history observations.
    let carrier = if height == NonZeroUsize::MIN {
        state.read_finalized_execution_carrier_with_read_budget(
            height,
            budget.work_left,
            budget.bytes_left,
            &read_budget,
        )
    } else {
        state.read_executed_carrier_from_checkpoints(
            height,
            budget.work_left,
            budget.bytes_left,
            &read_budget,
        )
    };
    match carrier {
        Ok(carrier) => {
            budget.work_left = budget
                .work_left
                .checked_sub(carrier.work_items())
                .ok_or_else(history_capacity_error)?;
            budget.bytes_left = budget
                .bytes_left
                .checked_sub(carrier.wire_bytes())
                .ok_or_else(history_capacity_error)?;
            Ok(Some(carrier.into_block()))
        }
        // The page's own work or byte allowance: a continuation, not an owner refusal.
        Err(ExecutionAttemptError::Rejected(QueryExecutionFail::GasBudgetExceeded)) => Ok(None),
        Err(ExecutionAttemptError::Deferred(deferred))
            if page_can_end && crate::canonical_history::permanent_refusal(&deferred).is_none() =>
        {
            drop(crate::canonical_history::query_attempt_error(
                ExecutionAttemptError::Deferred(deferred),
            ));
            Ok(None)
        }
        Err(error) => Err(crate::canonical_history::query_attempt_error(error)),
    }
}

/// One page of trigger completions, newest first, strictly after the cursor.
///
/// Completions are node-operator evidence: they are served only to a reader of all
/// ledger data, and any narrower visibility reads an empty page.
///
/// # Errors
/// Returns `query_scan_limit_exceeded` when the page's budget cannot read even its
/// first block, the original capacity refusal of that block, and history that changed
/// underneath the walk.
pub(crate) fn page(
    state: &Arc<CoreState>,
    prepared: &collections::Prepared<'_>,
    visibility: &DataspaceReadVisibility,
) -> Result<RowPage> {
    let empty = || prepared.subrow_page(Vec::new(), None).map_err(Error::from);
    if !visibility.can_read_all() {
        return empty();
    }
    let after = prepared.resume_subrow_position();
    let (lowest, highest) = prepared.height_range();
    let tip = u64::try_from(state.committed_height()).map_err(|_| history_capacity_error())?;
    let Some(anchor) = state.committed_block_hash_at_height(tip) else {
        if tip == 0 {
            return empty();
        }
        return Err(conversion_error(
            "trigger completion history is missing its captured tip".into(),
        ));
    };
    let first = highest
        .min(tip)
        .min(after.map_or(u64::MAX, |position| position.0))
        .min(
            prepared
                .resume_before_block()
                .map_or(u64::MAX, |height| height.saturating_sub(1)),
        );
    let lowest = lowest.max(1);
    if first < lowest {
        return empty();
    }
    let work = app_query_limits().max_fetch_size;
    let mut budget = HistoryReadBudget::new();
    let mut examined = 0_u64;
    let mut items = Vec::new();
    let mut last_examined = None;
    let mut completed_block = None;
    let mut end = PageEnd::Exhausted;
    for height in (lowest..=first).rev() {
        let Some(block) = read_block(state, height, &mut budget, completed_block.is_some())? else {
            end = PageEnd::BudgetSpent;
            break;
        };
        let complete = visit_block_rows(&block, height, after, |coordinate, row| {
            // Every examined completion spends scan work, including rows the filter rejects.
            if examined == work {
                end = PageEnd::BudgetSpent;
                return std::ops::ControlFlow::Break(());
            }
            examined += 1;
            if prepared.matches(&row) {
                if items.len() == prepared.limit() {
                    end = PageEnd::Full;
                    return std::ops::ControlFlow::Break(());
                }
                items.push(row);
            }
            last_examined = Some(coordinate);
            std::ops::ControlFlow::Continue(())
        })?;
        if !complete {
            break;
        }
        completed_block = Some(height);
    }
    require_history_anchor(state, tip, anchor)?;
    if end == PageEnd::Exhausted {
        return prepared.subrow_page(items, None).map_err(Into::into);
    }
    match continuation(completed_block, last_examined) {
        Some(Continuation::After(row)) => prepared.subrow_page(items, Some(row)),
        Some(Continuation::Below(height)) => prepared.block_boundary_page(items, height),
        None => Err(CollectionError::new(
            "query_scan_limit_exceeded",
            if after.is_some() || prepared.resume_before_block().is_some() {
                "cursor"
            } else {
                "filter"
            },
            format!(
                "reading this page's first block needs more history work than one page's \
                 scan budget ({work} blocks and completions)"
            ),
        )
        .with_hint(
            "the per-page budget is the node's `torii.app_api_max_fetch_size`; \
             history blocks larger than it cannot be paged",
        )),
    }
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_pages_continue_at_the_position_furthest_along_the_walk() {
        // A page stopped inside block 7 after examining one of its rows.
        assert_eq!(
            continuation(Some(8), Some((7, 2, 1))),
            Some(Continuation::After((7, 2, 1)))
        );
        // A page stopped before block 7: everything down to block 8 is complete.
        assert_eq!(
            continuation(Some(8), Some((9, 0, 0))),
            Some(Continuation::Below(8))
        );
        assert_eq!(
            continuation(Some(8), Some((8, 0, 0))),
            Some(Continuation::Below(8))
        );
        // A sparse stretch without any completion still advances.
        assert_eq!(continuation(Some(8), None), Some(Continuation::Below(8)));
        // The first block was only partly examined.
        assert_eq!(
            continuation(None, Some((9, 1, 0))),
            Some(Continuation::After((9, 1, 0)))
        );
        // Nothing examined and nothing complete: no safe continuation exists.
        assert_eq!(continuation(None, None), None);
    }

    #[test]
    fn openapi_documents_the_collection_contract_for_the_route() {
        let document = crate::openapi::compiled_spec();
        let operation = &document["paths"][ENDPOINT]["get"];
        let response_schema = |status: &str| {
            operation["responses"][status]["content"]["application/json"]["schema"]["$ref"].as_str()
        };
        assert_eq!(
            response_schema("200"),
            Some("#/components/schemas/TriggerCompletionPage")
        );
        assert_eq!(
            response_schema("400"),
            Some("#/components/schemas/ErrorEnvelope")
        );
        // Exactly the shared history controls: the bespoke parameters are retired.
        let parameters: Vec<&str> = operation["parameters"]
            .as_array()
            .expect("collection parameters")
            .iter()
            .map(|parameter| {
                assert_eq!(parameter["in"].as_str(), Some("query"));
                assert_eq!(parameter["required"].as_bool(), Some(false));
                parameter["name"].as_str().expect("parameter name")
            })
            .collect();
        assert_eq!(parameters, ["filter", "select", "limit", "cursor"]);
        let schemas = &document["components"]["schemas"];
        let page = &schemas["TriggerCompletionPage"];
        assert_eq!(page["additionalProperties"].as_bool(), Some(false));
        assert_eq!(
            page["properties"]["items"]["items"]["$ref"].as_str(),
            Some("#/components/schemas/TriggerCompletionRow")
        );
        assert!(
            page["properties"]
                .as_object()
                .is_some_and(|properties| !properties.contains_key("total")),
            "history collections report no total"
        );
        // The documented row fields are the collection spec's declared fields.
        let row = &schemas["TriggerCompletionRow"];
        assert_eq!(row["additionalProperties"].as_bool(), Some(false));
        assert_eq!(
            row["properties"]
                .as_object()
                .expect("row properties")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            specs::TRIGGER_COMPLETIONS
                .fields
                .iter()
                .map(|field| field.name)
                .collect::<BTreeSet<_>>()
        );
        let outcomes: Vec<&str> = row["properties"]["outcome"]["enum"]
            .as_array()
            .expect("outcome values")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(outcomes, ["Success", "Failure"]);
        let schemas = schemas.as_object().expect("component schemas");
        for retired in [
            "TriggerCompletionListResponse",
            "TriggerCompletionRecord",
            "TriggerCompletionSummary",
        ] {
            assert!(!schemas.contains_key(retired), "{retired}");
        }
    }

    #[test]
    fn completion_rows_carry_every_declared_field() {
        let completion = |outcome| InvocationCompletionV1 {
            callback_index: 3,
            trigger_id: "clock".parse().expect("trigger id"),
            outcome,
        };
        let success = completion_row(
            (9, 2, 1),
            Some(4),
            "call-hash",
            &completion(TriggerCompletedOutcome::Success),
        );
        let failure = completion_row(
            (9, 2, 0),
            None,
            "call-hash",
            &completion(TriggerCompletedOutcome::Failure("out of fuel".into())),
        );
        let declared: BTreeSet<&str> = specs::TRIGGER_COMPLETIONS
            .fields
            .iter()
            .map(|field| field.name)
            .collect();
        for row in [&success, &failure] {
            assert_eq!(
                row.keys().map(String::as_str).collect::<BTreeSet<_>>(),
                declared,
                "rows and the collection spec name the same fields"
            );
        }
        assert_eq!(success["block_height"], Value::from(9_u64));
        assert_eq!(success["output_index"], Value::from(2_u64));
        assert_eq!(success["completion_index"], Value::from(1_u64));
        assert_eq!(success["trigger_id"], Value::from("clock"));
        assert_eq!(success["trigger_execution_hash"], Value::from("call-hash"));
        assert_eq!(success["entrypoint_index"], Value::from(4_u64));
        assert_eq!(success["step_index"], Value::from(3_u64));
        assert_eq!(success["outcome"], Value::from("Success"));
        assert_eq!(success["message"], Value::Null);
        assert_eq!(failure["entrypoint_index"], Value::Null);
        assert_eq!(failure["outcome"], Value::from("Failure"));
        assert_eq!(failure["message"], Value::from("out of fuel"));
    }
}
