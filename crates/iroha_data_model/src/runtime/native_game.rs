//! Selected native game transfers: destination reservation precedes irreversible source export.
//!
//! Every proof authenticates an ordinary write of its independently signed root. Reservations
//! have no timeout: only the exact certified import completes destination custody, and only its
//! acknowledgement completes the source capture. Permanent unit tombstones never disappear.

use crate::{
    game_partition::{GAME_PARTITION_MAX_WORK, GameUnitExportV1, GameUnitV1},
    sumeragi_amx::{AmxCertifiedBlockV1, AmxWriteProofV1},
};
use iroha_crypto::Hash;
use norito::codec::{Decode, Encode};

use super::obligation::CompletionCaptureV1;

/// Selected service preserving both roots' original game completion semantics.
pub const SERVICE: &str = "native_game_capture";
/// Immutable reservation, export, import and final source-acknowledgement semantics.
pub const SERVICE_CLOSURE: &[u8] =
    b"iroha:runtime:native-game-capture:v1:source-intent-destination-reservation-export-import-ack-preexport-certified-cancel";

macro_rules! record {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$f:meta])* pub $field:ident: $ty:ty,)* }) => {
        #[derive(Debug,Clone,PartialEq,Eq,PartialOrd,Ord,Encode,Decode,norito::NoritoSchema,iroha_schema::IntoSchema,crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
        $(#[$m])*
        #[norito(deny_unknown_fields)]
        pub struct $name { $($(#[$f])* pub $field:$ty,)* }
    };
}

record! {
    /// Source request locking its original unit and completion graph before remote admission.
    #[norito_schema(name = "iroha_data_model::runtime::native_game::GameUnitExportIntentV1")]
    pub struct GameUnitExportIntentV1 {
        /// Shared registered application identity.
        pub world_id: Hash,
        /// Sole root authorized to export the unit.
        pub source: [u8; 32],
        /// Sole root from which the source may accept a certified reservation.
        pub destination: [u8; 32],
        /// Exact installed deterministic movement semantics.
        pub kernel_hash: [u8; 32],
        /// Complete expected post-export unit, including its incremented sequence.
        pub unit: GameUnitV1,
        /// Owner-chosen request identity; a canceled nonce is never reused for this unit sequence.
        pub nonce: u64,
        /// Original certified source request carrier height.
        pub source_height: u64,
    }
}
record! {
    /// Source abort committed while its original unit remained unexported and locked.
    #[norito_schema(name = "iroha_data_model::runtime::native_game::GameUnitExportCancelV1")]
    pub struct GameUnitExportCancelV1 {
        /// Exact admitted original request that can never export after this transition.
        pub intent: GameUnitExportIntentV1,
        /// Original source cancellation carrier height.
        pub source_height: u64,
    }
}
record! {
    /// Authentic cancellation proof; only an unexported source request may produce it.
    #[norito_schema(name = "iroha_data_model::runtime::native_game::GameUnitExportCancelProofV1")]
    pub struct GameUnitExportCancelProofV1 {
        /// Permanent original-source cancellation.
        pub record: GameUnitExportCancelV1,
        /// Source finality and authenticated result preimage.
        pub block: AmxCertifiedBlockV1,
        /// Ordinary-write path of the exact source cancellation.
        pub path: AmxWriteProofV1,
    }
}
record! {
    /// Exact destination acceptance, committed before the source may remove its unit.
    #[norito_schema(name = "iroha_data_model::runtime::native_game::GameUnitReservationV1")]
    pub struct GameUnitReservationV1 {
        /// Certified source request and complete locked unit identity.
        pub intent: GameUnitExportIntentV1,
        /// Original certified destination carrier height.
        pub destination_height: u64,
    }
}
record! {
    /// Authentic source intent and original unit lock admitted before destination acceptance.
    #[norito_schema(name = "iroha_data_model::runtime::native_game::GameUnitExportIntentProofV1")]
    pub struct GameUnitExportIntentProofV1 {
        /// Original source request and sole destination.
        pub record: GameUnitExportIntentV1,
        /// Authentic source finality and result preimage.
        pub block: AmxCertifiedBlockV1,
        /// Ordinary-write path of this exact request.
        pub path: AmxWriteProofV1,
    }
}
record! {
    /// Source custody removal bound to one previously authenticated destination reservation.
    #[norito_schema(name = "iroha_data_model::runtime::native_game::ReservedGameUnitExportV1")]
    pub struct ReservedGameUnitExportV1 {
        /// Exact destination acceptance used by the original source operation.
        pub reservation: GameUnitReservationV1,
        /// Immutable source tombstone and complete exported unit.
        pub export: GameUnitExportV1,
    }
}
record! {
    /// Destination's immutable assertion that the exact source export was imported once.
    #[derive(Copy)]
    #[norito_schema(name = "iroha_data_model::runtime::native_game::GameUnitImportAckV1")]
    pub struct GameUnitImportAckV1 {
        /// Identity of the original reservation and transfer.
        pub transfer_id: Hash,
        /// Exact canonical reserved export, including both roots and all unit fields.
        pub export_hash: Hash,
        /// Original destination import carrier height.
        pub destination_height: u64,
    }
}
record! {
    /// Public proof of the destination's original reservation; no relay gains authority.
    #[norito_schema(name = "iroha_data_model::runtime::native_game::GameUnitReservationProofV1")]
    pub struct GameUnitReservationProofV1 {
        /// Complete destination reservation.
        pub record: GameUnitReservationV1,
        /// Authentic destination finality and result preimage.
        pub block: AmxCertifiedBlockV1,
        /// Ordinary-write path of this exact reservation.
        pub path: AmxWriteProofV1,
    }
}
record! {
    /// Public proof of irreversible source removal under an accepted reservation.
    #[norito_schema(name = "iroha_data_model::runtime::native_game::ReservedGameUnitExportProofV1")]
    pub struct ReservedGameUnitExportProofV1 {
        /// Complete original reserved export.
        pub record: ReservedGameUnitExportV1,
        /// Authentic source finality and result preimage.
        pub block: AmxCertifiedBlockV1,
        /// Ordinary-write path of this exact source transition.
        pub path: AmxWriteProofV1,
    }
}
record! {
    /// Public proof of exact destination import, used by the source's final ACK.
    #[norito_schema(name = "iroha_data_model::runtime::native_game::GameUnitImportAckProofV1")]
    pub struct GameUnitImportAckProofV1 {
        /// Complete original import acknowledgement.
        pub record: GameUnitImportAckV1,
        /// Authentic destination finality and result preimage.
        pub block: AmxCertifiedBlockV1,
        /// Ordinary-write path of this exact acknowledgement.
        pub path: AmxWriteProofV1,
    }
}

/// Root-local custody of one selected transfer.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::native_game::NativeGameRoleV1")]
#[norito(tag = "kind", content = "content")]
pub enum NativeGameRoleV1 {
    /// Destination capacity and completion semantics are reserved until certified import.
    Destination,
    /// Source locks its intent, then retains its irreversible export until verified ACK.
    Source,
}
record! {
    /// Canonical selected custody and permanent proof discovery for one transfer on this root.
    #[norito_schema(name = "iroha_data_model::runtime::native_game::NativeGameTransferV1")]
    pub struct NativeGameTransferV1 {
        /// Which root owns this record and original completion capability.
        pub role: NativeGameRoleV1,
        /// Original source request, unchanged throughout the transfer.
        pub intent: GameUnitExportIntentV1,
        /// Destination acceptance, absent only while source still awaits it.
        pub reservation: Option<GameUnitReservationV1>,
        /// Source transition, absent only while destination still awaits it.
        pub export: Option<ReservedGameUnitExportV1>,
        /// Certified destination import, absent while the local obligation remains live.
        pub acknowledgement: Option<GameUnitImportAckV1>,
        /// Authentic source cancellation before any export; never a timeout release.
        pub cancellation: Option<GameUnitExportCancelV1>,
        /// Original native completion closure; no timeout or owner action clears it.
        pub capture: Option<CompletionCaptureV1>,
    }
}

fn witness_key(prefix: &[u8], id: Hash) -> Vec<u8> {
    let mut key = prefix.to_vec();
    key.extend_from_slice(id.as_ref());
    key
}
impl GameUnitExportIntentV1 {
    /// Stable identity shared by reservation, export, import and final source ACK.
    pub fn transfer_id(&self) -> Hash {
        Hash::new_from_chunks(&[
            b"iroha/game-reserved-transfer/v1",
            self.world_id.as_ref(),
            &self.source,
            &self.destination,
            self.unit.unit_id.as_ref(),
            &self.unit.sequence.to_le_bytes(),
            &self.nonce.to_le_bytes(),
        ])
    }
    /// Reserved ordinary-write key of the admitted source request and unit lock.
    pub fn witness_key(&self) -> Vec<u8> {
        witness_key(
            b"iroha/game-partition/export-intent/v1/",
            self.transfer_id(),
        )
    }
    /// Check intrinsic identity and finite unit bounds before proof or State admission.
    ///
    /// # Errors
    /// Equal roots, zero carrier/sequence or excessive remaining work is invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.source == self.destination
            || self.source_height == 0
            || self.unit.sequence == 0
            || self.unit.work_remaining > GAME_PARTITION_MAX_WORK
        {
            return Err("invalid reserved game transfer identity or unit bounds".into());
        }
        Ok(())
    }
}
impl GameUnitReservationV1 {
    /// Identity of the original admitted source request.
    pub fn transfer_id(&self) -> Hash {
        self.intent.transfer_id()
    }
    /// Reserved ordinary-write key of the destination acceptance.
    pub fn witness_key(&self) -> Vec<u8> {
        witness_key(b"iroha/game-partition/reservation/v1/", self.transfer_id())
    }
    /// Check the source request and original destination carrier.
    ///
    /// # Errors
    /// Invalid request identity or absent destination carrier.
    pub fn validate(&self) -> Result<(), String> {
        self.intent.validate()?;
        if self.destination_height == 0 {
            return Err("game reservation has no destination carrier".into());
        }
        Ok(())
    }
}
impl ReservedGameUnitExportV1 {
    /// Reserved ordinary-write key of source custody removal.
    pub fn witness_key(&self) -> Vec<u8> {
        witness_key(
            b"iroha/game-partition/reserved-export/v1/",
            self.reservation.transfer_id(),
        )
    }
    /// Digest of every immutable source transition field.
    ///
    /// # Errors
    /// Canonical encoding refused local resources.
    pub fn digest(&self) -> Result<Hash, norito::Error> {
        norito::encode_canonical(self)
            .map(|bytes| Hash::new_from_chunks(&[b"iroha/game-reserved-export/v1", &bytes]))
    }
    /// Check that source removal fulfills the exact accepted destination reservation.
    ///
    /// # Errors
    /// Any substituted root, world, kernel, unit or absent source carrier refuses.
    pub fn validate(&self) -> Result<(), String> {
        self.reservation.validate()?;
        let expected = &self.reservation.intent;
        if self.export.world_id != expected.world_id
            || self.export.source != expected.source
            || self.export.destination != expected.destination
            || self.export.kernel_hash != expected.kernel_hash
            || self.export.unit != expected.unit
            || self.export.source_height == 0
        {
            return Err("reserved game export differs from destination acceptance".into());
        }
        Ok(())
    }
}
impl GameUnitImportAckV1 {
    /// Reserved ordinary-write key of the irreversible destination import.
    pub fn witness_key(&self) -> Vec<u8> {
        witness_key(b"iroha/game-partition/import-ack/v1/", self.transfer_id)
    }
}
impl GameUnitExportCancelV1 {
    /// Reserved ordinary-write key proving this original source can never export the request.
    pub fn witness_key(&self) -> Vec<u8> {
        witness_key(
            b"iroha/game-partition/cancel-intent/v1/",
            self.intent.transfer_id(),
        )
    }
}
impl NativeGameTransferV1 {
    /// Intrinsic shape of the committed original and terminal record.
    ///
    /// # Errors
    /// Invalid or substituted identities, missing source custody or fabricated completion.
    pub fn validate(&self) -> Result<(), String> {
        self.intent.validate()?;
        if self.capture.is_some() != (self.acknowledgement.is_none() && self.cancellation.is_none())
            || self.acknowledgement.is_some() && self.cancellation.is_some()
            || self.role == NativeGameRoleV1::Source
                && self.reservation.is_some() != self.export.is_some()
            || self.role == NativeGameRoleV1::Destination
                && (self.reservation.is_none()
                    || self.export.is_some() != self.acknowledgement.is_some())
        {
            return Err("invalid native game custody/capture lifetime".into());
        }
        if let Some(cancel) = &self.cancellation {
            if cancel.intent != self.intent || cancel.source_height == 0 || self.export.is_some() {
                return Err(
                    "game cancellation does not identify an unexported original request".into(),
                );
            }
        }
        if let Some(reservation) = &self.reservation {
            reservation.validate()?;
            if reservation.intent != self.intent {
                return Err("game reservation differs from original source intent".into());
            }
        }
        if let Some(export) = &self.export {
            export.validate()?;
            if Some(&export.reservation) != self.reservation.as_ref() {
                return Err("native game capture has a substituted reservation".into());
            }
        }
        if let Some(ack) = &self.acknowledgement {
            let export = self
                .export
                .as_ref()
                .ok_or("game ACK has no source export")?;
            if ack.transfer_id != self.intent.transfer_id()
                || ack.destination_height == 0
                || ack.export_hash != export.digest().map_err(|error| error.to_string())?
            {
                return Err("game ACK differs from its original export".into());
            }
        }
        Ok(())
    }
}
