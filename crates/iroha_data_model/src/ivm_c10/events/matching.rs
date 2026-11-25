//! Borrowed matching for C10 triggers and subscriptions.

use super::{
    EventBoxC10V1, EventFilterBoxC10V1, PipelineEventBoxC10V1, PipelineEventFilterBoxC10V1,
    TransactionEventFilterC10V1, TransactionStatusC10V1,
};
use crate::events::{
    EventBox, EventFilter,
    pipeline::{PipelineEventBox, TransactionEvent, TransactionStatus},
};

impl TransactionEventFilterC10V1 {
    fn matches_baseline(&self, event: &TransactionEvent) -> bool {
        self.matches_source(
            event.hash,
            event.block_height,
            event.lane_id,
            event.dataspace_id,
        ) && self
            .status
            .as_ref()
            .is_none_or(|expected| match &event.status {
                TransactionStatus::Queued => matches!(expected, TransactionStatusC10V1::Queued),
                TransactionStatus::Expired => matches!(expected, TransactionStatusC10V1::Expired),
                TransactionStatus::Approved => matches!(expected, TransactionStatusC10V1::Approved),
                // An exact C10 rejection predicate names a different result definition.
                TransactionStatus::Rejected(_) => false,
            })
    }
}

impl EventFilter for PipelineEventFilterBoxC10V1 {
    type Event = PipelineEventBoxC10V1;

    fn matches(&self, event: &Self::Event) -> bool {
        match event {
            PipelineEventBoxC10V1::Transaction(event) => {
                matches!(self, Self::Transaction(filter) if filter.matches(event))
            }
            PipelineEventBoxC10V1::Block(event) => {
                matches!(self, Self::Block(filter) if filter.matches_event(event))
            }
            PipelineEventBoxC10V1::Witness(event) => {
                matches!(self, Self::Witness(filter) if filter.matches(event))
            }
            PipelineEventBoxC10V1::Warning(_) => false,
        }
    }
}

impl PipelineEventFilterBoxC10V1 {
    /// Match a baseline event against the exact C10 predicate.
    pub fn matches_baseline(&self, event: &PipelineEventBox) -> bool {
        match event {
            PipelineEventBox::Transaction(event) => {
                matches!(self, Self::Transaction(filter) if filter.matches_baseline(event))
            }
            PipelineEventBox::Block(event) => {
                matches!(self, Self::Block(filter) if filter.matches_event(event))
            }
            PipelineEventBox::Witness(event) => {
                matches!(self, Self::Witness(filter) if filter.matches(event))
            }
            PipelineEventBox::Warning(_) => false,
        }
    }
}

// These enclosing event families share each leaf except the pipeline. Keep the single-event
// and batch rules in one match, with the typed pipeline predicate chosen by the caller.
macro_rules! matches_enclosing_event {
    ($filter:expr, $event:expr, $family:ident, $pipeline_match:ident) => {
        match $event {
            $family::Pipeline(event) => {
                matches!($filter, Self::Pipeline(filter) if filter.$pipeline_match(event))
            }
            $family::PipelineBatch(events) => {
                matches!($filter, Self::Pipeline(filter) if events.iter().any(|event| filter.$pipeline_match(event)))
            }
            $family::Data(event) => matches!($filter, Self::Data(filter) if filter.matches(event)),
            $family::Time(event) => matches!($filter, Self::Time(filter) if filter.matches(event)),
            $family::ExecuteTrigger(event) => {
                matches!($filter, Self::ExecuteTrigger(filter) if filter.matches(event))
            }
            $family::TriggerCompleted(event) => {
                matches!($filter, Self::TriggerCompleted(filter) if filter.matches(event))
            }
        }
    };
}

impl EventFilter for EventFilterBoxC10V1 {
    type Event = EventBoxC10V1;

    fn matches(&self, event: &Self::Event) -> bool {
        matches_enclosing_event!(self, event, EventBoxC10V1, matches)
    }

    fn mintable(&self) -> bool {
        match self {
            Self::Time(filter) => filter.mintable(),
            _ => true,
        }
    }
}

impl EventFilterBoxC10V1 {
    /// Match a borrowed baseline event observed by a C10 trigger or subscription.
    ///
    /// A transaction filter with no status constraint can observe a baseline rejection. An
    /// exact C10 rejection can only match an event carrying the C10 rejection family.
    #[must_use]
    pub fn matches_baseline(&self, event: &EventBox) -> bool {
        matches_enclosing_event!(self, event, EventBox, matches_baseline)
    }
}

impl PipelineEventBoxC10V1 {
    /// Match a baseline subscription or trigger on a root that selected both event families.
    /// Only statuses with the same definition compare across the two families.
    #[must_use]
    pub fn matches_baseline_filter(
        &self,
        filter: &crate::events::pipeline::PipelineEventFilterBox,
    ) -> bool {
        use crate::events::pipeline::PipelineEventFilterBox as Filter;
        match (self, filter) {
            (Self::Transaction(event), Filter::Transaction(f)) => {
                f.hash().is_none_or(|x| x == event.hash)
                    && f.block_height.is_none_or(|x| x == event.block_height)
                    && f.lane_id.is_none_or(|x| x == event.lane_id)
                    && f.dataspace_id.is_none_or(|x| x == event.dataspace_id)
                    && f.status().as_ref().is_none_or(|status| {
                        matches!(
                            (status, &event.status),
                            (
                                TransactionStatus::Approved,
                                TransactionStatusC10V1::Approved
                            ) | (TransactionStatus::Queued, TransactionStatusC10V1::Queued)
                                | (TransactionStatus::Expired, TransactionStatusC10V1::Expired)
                        )
                    })
            }
            (Self::Block(event), Filter::Block(f)) => f.matches_event(event),
            (Self::Witness(event), Filter::Witness(f)) => f.matches(event),
            _ => false,
        }
    }
}
