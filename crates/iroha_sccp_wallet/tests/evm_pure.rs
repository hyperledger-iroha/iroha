//! Pure EVM wallet path against the shared SCCP v1 fixtures (`specs/sccp.md` §5.1–§5.2, §7.1).
//!
//! The coherent scenario of `fixtures/sccp/evm_calldata_v1.json` (rosters g7 and g8, the
//! attestations of heights 100, 150 (rotation) and 200, their signature sets, block and history
//! trees) is rebuilt into the Torii proof bundles of `iroha_sccp::api`. The tests prove that:
//!
//! - good direct and historical message bundles, control bundles and the rotation chain verify
//!   against the destination state, and the calldata the wallet builds from them is byte for
//!   byte the golden calldata that the EDR suite runs against `SccpTairaXor.sol`;
//! - every tampering fails: roster members, threshold and order, the signer bitmap, a
//!   signature, the Merkle path, the payload, the deadline, the Taira network id, an expired
//!   roster and an expired previous-roster grace;
//! - rotation chains that break a §5.1.5 bound fail, and long chains split into batches of 16;
//! - a control bundle of another revision or network fails;
//! - EIP-1559 transactions match independent `ethers` encodings and recover their signer.

use std::path::PathBuf;

use iroha_data_model::{bridge::SccpNetworkV1, sccp::attestation::SccpAttestationStatementV1};
use iroha_sccp::{
    api::{
        SccpControlProofBundleV1, SccpHistoryProofV1, SccpMessageProofBundleV1, SccpRosterViewV1,
        SccpRotationChainV1, SccpRotationStepV1, SccpSignatureSetV1,
    },
    v1::{
        constants::SECP256K1_N,
        eip712::AttestationFieldsV1,
        evm_abi::TransferToTairaCallV1,
        hashes::{control_leaf, keccak256, transfer_leaf},
        history::history_path,
        merkle::PromoteOddTree,
        proof::{DestinationV1, ProofError},
        roster::{RosterError, RosterStateV1, RosterV1, RotationError},
        signature::{SignatureError, SignatureSetV1, address_of_secret, sign_digest},
    },
};
use iroha_sccp_wallet::pure::{
    bundle::{
        AcceptedRosterV1, BundleError, BundlePurposeV1, DestinationContextV1, verify_message_bundle,
    },
    control::{
        ControlBundleError, DestinationControlStateV1, PreFinalizeActionV1, TairaControlV1,
        plan_before_finalize, verify_control_bundle,
    },
    evm::{
        Eip1559TransactionV1, EvmError, EvmSigningKey, SignedEip1559V1, apply_control_calldata,
        finalize_calldata, rotate_rosters_calldata, transfer_to_taira_calldata,
        void_expired_calldata, void_frozen_calldata,
    },
    rotation::{RotationChainError, verify_rotation_chain, verify_rotation_steps},
};
use norito::json::Value;

const T0: u64 = 1_800_000_000_000;
const DAY: u64 = 86_400_000;

// ---------------------------------------------------------------------------------------------
// Fixture loading
// ---------------------------------------------------------------------------------------------

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/sccp")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    norito::json::from_str(&text).expect("fixture JSON")
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("fixture field `{key}` is missing"))
}

fn bytes(value: &Value) -> Vec<u8> {
    let text = value.as_str().expect("hex string");
    hex::decode(text.strip_prefix("0x").expect("0x prefix")).expect("hex")
}

fn fixed<const N: usize>(value: &Value) -> [u8; N] {
    bytes(value).try_into().expect("fixed length")
}

fn number(value: &Value) -> u64 {
    value.as_u64().expect("integer")
}

fn roster(value: &Value) -> RosterV1 {
    let roster = RosterV1 {
        generation: number(field(value, "generation")),
        valid_from_ms: number(field(value, "valid_from_ms")),
        valid_until_ms: number(field(value, "valid_until_ms")),
        members: field(value, "members")
            .as_array()
            .expect("members")
            .iter()
            .map(fixed::<20>)
            .collect(),
    };
    assert_eq!(
        roster.digest(&[0x11; 32]).expect("valid roster"),
        fixed::<32>(field(value, "digest"))
    );
    roster
}

fn statement(value: &Value) -> SccpAttestationStatementV1 {
    SccpAttestationStatementV1 {
        height: number(field(value, "height")),
        epoch: number(field(value, "epoch")),
        timestamp_ms: number(field(value, "timestamp_ms")),
        block_hash: fixed(field(value, "block_hash")),
        sccp_root: fixed(field(value, "sccp_root")),
        message_count: u32::try_from(number(field(value, "message_count"))).expect("u32"),
        history_root: fixed(field(value, "history_root")),
        history_size: number(field(value, "history_size")),
        roster_digest: fixed(field(value, "roster_digest")),
        next_roster_digest: fixed(field(value, "next_roster_digest")),
    }
}

fn signatures(value: &Value) -> SccpSignatureSetV1 {
    let set = SignatureSetV1 {
        signer_bitmap: u32::try_from(number(field(value, "signer_bitmap"))).expect("u32"),
        signatures: bytes(field(value, "signatures")),
    };
    SccpSignatureSetV1::try_from(&set).expect("signature set view")
}

/// The rebuilt calldata scenario.
struct Scenario {
    taira: [u8; 32],
    destination: DestinationV1,
    deadline_ms: u64,
    g7: RosterV1,
    g8: RosterV1,
    a100: SccpAttestationStatementV1,
    a150: SccpAttestationStatementV1,
    a200: SccpAttestationStatementV1,
    s7_a100: SccpSignatureSetV1,
    s7_a150: SccpSignatureSetV1,
    s8_a200: SccpSignatureSetV1,
    payloads: Vec<Vec<u8>>,
    message_ids: Vec<[u8; 32]>,
    block100: PromoteOddTree,
    history: Vec<[u8; 32]>,
    calls: Value,
    secrets: Vec<[u8; 32]>,
}

