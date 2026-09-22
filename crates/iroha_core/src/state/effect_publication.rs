//! Joint physical custody for the State indexes changed by carrier publication.
//!
//! All probes precede visibility. A refusal retains the acquired prefix in its
//! caller-owned slot; physical release is separate from notification cleanup.
//! TODO: fund private successor projections so index mutation performs no new
//! allocation or payload retirement inside the publication interval.

use super::*;
use crate::publication_rwlock::PublicationRwLockWriteGuard;

macro_rules! effect_indexes {
    ($apply:ident) => {
        $apply! {
            latest_block_header: Option<BlockHeader>,
            merge_admission: MergeAdmissionState,
            da_commitments: DaCommitmentStore,
            da_confidential_compute: ConfidentialComputeStore,
            da_receipt_cursors: DaReceiptCursorIndex,
            da_shard_cursors: DaShardCursorIndex,
            da_pin_intents: DaPinStore,
            lane_relays: LaneRelayStore,
            lane_manifests: LaneManifestRegistryHandle,
            lane_privacy_registry: LanePrivacyRegistryHandle,
            da_indexes_hydrated: Option<Result<(), DaIndexHydrationError>>,
        }
    };
}

macro_rules! define_indexes {
    ($($field:ident: $ty:ty,)*) => {
        /// Original target locks; constructing the slot acquires nothing.
        pub(in crate::state) struct StateEffectLocks<'state> {
            target: &'state State,
            $(pub(in crate::state) $field: Option<PublicationRwLockWriteGuard<'state, $ty>> ,)*
            pub(in crate::state) sccp_registry_cache: Option<PublicationGuard<'state, SccpRegistryCache>>,
            retired: [concread::release::DeferredReleaseBatch; 12],
            attempted: bool,
            complete: bool,
            retired_manifests: Option<LaneManifestRegistryHandle>,
            retired_privacy: Option<LanePrivacyRegistryHandle>,
            retired_sccp: Option<Arc<ValidatedSccpRegistryV1>>,
        }

        impl<'state> StateEffectLocks<'state> {
            /// Install an inert slot before any sibling starts preparation.
            pub(in crate::state) fn new(target: &'state State) -> Self {
                Self {
                    target,
                    $($field: None,)*
                    sccp_registry_cache: None,
                    retired: [$(target.$field.deferred_releases(),)* target.sccp_registry_cache.deferred_releases()],
                    attempted: false,
                    complete: false,
                    retired_manifests: None,
                    retired_privacy: None,
                    retired_sccp: None,
                }
            }

            /// Retain each acquired original guard before probing its successor.
            /// The returned waiter belongs to the actual reader/writer blocker.
            pub(in crate::state) fn try_prepare(&mut self) -> Result<(), (&'static str, concread::release::ReleaseWait)> {
                assert!(!self.attempted, "effect lock preparation is one-shot");
                self.attempted = true;
                self.prepare_inner()
            }

            fn prepare_inner(&mut self) -> Result<(), (&'static str, concread::release::ReleaseWait)> {
                $(self.$field = Some(self.target.$field.try_write_or_wait()
                    .map_err(|wait| (stringify!($field), wait))?);)*
                self.sccp_registry_cache = Some(self.target.sccp_registry_cache.try_lock_or_wait()
                    .map_err(|wait| ("sccp_registry_cache", wait))?);
                self.complete = true;
                Ok(())
            }

            /// Complete the existing synchronous commit contract before visibility.
            /// Never block while retaining an acquired index prefix: release that
            /// prefix, wait by acquiring only the actual blocking lock, then retry
            /// the full original inventory. Notification cleanup stays retained.
            /// TODO: retire synchronous commit with the funded retained Apply owner.
            pub(in crate::state) fn prepare_blocking(&mut self) {
                assert!(!self.attempted, "effect lock preparation is one-shot");
                self.attempted = true;
                while let Err((field, _wait)) = self.prepare_inner() {
                    self.release_writers();
                    let names = [$(stringify!($field),)* "sccp_registry_cache"];
                    let index = names.iter().position(|name| *name == field).expect("original effect index");
                    match field {
                        $(stringify!($field) => assert!(self.target.$field.write().try_release_into(&mut self.retired[index]).is_ok(), "original effect wait source"),)*
                        "sccp_registry_cache" => assert!(self.target.sccp_registry_cache.lock().try_release_into(&mut self.retired[index]).is_ok(), "original SCCP wait source"),
                        _ => unreachable!("only original effect locks can refuse"),
                    }
                }
            }

            /// Release only physical guards. Every original notification and
            /// displaced registry allocation remains in this owner afterward.
            pub(in crate::state) fn release_writers(&mut self) {
                self.complete = false;
                let mut slots = self.retired.iter_mut();
                $(let retired = slots.next().expect("effect release slot");
                if let Some(guard) = self.$field.take() {
                    assert!(guard.try_release_into(retired).is_ok(), "original effect release source");
                })*
                if let Some(guard) = self.sccp_registry_cache.take() {
                    assert!(guard.try_release_into(slots.next().expect("SCCP release slot")).is_ok(), "original SCCP release source");
                }
            }

            /// Retain the real short preflight read until the enclosing State
            /// cleanup ends, including if validation unwinds. The later writer
            /// coalesces into this same original source batch.
            pub(in crate::state) fn with_merge_admission<R>(&mut self, inspect: impl FnOnce(&MergeAdmissionState) -> R) -> R {
                let names = [$(stringify!($field),)*];
                let index = names.iter().position(|name| *name == "merge_admission").expect("original merge index");
                let read = DeferredIndexRead {
                    guard: Some(self.target.merge_admission.read()),
                    releases: &mut self.retired[index],
                };
                inspect(&read)
            }

            /// Original guards remain owned through the exact registry swap.
            pub(in crate::state) fn install_registries(&mut self, manifests: LaneManifestRegistryHandle, privacy: LanePrivacyRegistryHandle) {
                assert!(self.complete && self.retired_manifests.is_none() && self.retired_privacy.is_none(), "one complete registry publication");
                self.retired_manifests = Some(std::mem::replace(&mut **self.lane_manifests.as_mut().expect("prepared manifests"), manifests));
                self.retired_privacy = Some(std::mem::replace(&mut **self.lane_privacy_registry.as_mut().expect("prepared privacy"), privacy));
            }

            /// Retain the displaced SCCP allocation through all physical fences.
            pub(in crate::state) fn install_sccp(&mut self, registry: Arc<ValidatedSccpRegistryV1>) {
                assert!(self.complete && self.retired_sccp.is_none(), "one complete SCCP publication");
                self.retired_sccp = Some(std::mem::replace(&mut self.sccp_registry_cache.as_mut().expect("prepared SCCP").registry, registry));
            }
        }

        impl Drop for StateEffectLocks<'_> {
            fn drop(&mut self) {
                self.release_writers();
            }
        }
    };
}
effect_indexes!(define_indexes);

