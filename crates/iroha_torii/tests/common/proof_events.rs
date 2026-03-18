//! Proof event fixtures shared across Torii tests to keep payloads consistent.
#![allow(dead_code)]

use iroha_data_model::{
    events::{
        EventBox, SharedDataEvent,
        data::proof::{ProofEvent, ProofRejected, ProofVerified},
    },
    prelude::DataEvent,
    proof::{ProofId, VerifyingKeyId},
};

/// Builder for deterministic proof events used in SSE/WS tests.
#[derive(Clone)]
#[allow(dead_code)]
pub struct ProofEventFixture {
    backend: String,
    proof_hash: [u8; 32],
    vk_name: Option<String>,
    vk_commitment: Option<[u8; 32]>,
    call_hash: Option<[u8; 32]>,
    envelope_hash: Option<[u8; 32]>,
}

#[allow(dead_code)]
impl ProofEventFixture {
    /// Start a fixture with the given backend and proof hash.
    pub fn new(backend: impl Into<String>, proof_hash: [u8; 32]) -> Self {
        Self {
            backend: backend.into(),
            proof_hash,
            vk_name: Some("vk".to_string()),
            vk_commitment: Some([0x55; 32]),
            call_hash: Some([0xAA; 32]),
            envelope_hash: None,
        }
    }

    /// Override the verifying key reference and commitment.
    pub fn with_vk(mut self, name: impl Into<String>, commitment: [u8; 32]) -> Self {
        self.vk_name = Some(name.into());
        self.vk_commitment = Some(commitment);
        self
    }

    /// Drop the verifying key reference/commitment from the payload.
    pub fn without_vk(mut self) -> Self {
        self.vk_name = None;
        self.vk_commitment = None;
        self
    }

    /// Override the call hash (or clear it).
    pub fn with_call_hash(mut self, call_hash: Option<[u8; 32]>) -> Self {
        self.call_hash = call_hash;
        self
    }

    /// Override the envelope hash (or clear it).
    pub fn with_envelope_hash(mut self, envelope_hash: Option<[u8; 32]>) -> Self {
        self.envelope_hash = envelope_hash;
        self
    }

    /// Build a `ProofVerified` event box.
    pub fn verified(self) -> EventBox {
        let event = ProofEvent::Verified(ProofVerified {
            id: self.id(),
            vk_ref: self.vk_ref(),
            vk_commitment: self.vk_commitment,
            call_hash: self.call_hash,
            envelope_hash: self.envelope_hash,
        });
        Self::wrap(event)
    }

    /// Build a `ProofRejected` event box.
    pub fn rejected(self) -> EventBox {
        let event = ProofEvent::Rejected(ProofRejected {
            id: self.id(),
            vk_ref: self.vk_ref(),
            vk_commitment: self.vk_commitment,
            call_hash: self.call_hash,
            envelope_hash: self.envelope_hash,
        });
        Self::wrap(event)
    }

    fn id(&self) -> ProofId {
        ProofId {
            backend: self.backend.clone(),
            proof_hash: self.proof_hash,
        }
    }

    fn vk_ref(&self) -> Option<VerifyingKeyId> {
        self.vk_name
            .as_ref()
            .map(|name| VerifyingKeyId::new(&self.backend, name))
    }

    fn wrap(event: ProofEvent) -> EventBox {
        EventBox::Data(SharedDataEvent::from(DataEvent::Proof(event)))
    }
}