fn scenario() -> Scenario {
    let calldata = fixture("evm_calldata_v1.json");
    let s = field(&calldata, "scenario");
    let taira = fixed::<32>(field(s, "taira_network_id"));
    assert_eq!(taira, [0x11; 32]);
    let destination_json = field(s, "destination");
    assert_eq!(
        field(destination_json, "target").as_str(),
        Some("ethereum-mainnet")
    );
    let destination = DestinationV1 {
        network: SccpNetworkV1::EthereumMainnet,
        route_revision: u32::try_from(number(field(destination_json, "route_revision")))
            .expect("u32"),
        destination_word: fixed(field(destination_json, "destination_word")),
    };
    let (payloads, message_ids): (Vec<Vec<u8>>, Vec<[u8; 32]>) = field(s, "payloads")
        .as_array()
        .expect("payloads")
        .iter()
        .map(|entry| {
            (
                bytes(field(entry, "payload")),
                fixed::<32>(field(entry, "message_id")),
            )
        })
        .unzip();
    let leaf = |index: usize| transfer_leaf(&message_ids[index], &destination.destination_word);
    let control = control_leaf(
        &taira,
        destination.network,
        &destination.destination_word,
        destination.route_revision,
        1,
        true,
    )
    .expect("control leaf");
    let block100 = PromoteOddTree::block(&[leaf(1), leaf(2), control]).expect("block 100");
    assert_eq!(block100.root(), fixed::<32>(field(s, "block_100_root")));
    let history: Vec<[u8; 32]> = field(s, "history_leaves")
        .as_array()
        .expect("history")
        .iter()
        .map(fixed::<32>)
        .collect();
    let eip712 = fixture("eip712_v1.json");
    let secrets = field(&eip712, "keys")
        .as_array()
        .expect("keys")
        .iter()
        .map(|key| fixed::<32>(field(key, "secret")))
        .collect();
    Scenario {
        taira,
        destination,
        deadline_ms: number(field(s, "deadline_ms")),
        g7: roster(field(s, "roster_g7")),
        g8: roster(field(s, "roster_g8")),
        a100: statement(field(s, "attestation_100")),
        a150: statement(field(s, "attestation_150_rotation")),
        a200: statement(field(s, "attestation_200")),
        s7_a100: signatures(field(s, "signatures_g7_100")),
        s7_a150: signatures(field(s, "signatures_g7_150")),
        s8_a200: signatures(field(s, "signatures_g8_200")),
        payloads,
        message_ids,
        block100,
        history,
        calls: field(&calldata, "calls").clone(),
        secrets,
    }
}

impl Scenario {
    fn call(&self, label: &str) -> Vec<u8> {
        let call = self
            .calls
            .as_array()
            .expect("calls")
            .iter()
            .find(|call| call.get("label").and_then(Value::as_str) == Some(label))
            .unwrap_or_else(|| panic!("golden call `{label}` is missing"));
        bytes(field(call, "calldata"))
    }

    fn digest(&self, statement: &SccpAttestationStatementV1) -> [u8; 32] {
        AttestationFieldsV1::from(*statement).digest(&self.taira)
    }

    fn view(&self, roster: &RosterV1) -> SccpRosterViewV1 {
        SccpRosterViewV1::from_roster(roster, &self.taira).expect("roster view")
    }

    fn history_proof(&self) -> SccpHistoryProofV1 {
        SccpHistoryProofV1 {
            height: 100,
            sccp_root: self.block100.root(),
            message_count: 3,
            leaf_index: 1,
            path: history_path(&self.history, 1).expect("history path"),
        }
    }

    /// Direct bundle of the nonce-1 transfer (leaf 0 of block 100, attested by g7).
    fn direct_bundle(&self) -> SccpMessageProofBundleV1 {
        SccpMessageProofBundleV1 {
            message_id: self.message_ids[1],
            payload: self.payloads[1].clone(),
            deadline_ms: self.deadline_ms,
            leaf_index: 0,
            message_count: 3,
            path: self.block100.path(0).expect("path"),
            statement: self.a100,
            digest: self.digest(&self.a100),
            roster: self.view(&self.g7),
            signatures: self.s7_a100.clone(),
            history: None,
        }
    }

    /// Historical bundle of the nonce-2 transfer (leaf 1 of block 100 through the history
    /// root attested at height 200 by g8).
    fn historical_bundle(&self) -> SccpMessageProofBundleV1 {
        SccpMessageProofBundleV1 {
            message_id: self.message_ids[2],
            payload: self.payloads[2].clone(),
            deadline_ms: self.deadline_ms,
            leaf_index: 1,
            message_count: 3,
            path: self.block100.path(1).expect("path"),
            statement: self.a200,
            digest: self.digest(&self.a200),
            roster: self.view(&self.g8),
            signatures: self.s8_a200.clone(),
            history: Some(self.history_proof()),
        }
    }

    fn control_bundle(&self, historical: bool) -> SccpControlProofBundleV1 {
        let (statement, roster, signatures, history) = if historical {
            (
                self.a200,
                &self.g8,
                self.s8_a200.clone(),
                Some(self.history_proof()),
            )
        } else {
            (self.a100, &self.g7, self.s7_a100.clone(), None)
        };
        SccpControlProofBundleV1 {
            network: self.destination.network,
            revision: self.destination.route_revision,
            control_nonce: 1,
            paused: true,
            leaf_index: 2,
            message_count: 3,
            path: self.block100.path(2).expect("path"),
            statement,
            digest: self.digest(&statement),
            roster: self.view(roster),
            signatures,
            history,
        }
    }

