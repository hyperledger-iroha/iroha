//! Desktop navigation choices and their exact persisted identities.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActiveView {
    Dashboard,
    Network,
    Activity,
    State,
    Composer,
    Chaos,
}
impl ActiveView {
    pub(super) fn label(self) -> &'static str {
        match self {
            ActiveView::Dashboard => "Dashboard",
            ActiveView::Network => "Network",
            ActiveView::Activity => "Activity",
            ActiveView::State => "State",
            ActiveView::Composer => "Transactions",
            ActiveView::Chaos => "Chaos Lab",
        }
    }
    pub(super) fn all() -> [ActiveView; 6] {
        [
            ActiveView::Dashboard,
            ActiveView::Network,
            ActiveView::Activity,
            ActiveView::State,
            ActiveView::Composer,
            ActiveView::Chaos,
        ]
    }
    pub(super) fn storage_value(self) -> &'static str {
        match self {
            ActiveView::Dashboard => "dashboard",
            ActiveView::Network => "network",
            ActiveView::Activity => "activity",
            ActiveView::State => "state",
            ActiveView::Composer => "composer",
            ActiveView::Chaos => "chaos",
        }
    }
    pub(super) fn from_storage_value(raw: &str) -> Option<Self> {
        match raw {
            "dashboard" => Some(Self::Dashboard),
            "network" => Some(Self::Network),
            "activity" => Some(Self::Activity),
            "state" => Some(Self::State),
            "composer" => Some(Self::Composer),
            "chaos" => Some(Self::Chaos),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActivityView {
    Logs,
    Events,
    Blocks,
}
impl ActivityView {
    pub(super) fn label(self) -> &'static str {
        match self {
            ActivityView::Logs => "Logs",
            ActivityView::Events => "Events",
            ActivityView::Blocks => "Blocks",
        }
    }
    pub(super) fn all() -> [ActivityView; 3] {
        [
            ActivityView::Logs,
            ActivityView::Events,
            ActivityView::Blocks,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{ActiveView, ActivityView};

    #[test]
    fn every_primary_view_retains_its_persisted_identity_and_label() {
        let expected = [
            ("dashboard", "Dashboard"),
            ("network", "Network"),
            ("activity", "Activity"),
            ("state", "State"),
            ("composer", "Transactions"),
            ("chaos", "Chaos Lab"),
        ];
        for (view, (stored, label)) in ActiveView::all().into_iter().zip(expected) {
            assert_eq!(view.storage_value(), stored);
            assert_eq!(view.label(), label);
            assert_eq!(ActiveView::from_storage_value(stored), Some(view));
        }
        for invalid in ["", " activity ", "Activity", "transactions", "unknown"] {
            assert_eq!(ActiveView::from_storage_value(invalid), None);
        }
    }

    #[test]
    fn activity_tabs_keep_their_order_and_labels() {
        assert_eq!(
            ActivityView::all(),
            [
                ActivityView::Logs,
                ActivityView::Events,
                ActivityView::Blocks
            ]
        );
        assert_eq!(
            ActivityView::all().map(ActivityView::label),
            ["Logs", "Events", "Blocks"]
        );
    }
}
