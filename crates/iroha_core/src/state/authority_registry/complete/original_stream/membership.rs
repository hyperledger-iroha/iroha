//! Atomic membership pair from one exact already prepared original transition.
//!
//! Complete capture replaces both complete tables, including empty replacements.
//! Change capture borrows the existing owner's committed-pair delta, retaining
//! exact original preimages and its distinct committed/logical frontier metadata.
//! The keyed State owner uses these bytes for the execution and publication cuts
//! and checks the undo entries against its retained predecessor.

use super::*;
use crate::state::storage_transactions::authority::{
    TransactionMembershipPublicationCut, TransactionMembershipPublicationDelta,
    TransactionMembershipSide, TransactionMembershipVisitError,
};

pub(super) fn visit_error(error: TransactionMembershipVisitError<CaptureError>) -> CaptureError {
    match error {
        TransactionMembershipVisitError::Consumer(error) => error,
        TransactionMembershipVisitError::Authority(error) => {
            CaptureError::Membership(MembershipCaptureRefusal(error))
        }
    }
}

pub(super) fn append(block: &StateBlock<'_>, output: &mut Collector) -> Result<(), CaptureError> {
    let fields = block.fields.as_ref().ok_or(CaptureError::NotFrozen)?;
    let writer = fields
        .transactions
        .prepared_membership_writer()
        .ok_or(CaptureError::NotFrozen)?;
    if !writer.belongs_to(&fields.state_ref.transactions)
        || writer.mode() != fields.world.parameters.mode()
    {
        return Err(CaptureError::Changed("state.transactions"));
    }
    if output.intent == CaptureIntent::Changes && !cfg!(all(test, sumeragi_core_mutation = "HC453"))
    {
        let surface = writer.publication_surface();
        let source = writer
            .membership_publication_delta(&surface, usize::MAX)
            .map_err(membership_error)?;
        return append_changes(surface, source, output);
    }
    let source = writer
        .membership_publication_authority_cut(usize::MAX)
        .map_err(membership_error)?;
    append_complete(writer.publication_surface(), source, output)
}

fn membership_error(
    error: crate::state::storage_transactions::authority::TransactionMembershipAuthorityError,
) -> CaptureError {
    CaptureError::Membership(MembershipCaptureRefusal(error))
}

/// Observe the staged successor from the original active writer or its logically prepared form.
/// Reading never stages a carrier or issues a second acquisition of its history.
pub(super) fn append_executing(
    fields: &crate::state::StateBlockFields<'_>,
    output: &mut Collector,
) -> Result<(), CaptureError> {
    let mode = fields.world.parameters.mode();
    let target = &fields.state_ref.transactions;
    let (surface, transition) =
        if let Some(writer) = fields.transactions.executing_membership_writer() {
            if !writer.belongs_to(target) || writer.mode() != mode {
                return Err(CaptureError::Changed("state.transactions"));
            }
            let transition = writer
                .membership_transition()
                .map_err(|error| CaptureError::Source(error.to_string()))?;
            (writer.publication_surface(), transition)
        } else if let Some(writer) = fields.transactions.prepared_membership_writer() {
            if !writer.belongs_to(target) || writer.mode() != mode {
                return Err(CaptureError::Changed("state.transactions"));
            }
            (writer.publication_surface(), writer.membership_transition())
        } else {
            return Err(CaptureError::Changed("state.transactions"));
        };
    if output.intent == CaptureIntent::Changes {
        let delta = transition
            .admit_delta(surface.clone(), &surface, usize::MAX)
            .map_err(membership_error)?;
        return append_changes(surface, delta, output);
    }
    let source = transition
        .admit_complete(usize::MAX)
        .map_err(membership_error)?;
    append_complete(surface, source, output)
}

fn append_complete(
    surface: crate::state::storage_transactions::TransactionsPublicationSurface,
    source: TransactionMembershipPublicationCut<'_>,
    output: &mut Collector,
) -> Result<(), CaptureError> {
    let before = source
        .predecessor_frontier_height()
        .map_err(membership_error)?;
    append_pair(
        surface,
        source.row_visits(),
        source.frontier_height(),
        before,
        |visitor| source.visit(visitor).map_err(visit_error),
        output,
    )
}

/// Rebuild both committed maps and the exact frontier of a pristine original writer.
/// Its staged successor already advances the frontier even with no transactions.
pub(super) fn append_committed(
    fields: &crate::state::StateBlockFields<'_>,
    output: &mut Collector,
) -> Result<(), CaptureError> {
    let writer = fields
        .transactions
        .executing_membership_writer()
        .ok_or(CaptureError::NotFrozen)?;
    if !writer.belongs_to(&fields.state_ref.transactions)
        || writer.mode() != mv::BlockMode::Ordinary
    {
        return Err(CaptureError::Changed("state.transactions"));
    }
    let source = || {
        writer
            .membership_authority_cut(usize::MAX)
            .map_err(membership_error)
    };
    let cut = source()?;
    let frontier = cut.frontier_height();
    append_pair(
        writer.publication_surface(),
        cut.row_visits(),
        frontier,
        frontier,
        |visitor| source()?.visit(visitor).map_err(visit_error),
        output,
    )
}