    fn rotation_chain(&self) -> SccpRotationChainV1 {
        SccpRotationChainV1 {
            steps: vec![SccpRotationStepV1 {
                statement: self.a150,
                digest: self.digest(&self.a150),
                signatures: self.s7_a150.clone(),
                current_roster: self.view(&self.g7),
                next_roster: self.view(&self.g8),
            }],
            first_unattested_handoff: None,
        }
    }

    /// The destination deployed with g7 at `T0 + 100 s`, observed at `now_ms`.
    fn g7_context(&self, now_ms: u64) -> DestinationContextV1 {
        DestinationContextV1 {
            taira_network_id: self.taira,
            destination: self.destination,
            roster_state: RosterStateV1::initial(&self.g7, &self.taira, T0 + 100_000)
                .expect("initial state"),
            now_ms,
        }
    }

    /// The destination after rotating to g8 at `T0 + 200 s`, observed at `now_ms`.
    fn rotated_context(&self, now_ms: u64) -> DestinationContextV1 {
        let mut state =
            RosterStateV1::initial(&self.g7, &self.taira, T0 + 100_000).expect("initial state");
        state
            .rotate(
                &AttestationFieldsV1::from(self.a150),
                &self.g7,
                &self.g8,
                &self.taira,
                T0 + 200_000,
            )
            .expect("rotation");
        DestinationContextV1 {
            roster_state: state,
            ..self.g7_context(now_ms)
        }
    }

    /// Sign `digest` with the fixture keys `keys` and address them in `roster`.
    fn sign(&self, roster: &RosterV1, keys: &[usize], digest: &[u8; 32]) -> SccpSignatureSetV1 {
        let entries: Vec<(usize, [u8; 65])> = keys
            .iter()
            .map(|key| {
                let secret = self.secrets[*key];
                let address = address_of_secret(&secret).expect("secret");
                let index = roster
                    .members
                    .iter()
                    .position(|member| *member == address)
                    .expect("signer is a member");
                (index, sign_digest(&secret, digest).expect("signature"))
            })
            .collect();
        let set = SignatureSetV1::from_signers(roster.n(), &entries).expect("set");
        SccpSignatureSetV1::try_from(&set).expect("view")
    }

    /// A rotation step from `current` to `next` signed by the fixture keys `keys` of `current`.
    fn rotation_step(
        &self,
        current: &RosterV1,
        next: &RosterV1,
        keys: &[usize],
    ) -> SccpRotationStepV1 {
        let statement = SccpAttestationStatementV1 {
            height: 1_000 + next.generation,
            epoch: next.generation,
            timestamp_ms: next.valid_from_ms,
            block_hash: keccak256(&[b"rotation", &next.generation.to_be_bytes()]),
            sccp_root: [0; 32],
            message_count: 0,
            history_root: self.a150.history_root,
            history_size: self.a150.history_size,
            roster_digest: current.digest(&self.taira).expect("current digest"),
            next_roster_digest: next.digest(&self.taira).unwrap_or([0x77; 32]),
        };
        let digest = self.digest(&statement);
        SccpRotationStepV1 {
            statement,
            digest,
            signatures: self.sign(current, keys, &digest),
            current_roster: self.view(current),
            next_roster: SccpRosterViewV1 {
                generation: next.generation,
                valid_from_ms: next.valid_from_ms,
                valid_until_ms: next.valid_until_ms,
                threshold: u8::try_from(next.threshold()).expect("u8"),
                members: next.members.clone(),
                digest: next.digest(&self.taira).unwrap_or([0x77; 32]),
            },
        }
    }

    /// A roster of fixture keys `keys` (sorted) with the given generation and validity.
    fn key_roster(&self, keys: &[usize], generation: u64, from: u64, until: u64) -> RosterV1 {
        let mut members: Vec<[u8; 20]> = keys
            .iter()
            .map(|key| address_of_secret(&self.secrets[*key]).expect("secret"))
            .collect();
        members.sort_unstable();
        RosterV1 {
            generation,
            valid_from_ms: from,
            valid_until_ms: until,
            members,
        }
    }
}

fn high_s(signature: &[u8; 65]) -> [u8; 65] {
    let mut out = *signature;
    let mut borrow = 0_i16;
    for index in (0..32).rev() {
        let value = i16::from(SECP256K1_N[index]) - i16::from(signature[32 + index]) - borrow;
        borrow = i16::from(value < 0);
        out[32 + index] = u8::try_from(value.rem_euclid(256)).expect("byte");
    }
    out[64] = if signature[64] == 27 { 28 } else { 27 };
    out
}

// ---------------------------------------------------------------------------------------------
// Good bundles produce the golden calldata
// ---------------------------------------------------------------------------------------------

#[test]
fn direct_finalize_bundle_verifies_and_matches_the_golden_calldata() {
    let s = scenario();
    let context = s.g7_context(T0 + 200_000);
    let verified = verify_message_bundle(&s.direct_bundle(), &context, BundlePurposeV1::Finalize)
        .expect("direct bundle verifies");
    assert_eq!(verified.attested.accepted_as, AcceptedRosterV1::Current);
    assert_eq!(verified.nonce(), 1);
    assert_eq!(verified.transfer.message_id, s.message_ids[1]);
    assert_eq!(verified.attested.signed.signatures.popcount(), 3);
    assert_eq!(
        finalize_calldata(&verified).expect("calldata"),
        s.call("finalize_direct")
    );
    assert_eq!(
        void_expired_calldata(&verified),
        Err(EvmError::WrongPurpose)
    );
}

#[test]
fn historical_finalize_bundle_verifies_and_matches_the_golden_calldata() {
    let s = scenario();
    let context = s.rotated_context(T0 + 300_000);
    let verified =
        verify_message_bundle(&s.historical_bundle(), &context, BundlePurposeV1::Finalize)
            .expect("historical bundle verifies");
    assert!(verified.is_historical());
    assert_eq!(verified.nonce(), 2);
    assert_eq!(
        finalize_calldata(&verified).expect("calldata"),
        s.call("finalize_historical")
    );
}

