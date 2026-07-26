#![allow(dead_code)]
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Hash32([u8; 32]);

impl Hash32 {
    const ZERO: Self = Self([0u8; 32]);

    fn repeat(byte: u8) -> Self {
        Self([byte; 32])
    }
}

#[derive(Clone, Debug)]
struct StreamingCapabilityTicket {
    ticket_id: Hash32,
    owner: String,
    dsid: String,
    lane_id: u8,
    settlement_bucket: u64,
    start_slot: u64,
    expire_slot: u64,
    prepaid_teu: u128,
    chunk_teu: u32,
    fanout_quota: u16,
    key_commitment: Hash32,
    nonce: u64,
}

#[derive(Clone, Debug)]
struct StreamingBucketReceipt {
    bucket_id: u64,
    dsid: String,
    lane_id: u8,
    ticket_id: Hash32,
    delivered_chunks: u32,
    delivered_teu: u128,
    mac_root: Hash32,
    relay_set_root: Hash32,
    signed_at_slot: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StreamingChunkMac {
    chunk_id: u64,
    seq_slot: u64,
    payload_hash: Hash32,
    mac: Hash32,
}

#[derive(Clone, Debug)]
struct StreamingShortfallProof {
    ticket: StreamingCapabilityTicket,
    receipt: StreamingBucketReceipt,
    challenged_mac: StreamingChunkMac,
    client_mac: StreamingChunkMac,
    transcript_commitment: Hash32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TicketLifecycle {
    Active {
        remaining_teu: u128,
        next_bucket: u64,
        last_mac_root: Option<Hash32>,
    },
    Exhausted,
    Expired,
    Refunded {
        refunded_teu: u128,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StreamingEvent {
    TicketMinted {
        ticket_id: Hash32,
        owner: String,
        prepaid_teu: u128,
    },
    BucketDebited {
        ticket_id: Hash32,
        bucket_id: u64,
        delivered_teu: u128,
    },
    TicketExhausted {
        ticket_id: Hash32,
    },
    TicketExpired {
        ticket_id: Hash32,
    },
    TicketRefunded {
        ticket_id: Hash32,
        bucket_id: u64,
        refunded_teu: u128,
    },
}

#[derive(Debug)]
struct StreamingAccessContract {
    tickets: HashMap<Hash32, (StreamingCapabilityTicket, TicketLifecycle)>,
    owner_index: HashSet<(String, String, u64)>,
    events: Vec<StreamingEvent>,
}

#[derive(Debug, PartialEq, Eq)]
enum ContractError {
    DuplicateActive,
    TicketNotFound,
    TicketNotActive,
    BucketMismatch,
    TeuMismatch,
    InsufficientBudget,
    Finalised,
    NotYetExpired,
    MacMismatch,
}

impl core::fmt::Display for ContractError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            ContractError::DuplicateActive => "ticket already active for owner and bucket",
            ContractError::TicketNotFound => "ticket not found",
            ContractError::TicketNotActive => "ticket not active",
            ContractError::BucketMismatch => "bucket mismatch",
            ContractError::TeuMismatch => "delivered teu mismatch",
            ContractError::InsufficientBudget => "insufficient prepaid teu",
            ContractError::Finalised => "ticket already finalised",
            ContractError::NotYetExpired => "expiry window not reached",
            ContractError::MacMismatch => "mac mismatch",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for ContractError {}

impl StreamingAccessContract {
    fn new() -> Self {
        Self {
            tickets: HashMap::new(),
            owner_index: HashSet::new(),
            events: Vec::new(),
        }
    }

    fn events(&self) -> &[StreamingEvent] {
        &self.events
    }

    fn mint(&mut self, ticket: StreamingCapabilityTicket) -> Result<(), ContractError> {
        let key = (
            ticket.owner.clone(),
            ticket.dsid.clone(),
            ticket.settlement_bucket,
        );
        if self.owner_index.contains(&key) {
            return Err(ContractError::DuplicateActive);
        }
        self.owner_index.insert(key);
        let state = TicketLifecycle::Active {
            remaining_teu: ticket.prepaid_teu,
            next_bucket: ticket.settlement_bucket,
            last_mac_root: None,
        };
        let ticket_id = ticket.ticket_id.clone();
        self.events.push(StreamingEvent::TicketMinted {
            ticket_id: ticket_id.clone(),
            owner: ticket.owner.clone(),
            prepaid_teu: ticket.prepaid_teu,
        });
        self.tickets.insert(ticket_id, (ticket, state));
        Ok(())
    }

    fn debit_bucket(
        &mut self,
        receipt: StreamingBucketReceipt,
        chunk_teu: u32,
    ) -> Result<(), ContractError> {
        let entry = self
            .tickets
            .get_mut(&receipt.ticket_id)
            .ok_or(ContractError::TicketNotFound)?;

        let ticket = &entry.0;
        let state = &mut entry.1;
        let TicketLifecycle::Active {
            remaining_teu,
            next_bucket,
            last_mac_root,
        } = state
        else {
            return Err(ContractError::TicketNotActive);
        };

        if receipt.bucket_id != *next_bucket {
            return Err(ContractError::BucketMismatch);
        }

        let expected_teu = u128::from(receipt.delivered_chunks) * u128::from(chunk_teu);
        if receipt.delivered_teu != expected_teu {
            return Err(ContractError::TeuMismatch);
        }

        if *remaining_teu < expected_teu {
            return Err(ContractError::InsufficientBudget);
        }

        *remaining_teu -= expected_teu;
        *next_bucket += 1;
        *last_mac_root = Some(receipt.mac_root.clone());

        self.events.push(StreamingEvent::BucketDebited {
            ticket_id: ticket.ticket_id.clone(),
            bucket_id: receipt.bucket_id,
            delivered_teu: receipt.delivered_teu,
        });

        if *remaining_teu == 0 {
            *state = TicketLifecycle::Exhausted;
            self.events.push(StreamingEvent::TicketExhausted {
                ticket_id: ticket.ticket_id.clone(),
            });
        }

        Ok(())
    }

    fn expire(&mut self, ticket_id: &Hash32, current_slot: u64) -> Result<(), ContractError> {
        let entry = self
            .tickets
            .get_mut(ticket_id)
            .ok_or(ContractError::TicketNotFound)?;
        let ticket = &entry.0;
        if current_slot <= ticket.expire_slot {
            return Err(ContractError::NotYetExpired);
        }
        match entry.1 {
            TicketLifecycle::Active { .. } | TicketLifecycle::Exhausted => {
                entry.1 = TicketLifecycle::Expired;
                self.events.push(StreamingEvent::TicketExpired {
                    ticket_id: ticket_id.clone(),
                });
                self.owner_index.remove(&(
                    ticket.owner.clone(),
                    ticket.dsid.clone(),
                    ticket.settlement_bucket,
                ));
                Ok(())
            }
            TicketLifecycle::Expired | TicketLifecycle::Refunded { .. } => {
                Err(ContractError::Finalised)
            }
        }
    }

    fn refund_unserved(
        &mut self,
        proof: StreamingShortfallProof,
        chunk_teu: u32,
    ) -> Result<(), ContractError> {
        let entry = self
            .tickets
            .get_mut(&proof.ticket.ticket_id)
            .ok_or(ContractError::TicketNotFound)?;
        let ticket = &entry.0;
        match entry.1 {
            TicketLifecycle::Active { .. } | TicketLifecycle::Exhausted => {
                if proof.receipt.ticket_id != ticket.ticket_id {
                    return Err(ContractError::TicketNotFound);
                }
                if proof.challenged_mac != proof.client_mac {
                    return Err(ContractError::MacMismatch);
                }
                let refunded_teu = u128::from(chunk_teu);
                entry.1 = TicketLifecycle::Refunded { refunded_teu };
                self.events.push(StreamingEvent::TicketRefunded {
                    ticket_id: ticket.ticket_id.clone(),
                    bucket_id: proof.receipt.bucket_id,
                    refunded_teu,
                });
                self.owner_index.remove(&(
                    ticket.owner.clone(),
                    ticket.dsid.clone(),
                    ticket.settlement_bucket,
                ));
                Ok(())
            }
            TicketLifecycle::Expired | TicketLifecycle::Refunded { .. } => {
                Err(ContractError::Finalised)
            }
        }
    }
}

#[test]
fn mint_debit_exhaustes_ticket() {
    let mut contract = StreamingAccessContract::new();
    let ticket = StreamingCapabilityTicket {
        ticket_id: Hash32::repeat(1),
        owner: "account::alice@stream".to_owned(),
        dsid: "ds::stream::video".to_owned(),
        lane_id: 0,
        settlement_bucket: 10,
        start_slot: 640,
        expire_slot: 768,
        prepaid_teu: 100,
        chunk_teu: 50,
        fanout_quota: 4,
        key_commitment: Hash32::repeat(2),
        nonce: 0,
    };

    contract.mint(ticket.clone()).expect("mint");

    let receipt = StreamingBucketReceipt {
        bucket_id: 10,
        dsid: ticket.dsid.clone(),
        lane_id: ticket.lane_id,
        ticket_id: ticket.ticket_id.clone(),
        delivered_chunks: 1,
        delivered_teu: 50,
        mac_root: Hash32::repeat(3),
        relay_set_root: Hash32::repeat(4),
        signed_at_slot: 641,
    };

    contract
        .debit_bucket(receipt.clone(), ticket.chunk_teu)
        .expect("debit bucket");

    // Debit remaining balance to zero.
    let receipt_last = StreamingBucketReceipt {
        bucket_id: 11,
        mac_root: Hash32::repeat(5),
        signed_at_slot: 642,
        ..receipt
    };
    contract
        .debit_bucket(receipt_last, ticket.chunk_teu)
        .expect("debit bucket");

    assert_eq!(
        contract.events(),
        &[
            StreamingEvent::TicketMinted {
                ticket_id: ticket.ticket_id.clone(),
                owner: ticket.owner.clone(),
                prepaid_teu: ticket.prepaid_teu,
            },
            StreamingEvent::BucketDebited {
                ticket_id: ticket.ticket_id.clone(),
                bucket_id: 10,
                delivered_teu: 50,
            },
            StreamingEvent::BucketDebited {
                ticket_id: ticket.ticket_id.clone(),
                bucket_id: 11,
                delivered_teu: 50,
            },
            StreamingEvent::TicketExhausted {
                ticket_id: ticket.ticket_id.clone()
            },
        ],
    );
}

#[test]
fn refund_shortfall_transitions_ticket() {
    let mut contract = StreamingAccessContract::new();
    let ticket = StreamingCapabilityTicket {
        ticket_id: Hash32::repeat(9),
        owner: "account::bob@stream".to_owned(),
        dsid: "ds::stream::audio".to_owned(),
        lane_id: 1,
        settlement_bucket: 5,
        start_slot: 320,
        expire_slot: 512,
        prepaid_teu: 100,
        chunk_teu: 25,
        fanout_quota: 2,
        key_commitment: Hash32::repeat(7),
        nonce: 1,
    };

    contract.mint(ticket.clone()).expect("mint");

    let receipt = StreamingBucketReceipt {
        bucket_id: 5,
        dsid: ticket.dsid.clone(),
        lane_id: ticket.lane_id,
        ticket_id: ticket.ticket_id.clone(),
        delivered_chunks: 2,
        delivered_teu: 50,
        mac_root: Hash32::repeat(8),
        relay_set_root: Hash32::repeat(6),
        signed_at_slot: 321,
    };

    contract
        .debit_bucket(receipt.clone(), ticket.chunk_teu)
        .expect("debit bucket");

    let mac = StreamingChunkMac {
        chunk_id: 9,
        seq_slot: 322,
        payload_hash: Hash32::repeat(10),
        mac: Hash32::repeat(11),
    };

    let dispute = StreamingShortfallProof {
        ticket: ticket.clone(),
        receipt: receipt.clone(),
        challenged_mac: mac.clone(),
        client_mac: mac,
        transcript_commitment: Hash32::repeat(12),
    };

    contract
        .refund_unserved(dispute, ticket.chunk_teu)
        .expect("refund");

    assert!(
        matches!(contract.events().last(), Some(StreamingEvent::TicketRefunded { ticket_id, bucket_id, refunded_teu }) if *ticket_id == ticket.ticket_id && *bucket_id == 5 && *refunded_teu == 25)
    );
}

#[test]
fn expire_transitions_ticket_to_expired_state() {
    let mut contract = StreamingAccessContract::new();
    let ticket = StreamingCapabilityTicket {
        ticket_id: Hash32::repeat(7),
        owner: "account::carol@stream".to_owned(),
        dsid: "ds::stream::mixed".to_owned(),
        lane_id: 2,
        settlement_bucket: 12,
        start_slot: 256,
        expire_slot: 384,
        prepaid_teu: 150,
        chunk_teu: 30,
        fanout_quota: 3,
        key_commitment: Hash32::repeat(4),
        nonce: 5,
    };

    contract.mint(ticket.clone()).expect("mint ticket");

    let early_err = contract
        .expire(&ticket.ticket_id, ticket.expire_slot)
        .expect_err("ticket must not expire before expiry slot");
    assert!(matches!(early_err, ContractError::NotYetExpired));

    contract
        .expire(&ticket.ticket_id, ticket.expire_slot + 1)
        .expect("expire after slot boundary");

    let entry = contract
        .tickets
        .get(&ticket.ticket_id)
        .expect("ticket entry present after expiry");
    assert!(
        matches!(entry.1, TicketLifecycle::Expired),
        "ticket lifecycle should transition to Expired"
    );

    assert!(
        matches!(contract.events().last(), Some(StreamingEvent::TicketExpired { ticket_id }) if *ticket_id == ticket.ticket_id),
        "last event should record ticket expiry"
    );

    let final_err = contract
        .expire(&ticket.ticket_id, ticket.expire_slot + 2)
        .expect_err("ticket must not expire twice");
    assert!(matches!(final_err, ContractError::Finalised));
}
