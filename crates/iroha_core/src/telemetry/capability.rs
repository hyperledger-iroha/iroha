//! Telemetry capability shims bridging configuration profiles with runtime handles.

use iroha_config::parameters::actual::{TelemetryCapabilities, TelemetryProfile};

/// Trait implemented by types that can expose telemetry capability decisions.
pub trait TelemetryGate {
    /// Return the active telemetry profile for this gate.
    fn profile(&self) -> TelemetryProfile;

    /// Return the capability set implied by the active profile.
    #[inline]
    fn capabilities(&self) -> TelemetryCapabilities {
        self.profile().capabilities()
    }

    /// Whether lightweight metrics should be collected and exposed.
    #[inline]
    fn allows_metrics(&self) -> bool {
        self.capabilities().metrics_enabled()
    }

    /// Whether expensive metrics/pipelines should be active.
    #[inline]
    fn allows_expensive_metrics(&self) -> bool {
        self.capabilities().expensive_metrics_enabled()
    }

    /// Whether developer-only telemetry outputs should run.
    #[inline]
    fn allows_developer_outputs(&self) -> bool {
        self.capabilities().developer_outputs_enabled()
    }

    /// Whether the gate disables all telemetry outputs.
    #[inline]
    fn is_disabled(&self) -> bool {
        self.profile().is_disabled()
    }
}

impl TelemetryGate for TelemetryProfile {
    fn profile(&self) -> TelemetryProfile {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_gate_derives_capabilities_from_profile() {
        let profile = TelemetryProfile::Extended;
        assert!(profile.allows_metrics());
        assert!(profile.allows_expensive_metrics());
        assert!(!profile.allows_developer_outputs());
        assert!(!profile.is_disabled());

        let disabled = TelemetryProfile::Disabled;
        assert!(!disabled.allows_metrics());
        assert!(!disabled.allows_expensive_metrics());
        assert!(!disabled.allows_developer_outputs());
        assert!(disabled.is_disabled());
    }
}