#[test]
fn void_bundles_match_the_golden_calldata_after_the_deadline() {
    let s = scenario();
    let after = s.deadline_ms + 1;
    let direct = verify_message_bundle(
        &s.direct_bundle(),
        &s.g7_context(after),
        BundlePurposeV1::VoidExpired,
    )
    .expect("void verifies");
    assert_eq!(
        void_expired_calldata(&direct).expect("calldata"),
        s.call("void_expired")
    );
    assert_eq!(finalize_calldata(&direct), Err(EvmError::WrongPurpose));
    let historical = verify_message_bundle(
        &s.historical_bundle(),
        &s.rotated_context(after),
        BundlePurposeV1::VoidExpired,
    )
    .expect("historical void verifies");
    assert_eq!(
        void_expired_calldata(&historical).expect("calldata"),
        s.call("void_expired_historical")
    );
    assert_eq!(
        verify_message_bundle(
            &s.direct_bundle(),
            &s.g7_context(s.deadline_ms),
            BundlePurposeV1::VoidExpired
        ),
        Err(BundleError::DeadlineNotReached)
    );
    assert_eq!(
        void_frozen_calldata(4, 3).expect("calldata"),
        s.call("void_frozen")
    );
}

#[test]
fn transfer_to_taira_calldata_matches_the_golden_calldata() {
    let s = scenario();
    let golden = s.call("transfer_to_taira");
    let call = TransferToTairaCallV1::decode(&golden).expect("canonical golden");
    assert_eq!(
        transfer_to_taira_calldata(
            &call.taira_recipient,
            call.token_amount,
            call.expected_nonce
        )
        .expect("calldata"),
        golden
    );
}

#[test]
fn rotation_chain_verifies_and_matches_the_golden_calldata() {
    let s = scenario();
    let context = s.g7_context(T0 + 200_000);
    let plan = verify_rotation_chain(
        &s.rotation_chain(),
        &context.roster_state,
        8,
        &s.taira,
        context.now_ms,
    )
    .expect("rotation verifies");
    assert_eq!(plan.final_state, s.rotated_context(0).roster_state);
    let calls = rotate_rosters_calldata(&plan);
    assert_eq!(calls, vec![s.call("rotate_one")]);
    // With the destination rotated, the g8 bundle is accepted as the current roster and the
    // g7 bundle as the previous one within its grace.
    let rotated = s.rotated_context(T0 + 300_000);
    let previous = verify_message_bundle(&s.direct_bundle(), &rotated, BundlePurposeV1::Finalize)
        .expect("previous roster within grace");
    assert_eq!(previous.attested.accepted_as, AcceptedRosterV1::Previous);
}

#[test]
fn control_bundles_verify_and_match_the_golden_calldata() {
    let s = scenario();
    let applied = DestinationControlStateV1::default();
    let direct = verify_control_bundle(
        &s.control_bundle(false),
        &s.g7_context(T0 + 200_000),
        &applied,
    )
    .expect("direct control verifies");
    assert_eq!(
        apply_control_calldata(&direct),
        s.call("apply_control_direct")
    );
    assert!(!direct.finalize_allowed_after_apply());
    let historical = verify_control_bundle(
        &s.control_bundle(true),
        &s.rotated_context(T0 + 300_000),
        &applied,
    )
    .expect("historical control verifies");
    assert_eq!(
        apply_control_calldata(&historical),
        s.call("apply_control_historical")
    );
    // §7.1 step 5: the attested pause is applied first and stops the finalization.
    assert_eq!(
        plan_before_finalize(
            &applied,
            Some(&TairaControlV1 {
                control_nonce: 1,
                paused: true,
                attested: true
            })
        ),
        PreFinalizeActionV1::ApplyControl {
            control_nonce: 1,
            paused: true
        }
    );
}

// ---------------------------------------------------------------------------------------------
// Tampering
// ---------------------------------------------------------------------------------------------

fn finalize(
    bundle: &SccpMessageProofBundleV1,
    context: &DestinationContextV1,
) -> Result<(), BundleError> {
    verify_message_bundle(bundle, context, BundlePurposeV1::Finalize).map(|_| ())
}

#[test]
fn tampered_rosters_fail() {
    let s = scenario();
    let context = s.g7_context(T0 + 200_000);
    let good = s.direct_bundle();

    let mut swapped = good.clone();
    swapped.roster.members[3] = [0xee; 20];
    assert_eq!(
        finalize(&swapped, &context),
        Err(BundleError::RosterDigestMismatch)
    );
    let mut swapped_with_digest = swapped.clone();
    let forged = swapped_with_digest.roster.to_roster().expect("ordered");
    swapped_with_digest.roster.digest = forged.digest(&s.taira).expect("digest");
    assert_eq!(
        finalize(&swapped_with_digest, &context),
        Err(BundleError::StatementRosterMismatch)
    );
    let mut threshold = good.clone();
    threshold.roster.threshold = 2;
    assert_eq!(
        finalize(&threshold, &context),
        Err(BundleError::Roster(RosterError::BadThreshold))
    );
    let mut unordered = good.clone();
    unordered.roster.members.swap(1, 2);
    assert_eq!(
        finalize(&unordered, &context),
        Err(BundleError::Roster(RosterError::BadOrder))
    );
    let mut shrunk = good;
    shrunk.roster.members.pop();
    assert_eq!(
        finalize(&shrunk, &context),
        Err(BundleError::Roster(RosterError::BadSize))
    );
}

