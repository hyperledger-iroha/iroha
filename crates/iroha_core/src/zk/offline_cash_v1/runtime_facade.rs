//! Public fail-closed wallet-runtime boundary for Offline Cash V1.
//!
//! Core does not yet have a production secure-device backend that can own the
//! complete durable wallet lifecycle. This module therefore exposes the final
//! product vocabulary without exposing any private state-transition owner or
//! pretending that structural verification is a wallet runtime.

/// Stable product state of an Offline Cash V1 wallet session.
///
/// The discriminants are part of the public Core API. They are status codes,
/// not a Norito or device-wire codec.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineCashWalletSessionStateV1 {
    /// No qualifying production wallet runtime is installed.
    Unavailable = 0,
    /// Secure-device setup is required.
    SetupRequired = 1,
    /// Setup completed but the offline balance is empty.
    Empty = 2,
    /// An online top-up is pending.
    TopUpPending = 3,
    /// Offline value is available.
    Available = 4,
    /// A receiver request is ready for peer handoff.
    ReceiveRequestReady = 5,
    /// A sender transition is being prepared.
    SendPreparing = 6,
    /// The sender committed before exposing the payment.
    PaymentCommitted = 7,
    /// A committed sender payment is awaiting acknowledgement evidence.
    AwaitingAcknowledgement = 8,
    /// The receiver persisted the payment before acknowledgement production.
    Received = 9,
    /// An online redemption is pending.
    RedeemPending = 10,
    /// Durable recovery must complete before another action.
    RecoveryRequired = 11,
    /// The wallet runtime reached a terminal local error state.
    Error = 12,
}

/// Availability status of the production Offline Cash V1 wallet runtime.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineCashWalletSessionStatusV1 {
    /// No qualifying secure-device wallet runtime is installed.
    Unavailable = 0,
}

/// High-level wallet action rejected by the fail-closed facade.
///
/// These values select no device command and carry no authority. The facade
/// rejects every variant until Core is joined to a reviewed production
/// secure-device backend and durable owner store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineCashWalletSessionActionV1 {
    /// Configure the secure offline wallet.
    SetUp,
    /// Start an authenticated online top-up.
    TopUp,
    /// Create a receiver-bound payment request.
    CreateReceiveRequest,
    /// Prepare a sender transition.
    PrepareSend,
    /// Commit a prepared sender payment before handoff.
    CommitPayment,
    /// Record receiver acknowledgement as evidence only.
    RecordAcknowledgementEvidence,
    /// Persist a verified receiver payment.
    ReceivePayment,
    /// Start an authenticated online redemption.
    Redeem,
    /// Recover the exact durable lifecycle state.
    Recover,
}

/// Failure returned by the fail-closed wallet-runtime facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineCashWalletSessionErrorV1 {
    /// A qualifying production secure-device runtime is unavailable.
    Unavailable,
}

impl core::fmt::Display for OfflineCashWalletSessionErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unavailable => {
                formatter.write_str("production Offline Cash V1 wallet runtime is unavailable")
            }
        }
    }
}

impl std::error::Error for OfflineCashWalletSessionErrorV1 {}

/// Opaque fail-closed Core facade for one Offline Cash V1 wallet session.
///
/// This value is deliberately non-`Clone`, has no Norito/schema codec, stores
/// no balance, guard, outbox, verifier receipt, device owner, byte buffer, or
/// native handle, and accepts no caller-supplied wall clock. It cannot be used
/// to enable offline funds. [`Self::open`] and every action return
/// [`OfflineCashWalletSessionErrorV1::Unavailable`] until the reviewed secure
/// runtime exists.
#[must_use = "the unavailable wallet-session status must be observed"]
#[allow(
    missing_copy_implementations,
    reason = "the opaque wallet-runtime boundary must remain move-only even while its sentinel is zero-sized"
)]
pub struct OfflineCashWalletSessionV1 {
    _unavailable: (),
}

impl core::fmt::Debug for OfflineCashWalletSessionV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashWalletSessionV1")
            .field("status", &OfflineCashWalletSessionStatusV1::Unavailable)
            .finish_non_exhaustive()
    }
}

impl OfflineCashWalletSessionV1 {
    /// Construct the inert status sentinel.
    ///
    /// The returned value owns no runtime capability and cannot perform an
    /// action. It exists so callers can inspect the explicit unavailable state
    /// without manufacturing a native handle.
    #[must_use]
    pub const fn unavailable() -> Self {
        Self { _unavailable: () }
    }

    /// Attempt to open a production wallet runtime.
    ///
    /// # Errors
    ///
    /// Always returns [`OfflineCashWalletSessionErrorV1::Unavailable`].
    pub const fn open() -> Result<Self, OfflineCashWalletSessionErrorV1> {
        Err(OfflineCashWalletSessionErrorV1::Unavailable)
    }

    /// Return the explicit production-runtime availability status.
    #[must_use]
    pub const fn status(&self) -> OfflineCashWalletSessionStatusV1 {
        OfflineCashWalletSessionStatusV1::Unavailable
    }

    /// Return the inert facade's stable product state.
    #[must_use]
    pub const fn state(&self) -> OfflineCashWalletSessionStateV1 {
        OfflineCashWalletSessionStateV1::Unavailable
    }

