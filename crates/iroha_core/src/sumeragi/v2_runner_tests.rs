//! Integration-heavy unit cases for the Sumeragi v2 runner.

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    sync::{Mutex, atomic::AtomicUsize},
};

use iroha_config::parameters::actual::{NodeRole, SumeragiV2KeyPolicy, SumeragiV2Limits};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::decode_framed_signed_block,
    isi::Log,
    peer::PeerId,
    transaction::{TransactionBuilder, signed::TransactionResultInner},
    trigger::DataTriggerSequence,
};
use iroha_logger::Level;
use iroha_p2p::network::{
    NetworkActorAdmissionError, NetworkReplyFlushAckTestFixture, NetworkReplyRouteTestFixture,
    NetworkReplyRoutes,
};
use tempfile::TempDir;

use super::super::FairV2IngressPushError;
use super::*;
use crate::{
    NetworkMessage,
    merge_sidecar::{
        CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarChunkV1,
        CertifiedMergeSidecarCloseV1, CertifiedMergeSidecarMessage,
        CertifiedMergeSidecarSemanticSequenceV1, CertifiedMergeSidecarServiceGenerationV1,
        CertifiedMergeSidecarStreamEpochV1,
    },
    sumeragi::LaneRelayMessage,
};

include!("tests/v2_runner_unsealed_00.rs");
include!("tests/v2_runner_unsealed_01.rs");
include!("tests/v2_runner_unsealed_02.rs");