#[test]
fn tampered_bitmaps_and_signatures_fail() {
    let s = scenario();
    let context = s.g7_context(T0 + 200_000);
    let good = s.direct_bundle();
    assert_eq!(
        good.signatures.signer_bitmap, 0b1110,
        "g7 slot 0 is keyless"
    );

    let mut zero_slot = good.clone();
    zero_slot.signatures.signer_bitmap |= 1;
    let first = zero_slot.signatures.signatures[0];
    zero_slot.signatures.signatures.insert(0, first);
    assert_eq!(
        finalize(&zero_slot, &context),
        Err(BundleError::Signatures(SignatureError::ZeroMember))
    );
    let mut out_of_range = good.clone();
    out_of_range.signatures.signer_bitmap |= 1 << 4;
    out_of_range.signatures.signatures.push(first);
    assert_eq!(
        finalize(&out_of_range, &context),
        Err(BundleError::Signatures(SignatureError::BitmapOutOfRange))
    );
    let mut count_mismatch = good.clone();
    count_mismatch.signatures.signer_bitmap = 0b0110;
    assert!(matches!(
        finalize(&count_mismatch, &context),
        Err(BundleError::Shape(_))
    ));
    let mut too_few = good.clone();
    too_few.signatures.signer_bitmap = 0b0110;
    too_few.signatures.signatures.pop();
    assert_eq!(
        finalize(&too_few, &context),
        Err(BundleError::TooFewSignatures {
            count: 2,
            threshold: 3
        })
    );
    let mut shifted = good.clone();
    shifted.signatures.signatures.swap(0, 1);
    assert!(matches!(
        finalize(&shifted, &context),
        Err(BundleError::Signatures(SignatureError::WrongSigner))
    ));
    let mut flipped = good.clone();
    flipped.signatures.signatures[2][5] ^= 0x40;
    assert!(matches!(
        finalize(&flipped, &context),
        Err(BundleError::Signatures(_))
    ));
    let mut malleated = good.clone();
    malleated.signatures.signatures[1] = high_s(&malleated.signatures.signatures[1]);
    assert_eq!(
        finalize(&malleated, &context),
        Err(BundleError::Signatures(SignatureError::BadS))
    );
    let mut bad_v = good;
    bad_v.signatures.signatures[0][64] = 29;
    assert_eq!(
        finalize(&bad_v, &context),
        Err(BundleError::Signatures(SignatureError::BadRecoveryByte))
    );
}

#[test]
fn tampered_paths_payloads_and_deadlines_fail() {
    let s = scenario();
    let context = s.g7_context(T0 + 200_000);
    let good = s.direct_bundle();

    let mut sibling = good.clone();
    sibling.path[0][31] ^= 1;
    assert_eq!(
        finalize(&sibling, &context),
        Err(BundleError::Proof(ProofError::BadBlockPath))
    );
    let mut longer = good.clone();
    longer.path.push([0; 32]);
    assert_eq!(
        finalize(&longer, &context),
        Err(BundleError::Proof(ProofError::BadBlockPath))
    );
    let mut index = good.clone();
    index.leaf_index = 1;
    assert_eq!(
        finalize(&index, &context),
        Err(BundleError::Proof(ProofError::BadBlockPath))
    );
    let mut count = good.clone();
    count.message_count = 4;
    assert_eq!(
        finalize(&count, &context),
        Err(BundleError::MessageCountMismatch)
    );
    // Another amount: a different message, not in the attested block.
    let mut amount = good.clone();
    let amount_offset = amount.payload.len() - 3 - 13 - 3 - 20 - 3 - 35 - 1;
    amount.payload[amount_offset] ^= 1;
    assert_eq!(
        finalize(&amount, &context),
        Err(BundleError::Proof(ProofError::BadBlockPath))
    );
    let mut trailing = good.clone();
    trailing.payload.push(0);
    assert_eq!(
        finalize(&trailing, &context),
        Err(BundleError::Proof(ProofError::BadPayload))
    );
    let mut bsc = good.clone();
    bsc.payload[9] = 2; // dest_domain = BSC
    assert_eq!(
        finalize(&bsc, &context),
        Err(BundleError::Proof(ProofError::BadPayload)),
        "the route id no longer matches the destination domain"
    );
    let mut message_id = good.clone();
    message_id.message_id = s.message_ids[2];
    assert_eq!(
        finalize(&message_id, &context),
        Err(BundleError::MessageIdMismatch)
    );
    let mut deadline = good.clone();
    deadline.deadline_ms += 1;
    assert_eq!(
        finalize(&deadline, &context),
        Err(BundleError::DeadlineMismatch)
    );
    assert_eq!(
        finalize(&good, &s.g7_context(s.deadline_ms + 1)),
        Err(BundleError::DeadlinePassed)
    );
    assert_eq!(finalize(&good, &s.g7_context(s.deadline_ms)), Ok(()));
}