    /// Attempt one high-level wallet action without accepting host time or
    /// returning bytes, handles, or state-transition owners.
    ///
    /// # Errors
    ///
    /// Always returns [`OfflineCashWalletSessionErrorV1::Unavailable`] and
    /// leaves the facade in [`OfflineCashWalletSessionStateV1::Unavailable`].
    pub const fn attempt(
        &mut self,
        _action: OfflineCashWalletSessionActionV1,
    ) -> Result<(), OfflineCashWalletSessionErrorV1> {
        Err(OfflineCashWalletSessionErrorV1::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATES: [OfflineCashWalletSessionStateV1; 13] = [
        OfflineCashWalletSessionStateV1::Unavailable,
        OfflineCashWalletSessionStateV1::SetupRequired,
        OfflineCashWalletSessionStateV1::Empty,
        OfflineCashWalletSessionStateV1::TopUpPending,
        OfflineCashWalletSessionStateV1::Available,
        OfflineCashWalletSessionStateV1::ReceiveRequestReady,
        OfflineCashWalletSessionStateV1::SendPreparing,
        OfflineCashWalletSessionStateV1::PaymentCommitted,
        OfflineCashWalletSessionStateV1::AwaitingAcknowledgement,
        OfflineCashWalletSessionStateV1::Received,
        OfflineCashWalletSessionStateV1::RedeemPending,
        OfflineCashWalletSessionStateV1::RecoveryRequired,
        OfflineCashWalletSessionStateV1::Error,
    ];

    const ACTIONS: [OfflineCashWalletSessionActionV1; 9] = [
        OfflineCashWalletSessionActionV1::SetUp,
        OfflineCashWalletSessionActionV1::TopUp,
        OfflineCashWalletSessionActionV1::CreateReceiveRequest,
        OfflineCashWalletSessionActionV1::PrepareSend,
        OfflineCashWalletSessionActionV1::CommitPayment,
        OfflineCashWalletSessionActionV1::RecordAcknowledgementEvidence,
        OfflineCashWalletSessionActionV1::ReceivePayment,
        OfflineCashWalletSessionActionV1::Redeem,
        OfflineCashWalletSessionActionV1::Recover,
    ];

    #[test]
    fn wallet_session_state_vocabulary_is_exactly_thirteen_stable_codes() {
        assert_eq!(
            STATES.map(|state| state as u8),
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
        );
        assert_eq!(
            STATES.map(|state| format!("{state:?}")),
            [
                "Unavailable",
                "SetupRequired",
                "Empty",
                "TopUpPending",
                "Available",
                "ReceiveRequestReady",
                "SendPreparing",
                "PaymentCommitted",
                "AwaitingAcknowledgement",
                "Received",
                "RedeemPending",
                "RecoveryRequired",
                "Error",
            ]
        );
    }

    #[test]
    fn unavailable_facade_returns_no_action_output_or_handle_and_never_advances() {
        assert!(matches!(
            OfflineCashWalletSessionV1::open(),
            Err(OfflineCashWalletSessionErrorV1::Unavailable)
        ));
        assert_eq!(core::mem::size_of::<OfflineCashWalletSessionV1>(), 0);

        let mut session = OfflineCashWalletSessionV1::unavailable();
        assert_eq!(
            session.status(),
            OfflineCashWalletSessionStatusV1::Unavailable
        );
        assert_eq!(
            session.state(),
            OfflineCashWalletSessionStateV1::Unavailable
        );
        for action in ACTIONS {
            let outcome: Result<(), OfflineCashWalletSessionErrorV1> = session.attempt(action);
            assert_eq!(outcome, Err(OfflineCashWalletSessionErrorV1::Unavailable));
            assert_eq!(
                session.state(),
                OfflineCashWalletSessionStateV1::Unavailable
            );
        }
    }

    #[test]
    fn wallet_session_facade_is_non_clone_non_codec_and_owner_free_by_source_contract() {
        let source = include_str!("runtime_facade.rs");
        assert!(
            source.contains("pub struct OfflineCashWalletSessionV1 {\n    _unavailable: (),\n}")
        );
        let declaration_prefix = source
            .split_once("pub struct OfflineCashWalletSessionV1 {")
            .expect("wallet-session declaration exists")
            .0
            .rsplit_once("\n\n")
            .expect("wallet-session declaration has an attribute boundary")
            .1;
        assert!(!declaration_prefix.contains("derive"));
        for trait_name in ["Clone", "Encode", "Decode", "IntoSchema"] {
            let forbidden = format!("impl {trait_name} for OfflineCashWalletSessionV1");
            assert!(!source.contains(&forbidden));
        }

        let implementation = source
            .split_once("impl OfflineCashWalletSessionV1 {")
            .expect("wallet-session implementation exists")
            .1
            .split_once("#[cfg(test)]")
            .expect("wallet-session implementation precedes its tests")
            .0;
        assert!(!implementation.contains("now_ms"));
        assert!(!implementation.contains("SystemTime"));
        assert!(!implementation.contains("Instant"));
        assert!(implementation.contains(
            ") -> Result<(), OfflineCashWalletSessionErrorV1> {\n        Err(OfflineCashWalletSessionErrorV1::Unavailable)"
        ));
    }
}