type RowVisitor<'a> = dyn FnMut(
        TransactionMembershipSide,
        &iroha_crypto::HashOf<iroha_data_model::transaction::TransactionEntrypoint>,
        u64,
    ) -> Result<(), CaptureError>
    + 'a;

fn append_pair(
    surface: crate::state::storage_transactions::TransactionsPublicationSurface,
    row_visits: usize,
    current: u64,
    before: u64,
    mut visit: impl FnMut(&mut RowVisitor<'_>) -> Result<(), CaptureError>,
    output: &mut Collector,
) -> Result<(), CaptureError> {
    let mut counts = [0usize; 2];
    output.admit_source_visits(row_visits)?;
    visit(&mut |side, _, _| {
        let index = usize::from(side == TransactionMembershipSide::Rollback);
        counts[index] = counts[index]
            .checked_add(1)
            .ok_or(CaptureError::CountLimit)?;
        Ok(())
    })?;
    for (index, side, id) in [
        (
            0,
            TransactionMembershipSide::Current,
            "state.transactions.current",
        ),
        (
            1,
            TransactionMembershipSide::Rollback,
            "state.transactions.rollback",
        ),
    ] {
        if !output.begin(
            id,
            SourceIdentity::Membership(surface.clone()),
            counts[index],
            0,
        )? {
            continue;
        }
        output
            .pending
            .as_mut()
            .expect("same original membership field")
            .replacement = true;
        let retained = output.retained(false);
        let mut seen = 0usize;
        output.admit_source_visits(row_visits)?;
        visit(&mut |which, key, height| {
            if which == side {
                if seen >= retained {
                    output.row(
                        false,
                        |writer| bare(key, writer),
                        Some(|writer: &mut dyn Write| bare(&height, writer)),
                    )?;
                }
                seen += 1;
            }
            Ok(())
        })?;
        output.finish_field()?;
    }
    output.projected_cell(
        "state.transactions.frontier",
        SourceIdentity::Membership(surface.clone()),
        // This is an admitted original frontier publication, including a
        // repeated equal-height touch. Preserve its exact predecessor value.
        true,
        |writer| bare(&current, writer),
        |writer| bare(&before, writer),
    )
}

/// Retain only exact paired changes, with no scan of accumulated membership.
fn append_changes(
    expected: crate::state::storage_transactions::TransactionsPublicationSurface,
    source: TransactionMembershipPublicationDelta<'_>,
    output: &mut Collector,
) -> Result<(), CaptureError> {
    output.bind_membership_delta(source.base())?;
    let fields = [
        (
            TransactionMembershipSide::Current,
            "state.transactions.current",
        ),
        (
            TransactionMembershipSide::Rollback,
            "state.transactions.rollback",
        ),
    ];
    let mut complete = [false; 2];
    for (index, (_, id)) in fields.iter().enumerate() {
        complete[index] =
            output.already_captured(id, &SourceIdentity::Membership(expected.clone()))?;
    }
    if complete != [true, true] {
        // Count only the actual delta key sets. All subsequent filtered passes
        // admit the same complete pair's physical visits, including skipped
        // prefixes; none traverses historical rows.
        let mut counts = [0usize; 2];
        output.admit_source_visits(source.row_visits())?;
        source
            .visit(|side, _, _, _| {
                let index = usize::from(side == TransactionMembershipSide::Rollback);
                counts[index] = counts[index]
                    .checked_add(1)
                    .ok_or(CaptureError::CountLimit)?;
                Ok(())
            })
            .map_err(visit_error)?;
        for (index, (side, id)) in fields.into_iter().enumerate() {
            if complete[index] {
                continue;
            }
            if !output.begin(
                id,
                SourceIdentity::Membership(expected.clone()),
                counts[index],
                counts[index],
            )? {
                continue;
            }
            #[cfg(all(test, sumeragi_core_mutation = "HC454"))]
            let _replaced_original_key = output.pending.as_mut().and_then(|field| field.key.take());
            for undo in [false, true] {
                let retained = output.retained(undo);
                let mut seen = 0usize;
                output.admit_source_visits(source.row_visits())?;
                source
                    .visit(|which, key, before, after| {
                        if which == side {
                            if seen >= retained {
                                let value = if undo { before } else { after };
                                output.row(
                                    undo,
                                    |writer| bare(key, writer),
                                    value.map(|value| {
                                        move |writer: &mut dyn Write| bare(&value, writer)
                                    }),
                                )?;
                            }
                            seen += 1;
                        }
                        Ok(())
                    })
                    .map_err(visit_error)?;
            }
            output.finish_field()?;
        }
    }
    output.projected_cell(
        "state.transactions.frontier",
        SourceIdentity::Membership(expected),
        true,
        |writer| bare(&source.base().successor_frontier(), writer),
        |writer| bare(&source.base().committed_frontier(), writer),
    )
}