#[test]
fn wrong_network_destination_and_expired_rosters_fail() {
    let s = scenario();
    let good = s.direct_bundle();
    let context = s.g7_context(T0 + 200_000);

    let other_taira = DestinationContextV1 {
        taira_network_id: [0x12; 32],
        ..context
    };
    assert_eq!(
        finalize(&good, &other_taira),
        Err(BundleError::DigestMismatch)
    );
    let mut statement = good.clone();
    statement.statement.height += 1;
    assert_eq!(
        finalize(&statement, &context),
        Err(BundleError::DigestMismatch)
    );
    let other_contract = DestinationContextV1 {
        destination: DestinationV1 {
            destination_word: [0x33; 32],
            ..s.destination
        },
        ..context
    };
    assert_eq!(
        finalize(&good, &other_contract),
        Err(BundleError::Proof(ProofError::BadBlockPath))
    );
    let other_revision = DestinationContextV1 {
        destination: DestinationV1 {
            route_revision: 2,
            ..s.destination
        },
        ..context
    };
    assert_eq!(
        finalize(&good, &other_revision),
        Err(BundleError::Proof(ProofError::WrongDestination))
    );
    let other_chain = DestinationContextV1 {
        destination: DestinationV1 {
            network: SccpNetworkV1::BscMainnet,
            ..s.destination
        },
        ..context
    };
    assert_eq!(
        finalize(&good, &other_chain),
        Err(BundleError::Proof(ProofError::WrongDestination))
    );
    // g7 expires at T0 + 14 d; voiding after that needs a newer roster.
    assert_eq!(
        verify_message_bundle(
            &good,
            &s.g7_context(s.g7.valid_until_ms + 1),
            BundlePurposeV1::VoidExpired
        )
        .map(|_| ()),
        Err(BundleError::RosterNotAccepted)
    );
    // After the rotation g7 is only accepted within the 24 h grace.
    let rotated_state = s.rotated_context(0).roster_state;
    assert_eq!(rotated_state.prev_valid_until_ms, T0 + 200_000 + DAY);
    let void = |now_ms: u64| {
        verify_message_bundle(
            &good,
            &s.rotated_context(now_ms),
            BundlePurposeV1::VoidExpired,
        )
        .map(|verified| verified.attested.accepted_as)
    };
    assert_eq!(void(T0 + 200_000 + DAY), Ok(AcceptedRosterV1::Previous));
    assert_eq!(
        void(T0 + 200_000 + DAY + 1),
        Err(BundleError::RosterNotAccepted)
    );
    // The g8 bundle is unknown to a destination that has not rotated.
    assert_eq!(
        finalize(&s.historical_bundle(), &s.g7_context(T0 + 300_000)),
        Err(BundleError::RosterNotAccepted)
    );
    let mut history = s.historical_bundle();
    history.history.as_mut().expect("history").path[0][0] ^= 1;
    assert_eq!(
        finalize(&history, &s.rotated_context(T0 + 300_000)),
        Err(BundleError::Proof(ProofError::BadHistoryPath))
    );
}

// ---------------------------------------------------------------------------------------------
// Rotation bounds
// ---------------------------------------------------------------------------------------------

/// Rotation of the g7 destination at `T0 + 200 s` and the fixture's genuine g7 → g8 step.
struct RotationCase {
    s: Scenario,
    state: RosterStateV1,
    now: u64,
    from: u64,
    until: u64,
    good: SccpRotationStepV1,
}

impl RotationCase {
    fn new() -> Self {
        let s = scenario();
        let context = s.g7_context(T0 + 200_000);
        let from = T0 + 150_000;
        let until = from + 14 * DAY;
        let good = s.rotation_step(
            &s.g7,
            &s.key_roster(&[0, 1, 2, 3], 8, from, until),
            &[0, 1, 2],
        );
        Self {
            state: context.roster_state,
            now: context.now_ms,
            from,
            until,
            good,
            s,
        }
    }

    fn rotate(&self, steps: &[SccpRotationStepV1], target: u64) -> Result<(), RotationChainError> {
        verify_rotation_steps(steps, None, &self.state, target, &self.s.taira, self.now).map(|_| ())
    }

    /// A g7-signed step to a roster of all four fixture keys with the given bounds.
    fn to(&self, generation: u64, from: u64, until: u64) -> SccpRotationStepV1 {
        self.s.rotation_step(
            &self.s.g7,
            &self.s.key_roster(&[0, 1, 2, 3], generation, from, until),
            &[0, 1, 2],
        )
    }
}

fn rotation_error(error: RotationError) -> Result<(), RotationChainError> {
    Err(RotationChainError::Rotation { index: 0, error })
}

#[test]
fn rotation_chains_with_next_roster_bound_violations_fail() {
    let case = RotationCase::new();
    let (from, until, now) = (case.from, case.until, case.now);
    assert_eq!(case.rotate(std::slice::from_ref(&case.good), 8), Ok(()));
    assert_eq!(
        case.rotate(&[case.to(9, from, until)], 9),
        rotation_error(RotationError::GenerationNotSequential)
    );
    let mut shifted = case.good.clone();
    shifted.statement.timestamp_ms += 1;
    shifted.digest = case.s.digest(&shifted.statement);
    shifted.signatures = case.s.sign(&case.s.g7, &[0, 1, 2], &shifted.digest);
    assert_eq!(
        case.rotate(&[shifted], 8),
        rotation_error(RotationError::ValidFromMismatch)
    );
    // Validity above MAX_ROSTER_VALIDITY_MS, a start beyond the clock-skew bound and an
    // already expired successor.
    for (valid_from, valid_until) in [
        (from, from + 30 * DAY + 1),
        (now + 3_600_001, now + 3_600_001 + DAY),
        (from, now),
    ] {
        assert_eq!(
            case.rotate(&[case.to(8, valid_from, valid_until)], 8),
            rotation_error(RotationError::BadValidity),
            "{valid_from}..{valid_until}"
        );
    }
    let mut other_next = case.s.key_roster(&[0, 1, 2], 8, from, until);
    other_next.members.push([0xfe; 20]);
    let mut next_mismatch = case.good.clone();
    next_mismatch.next_roster = case.s.view(&other_next);
    assert_eq!(
        case.rotate(&[next_mismatch], 8),
        rotation_error(RotationError::NextDigestMismatch)
    );
}