#[cfg(test)]
#[path = "effect_publication_tests.rs"]
mod tests;

/// A borrowing physical scope; cleanup stays with the enclosing original slot.
pub(in crate::state) struct EffectLockScope<'scope, 'state>(&'scope mut StateEffectLocks<'state>);
impl<'state> StateEffectLocks<'state> {
    /// Ensure unwind releases index guards before the caller's other writers.
    pub(in crate::state) fn physical_scope(&mut self) -> EffectLockScope<'_, 'state> {
        EffectLockScope(self)
    }
}
impl<'state> std::ops::Deref for EffectLockScope<'_, 'state> {
    type Target = StateEffectLocks<'state>;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
impl std::ops::DerefMut for EffectLockScope<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}
impl Drop for EffectLockScope<'_, '_> {
    fn drop(&mut self) {
        self.0.release_writers();
    }
}

/// A short original index read whose notification remains in the caller's batch.
struct DeferredIndexRead<'scope, 'state, T> {
    guard: Option<crate::publication_rwlock::PublicationRwLockReadGuard<'state, T>>,
    releases: &'scope mut concread::release::DeferredReleaseBatch,
}
impl<T> std::ops::Deref for DeferredIndexRead<'_, '_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.guard.as_deref().expect("original short index read")
    }
}
impl<T> Drop for DeferredIndexRead<'_, '_, T> {
    fn drop(&mut self) {
        if let Some(guard) = self.guard.take() {
            assert!(
                guard.try_release_into(self.releases).is_ok(),
                "original short index release source"
            );
        }
    }
}
