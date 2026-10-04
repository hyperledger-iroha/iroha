#[cfg(feature = "app_api")]
fn collect_committed_transaction_page<T>(
    state: &CoreState,
    filter: iroha_data_model::query::dsl::CompoundPredicate<
        iroha_data_model::query::CommittedTransaction,
    >,
    pagination: EffectivePagination,
    fetch_size: Option<u64>,
    count_mode: AppCountMode,
    mut project: impl FnMut(&iroha_data_model::query::CommittedTransaction) -> Option<T>,
) -> Result<PageResult<T>> {
    let take = pagination
        .limit
        .unwrap_or(pagination.cap)
        .min(fetch_size.unwrap_or(pagination.cap));
    let take = usize::try_from(take).unwrap_or(usize::MAX);
    let offset = usize::try_from(pagination.offset).unwrap_or(usize::MAX);
    let mut items = Vec::new();
    items
        .try_reserve_exact(take)
        .map_err(|_| Error::Query(iroha_data_model::ValidationFail::TooComplex))?;
    let mut matched = 0_usize;
    let mut has_more = false;
    let view = state.view();
    iroha_core::smartcontracts::isi::tx::visit_committed_transactions_bounded(
        &view,
        filter,
        app_query_limits().max_fetch_size,
        iroha_core::smartcontracts::isi::tx::transaction_history_byte_limit(
            app_query_limits().max_fetch_size,
        ),
        |transaction, typed_match| {
            if !typed_match {
                return Ok(std::ops::ControlFlow::Continue(()));
            }
            let Some(item) = project(&transaction) else {
                return Ok(std::ops::ControlFlow::Continue(()));
            };
            let position = matched;
            matched = matched.saturating_add(1);
            if position < offset {
                return Ok(std::ops::ControlFlow::Continue(()));
            }
            if items.len() < take {
                items.push(item);
                return Ok(std::ops::ControlFlow::Continue(()));
            }
            has_more = true;
            Ok(if count_mode == AppCountMode::Exact {
                std::ops::ControlFlow::Continue(())
            } else {
                std::ops::ControlFlow::Break(())
            })
        },
    )
    .map_err(|error| Error::Query(iroha_data_model::ValidationFail::QueryFailed(error)))?;
    let returned_until = offset.saturating_add(items.len());
    has_more |= matched > returned_until;
    Ok(PageResult {
        items,
        total: (count_mode == AppCountMode::Exact).then_some(matched),
        has_more,
    })
}