#[test]
fn rotation_chains_with_bad_signers_or_breaks_fail() {
    let case = RotationCase::new();
    let (from, until) = (case.from, case.until);
    let under_signed = case.s.rotation_step(
        &case.s.g7,
        &case.s.key_roster(&[0, 1, 2, 3], 8, from, until),
        &[0, 1],
    );
    assert_eq!(
        case.rotate(&[under_signed], 8),
        Err(RotationChainError::Step {
            index: 0,
            error: BundleError::TooFewSignatures {
                count: 2,
                threshold: 3
            }
        })
    );
    // Signed by the next roster instead of the destination's current one.
    let g8 = case.s.key_roster(&[0, 1, 2, 3], 8, from, until);
    let wrong_signer = case.s.rotation_step(
        &g8,
        &case.s.key_roster(&[0, 1, 2, 3], 9, from + 1, until),
        &[0, 1, 2],
    );
    assert_eq!(
        case.rotate(&[wrong_signer], 9),
        rotation_error(RotationError::RosterNotAccepted)
    );
    // The current roster expired: the destination is frozen.
    assert_eq!(
        verify_rotation_steps(
            std::slice::from_ref(&case.good),
            None,
            &case.state,
            8,
            &case.s.taira,
            case.s.g7.valid_until_ms + 1
        )
        .map(|_| ()),
        rotation_error(RotationError::RosterNotAccepted)
    );
    // A chain that stops at an unattested handoff.
    assert_eq!(
        verify_rotation_steps(
            std::slice::from_ref(&case.good),
            Some(4242),
            &case.state,
            9,
            &case.s.taira,
            case.now
        ),
        Err(RotationChainError::UnattestedHandoff {
            height: 4242,
            reached: 8,
            target: 9
        })
    );
}

#[test]
fn a_lagging_destination_replays_seventeen_generations_in_two_calls() {
    let s = scenario();
    let context = s.g7_context(T0 + 200_000);
    let mut current = s.g7.clone();
    let mut steps = Vec::new();
    for offset in 1..=17_u64 {
        let from = T0 + 150_000 + offset;
        let next = s.key_roster(&[0, 1, 2, 3], current.generation + 1, from, from + 14 * DAY);
        let keys: &[usize] = if current.generation == 7 {
            &[0, 1, 2]
        } else {
            &[1, 2, 3]
        };
        steps.push(s.rotation_step(&current, &next, keys));
        current = next;
    }
    let plan = verify_rotation_steps(
        &steps,
        None,
        &context.roster_state,
        24,
        &s.taira,
        context.now_ms,
    )
    .expect("chain verifies");
    assert_eq!(plan.final_state.generation, 24);
    let calls = rotate_rosters_calldata(&plan);
    assert_eq!(calls.len(), 2);
    assert!(
        calls
            .iter()
            .all(|call| call[..4] == [0x90, 0x9e, 0xa4, 0x56])
    );
}

// ---------------------------------------------------------------------------------------------
// Controls
// ---------------------------------------------------------------------------------------------

#[test]
fn control_bundles_of_another_revision_or_network_fail() {
    let s = scenario();
    let context = s.g7_context(T0 + 200_000);
    let applied = DestinationControlStateV1::default();
    let mut other_revision = s.control_bundle(false);
    other_revision.revision = 2;
    assert_eq!(
        verify_control_bundle(&other_revision, &context, &applied),
        Err(ControlBundleError::WrongDeployment)
    );
    let revision_two = DestinationContextV1 {
        destination: DestinationV1 {
            route_revision: 2,
            ..s.destination
        },
        ..context
    };
    assert_eq!(
        verify_control_bundle(&other_revision, &revision_two, &applied),
        Err(ControlBundleError::Attestation(BundleError::Proof(
            ProofError::BadBlockPath
        ))),
        "the control leaf binds the revision"
    );
    let mut other_network = s.control_bundle(false);
    other_network.network = SccpNetworkV1::BscMainnet;
    assert_eq!(
        verify_control_bundle(&other_network, &context, &applied),
        Err(ControlBundleError::WrongDeployment)
    );
    let mut resumed = s.control_bundle(false);
    resumed.paused = false;
    assert_eq!(
        verify_control_bundle(&resumed, &context, &applied),
        Err(ControlBundleError::Attestation(BundleError::Proof(
            ProofError::BadBlockPath
        )))
    );
    let mut transfer_position = s.control_bundle(false);
    transfer_position.leaf_index = 0;
    transfer_position.path = s.block100.path(0).expect("path");
    assert_eq!(
        verify_control_bundle(&transfer_position, &context, &applied),
        Err(ControlBundleError::Attestation(BundleError::Proof(
            ProofError::BadBlockPath
        )))
    );
    assert_eq!(
        verify_control_bundle(
            &s.control_bundle(false),
            &context,
            &DestinationControlStateV1 {
                control_nonce: 1,
                minting_paused: true
            }
        ),
        Err(ControlBundleError::Stale)
    );
}

// ---------------------------------------------------------------------------------------------
// EIP-1559
// ---------------------------------------------------------------------------------------------

/// Vectors produced by `ethers` 6 (`Wallet.signTransaction`) with fixture key 0.
struct EthersVector {
    chain_id: u64,
    nonce: u64,
    priority: u128,
    max_fee: u128,
    gas_limit: u64,
    to: Option<[u8; 20]>,
    value: u128,
    data: Vec<u8>,
    unsigned: &'static str,
    signing_hash: &'static str,
    signed: &'static str,
    hash: &'static str,
}

fn ethers_vectors() -> Vec<EthersVector> {
    vec![
        EthersVector {
            chain_id: 1,
            nonce: 7,
            priority: 2_000_000_000,
            max_fee: 30_000_000_000,
            gas_limit: 300_000,
            to: Some([0x22; 20]),
            value: 0,
            data: hex::decode(
                "5b094c0000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000003",
            )
            .expect("hex"),
            unsigned: "02f86e010784773594008506fc23ac00830493e094222222222222222222222222222222222222222280b8445b094c0000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000003c0",
            signing_hash: "7ac8e9a661b4af960911fa48c1a4d3d8a662f9b70d692046a4f6e9a19c9615e4",
            signed: "02f8b1010784773594008506fc23ac00830493e094222222222222222222222222222222222222222280b8445b094c0000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000003c080a0de11561abac7d15eecc33c8fd926227a66b7b38abe552e40d1347c60a40b0687a058c004a5e32d982425206ead057b34749a313d024e179e79dafd8997a3199724",
            hash: "0c5c47605cae9f496dbd72a4d00d3f50a8b948474ff484137b6d7ace795b5186",
        },
        EthersVector {
            chain_id: 56,
            nonce: 0,
            priority: 0,
            max_fee: 1_000_000_000,
            gas_limit: 21_000,
            to: Some(
                hex::decode("7e90b4f929bcfd28c8dec8831d30dacb139d7184")
                    .expect("hex")
                    .try_into()
                    .expect("20 bytes"),
            ),
            value: 1_000_000_000_000_000_000,
            data: Vec::new(),
            unsigned: "02eb388080843b9aca00825208947e90b4f929bcfd28c8dec8831d30dacb139d7184880de0b6b3a764000080c0",
            signing_hash: "8685178feadcd84692a5bf8fa5e5d34ad849ebe68029be6ce936fea6fdf6566a",
            signed: "02f86e388080843b9aca00825208947e90b4f929bcfd28c8dec8831d30dacb139d7184880de0b6b3a764000080c080a094999ef2cd185e46d8260e04ba7cecc813a8f231ed30cd0124f47b404a55a394a0312984475358507fde823756782855080bf59efc5d5c3b19db814d347d7bfa85",
            hash: "3f65e77155c73d0030f6d91bcd2e77a7d54ec52c049b4d470b420c735498c2b8",
        },
        EthersVector {
            chain_id: 1,
            nonce: 1,
            priority: 1,
            max_fee: 1,
            gas_limit: 3_000_000,
            to: None,
            value: 0,
            data: vec![0x60; 60],
            unsigned: "02f84901010101832dc6c08080b83c606060606060606060606060606060606060606060606060606060606060606060606060606060606060606060606060606060606060606060606060c0",
            signing_hash: "066d6fc4aa9239811db3aa65e3cc06fe01b772a48b9d63fdadc6661e7141a43b",
            signed: "02f88c01010101832dc6c08080b83c606060606060606060606060606060606060606060606060606060606060606060606060606060606060606060606060606060606060606060606060c001a0ecd42ee94bae8709e11bb11217b1f92f24f325c77ec0857c1d6bfea797d6dc6ea02e86336ecdd9c790eeb0f56e4ceb010c7fb580b1214c28591dc62fd5e2cc1df4",
            hash: "3e3b452deef43cc376f632641db36013aeab8fab4707bf3020ad4cdcd7b485f0",
        },
    ]
}

#[test]
fn eip1559_transactions_match_ethers_and_recover_the_signer() {
    let s = scenario();
    let key = EvmSigningKey::from_secret(s.secrets[0]).expect("fixture key 0");
    assert_eq!(
        hex::encode(key.address()),
        "7e90b4f929bcfd28c8dec8831d30dacb139d7184"
    );
    for vector in ethers_vectors() {
        let transaction = Eip1559TransactionV1 {
            chain_id: vector.chain_id,
            nonce: vector.nonce,
            max_priority_fee_per_gas: vector.priority,
            max_fee_per_gas: vector.max_fee,
            gas_limit: vector.gas_limit,
            to: vector.to,
            value: vector.value,
            data: vector.data,
        };
        assert_eq!(hex::encode(transaction.unsigned_bytes()), vector.unsigned);
        assert_eq!(hex::encode(transaction.signing_hash()), vector.signing_hash);
        let export = transaction.export_unsigned();
        assert_eq!(export.hex(), format!("0x{}", vector.unsigned));
        assert_eq!(
            Eip1559TransactionV1::decode_unsigned(&export.unsigned).expect("decodes"),
            transaction
        );
        let signed = transaction.sign(&key).expect("signs");
        assert_eq!(signed.sender().expect("sender"), key.address());
        // RFC 6979 with low-S is deterministic, so the signature equals ethers' byte for byte.
        assert_eq!(hex::encode(signed.raw()), vector.signed);
        assert_eq!(hex::encode(signed.hash()), vector.hash);
        let decoded =
            SignedEip1559V1::decode(&hex::decode(vector.signed).expect("hex")).expect("decodes");
        assert_eq!(decoded, signed);
        assert_eq!(decoded.sender().expect("sender"), key.address());
    }
}

#[test]
fn finalize_calldata_ships_in_a_signed_transaction_from_a_key_file() {
    let s = scenario();
    let verified = verify_message_bundle(
        &s.direct_bundle(),
        &s.g7_context(T0 + 200_000),
        BundlePurposeV1::Finalize,
    )
    .expect("verifies");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("relayer.key");
    std::fs::write(&path, format!("0x{}\n", hex::encode(s.secrets[3]))).expect("write key");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("mode");
    }
    let key = EvmSigningKey::load(&path).expect("owner-only key file");
    let transaction = Eip1559TransactionV1::contract_call(
        s.destination.network,
        s.destination.destination_word[12..]
            .try_into()
            .expect("20 bytes"),
        finalize_calldata(&verified).expect("calldata"),
        0,
        1_000_000_000,
        40_000_000_000,
        250_000,
    )
    .expect("transaction");
    let signed = transaction.sign(&key).expect("signed");
    let decoded = SignedEip1559V1::decode(&signed.raw()).expect("decodes");
    assert_eq!(decoded.transaction.data, s.call("finalize_direct"));
    assert_eq!(decoded.sender().expect("sender"), key.address());
    // The --emit path: an external signer returns a signature over the exported hash.
    let export = transaction.export_unsigned();
    let external = key
        .sign_digest(&export.signing_hash)
        .expect("external signature");
    assert_eq!(
        transaction
            .with_signature_from(&external, &key.address())
            .expect("attached"),
        signed
    );
    let parts = export.qr_payloads(1_000).expect("qr parts");
    assert!(parts.len() > 1);
    assert!(parts.iter().all(|part| part.len() <= 1_000));
}
