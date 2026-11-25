//! Recursive lifecycle edges not exercised by the fixed-point/malformed-Payment
//! run or the fresh-process two-wallet custody test. Only isolated software
//! keys are used. No apparent settlement or public ledger is created.
use super::*;
use crate::{bootstrap_context as context, control, full::WalletKey, unload::tests::Map};
use iroha_data_model::account::AccountId;

type LoadOriginal = (KagemushaWalletLoadReceiptV1, KagemushaWalletLoadFinalityV1);

#[derive(Clone)]
struct Wallet {
    key: WalletKey,
    account: AccountId,
    bootstrap: ExactOperationWitness,
    state: bootstrap::Inputs,
    head: VerifiedWalletHead,
    map: Map,
}

fn finish(
    parameters: &AuthenticatedParameters,
    key: &WalletKey,
    predecessor: &VerifiedWalletHead,
    operation: &ExactOperationWitness,
) -> CompletedOperation {
    let engine = NativeEngine::default();
    let prepared = engine.prepare(operation, predecessor, parameters).unwrap();
    let selected = sign_selected(&prepared, &key.d, "isolated Control lifecycle").unwrap();
    engine.finalize(prepared, &selected, parameters).unwrap()
}

impl Wallet {
    fn enroll(parameters: &AuthenticatedParameters, label: &str, id: u8, account: &str) -> Self {
        let key = WalletKey::labelled(label);
        let account = AccountId::new(account.parse().unwrap());
        let digest = kagemusha_wallet_account_digest_v1(&account).unwrap();
        let (bootstrap, marker) =
            software_bootstrap(parameters, &key, vector_asset().asset_digest(), digest, id)
                .unwrap();
        bootstrap.credential().enrollment_fields().unwrap();
        let ExactOperationWitness::Bootstrap(b) = &bootstrap else {
            unreachable!()
        };
        let state = b.input.clone();
        let origin = NativeEngine::default()
            .enrollment_origin(b, marker, parameters)
            .unwrap();
        let completed = finish(parameters, &key, &origin, &bootstrap);
        Self {
            key,
            account,
            bootstrap,
            state,
            head: completed.head().clone(),
            map: Map::new(),
        }
    }

    fn advance(&mut self, params: &AuthenticatedParameters, op: &ExactOperationWitness) -> Vec<u8> {
        let completed = finish(params, &self.key, &self.head, op);
        let (core, rest, statement) = match op {
            ExactOperationWitness::RefreshTimeAnchor(w)
            | ExactOperationWitness::Unload { step: w, .. } => {
                (w.after_core, w.after_rest, w.statement)
            }
            ExactOperationWitness::Load { step: w, .. } => {
                (w.after_core, w.after_rest, w.statement)
            }
            ExactOperationWitness::Send(w) => (w.after_core, w.after_rest, w.statement),
            ExactOperationWitness::Receive { step: w, .. } => {
                (w.after_core, w.after_rest, w.statement)
            }
            ExactOperationWitness::Bootstrap(_) | ExactOperationWitness::ArchiveSent { .. } => {
                unreachable!()
            }
        };
        self.set_head(completed.head(), core, rest, statement);
        completed.completion().output_part(1).unwrap()
    }

    fn set_head(
        &mut self,
        head: &VerifiedWalletHead,
        core: [Fp; 30],
        rest: [Fp; 6],
        statement: [Fp; 26],
    ) {
        assert_eq!(
            head.profile()[crate::full::HEAD_IO],
            crate::monetary_state::commitment(&core, &rest)
        );
        assert_eq!(self.map.root(), core[19]);
        eprintln!(
            "lifecycle {} -> {} balance={} load ordinal={} redeem ordinal={}",
            self.state.statement[16].to_repr()[0],
            statement[16].to_repr()[0],
            u128::from_le_bytes(core[9].to_repr()[..16].try_into().unwrap()),
            core[13].to_repr()[0],
            core[14].to_repr()[0]
        );
        self.head = head.clone();
        (self.state.core, self.state.rest, self.state.statement) = (core, rest, statement);
    }

    fn load(&mut self, params: &AuthenticatedParameters, original: &LoadOriginal) {
        assert_eq!(Fp::from_u128(original.0.ordinal), self.state.core[13]);
        let before = self.state.core[9];
        let witness = load_from_receipt(&self.state, &original.0, &original.1, |k, v| {
            (self.map.insert(k, v), self.map.root())
        })
        .unwrap();
        assert!(self.advance(params, &witness).is_empty());
        assert_eq!(self.state.core[9], before + Fp::from(100));
        assert!(
            NativeEngine::default()
                .prepare(&witness, &self.head, params)
                .is_err()
        );
    }

    fn refresh(&mut self, params: &AuthenticatedParameters, time: u64) -> ExactOperationWitness {
        let before = self.state.core[9];
        let refresh = control::tests::recursive_refresh_at(
            &self.state.core,
            &self.state.rest,
            self.head.profile(),
            &self.key,
            &self.state,
            time,
        );
        let witness = ExactOperationWitness::RefreshTimeAnchor(Box::new(refresh));
        assert!(self.advance(params, &witness).is_empty());
        assert_eq!(self.state.core[9], before);
        assert_eq!(self.state.core[28], Fp::from(time));
        witness
    }

    fn unload_input(&self) -> crate::unload::Witness {
        let s = &self.state;
        crate::unload::tests::recursive_unload_with_map(
            (&s.core, &s.rest, &s.credential, &s.statement, &s.receipt),
            *self.head.profile(),
            &self.key,
            &self.map,
            0,
            Fp::ZERO,
        )
    }

    fn unload(&mut self, params: &AuthenticatedParameters) -> [u8; 32] {
        let old_ordinal = self.state.core[14];
        let old_balance = self.state.core[9];
        let input = self.unload_input();
        let entry = self.map.insert(input.recovery.key, input.recovery.value);
        assert_eq!(entry.old_root, input.recovery.old_root);
        let control = control::Witness::unload(input);
        let destinations = engine::UnloadDestinations {
            account: self.account.clone(),
            charge: KagemushaWalletUnloadChargeV1::None,
            quote_certificate: vec![],
        };
        let op = ExactOperationWitness::Unload {
            step: Box::new(control.clone()),
            destinations: Box::new(destinations),
        };
        let encoded = op.encode().unwrap();
        assert_eq!(
            ExactOperationWitness::decode(&encoded)
                .unwrap()
                .encode()
                .unwrap(),
            encoded
        );
        let engine = NativeEngine::isolated_unload_fixture();
        let prepared = engine.prepare(&op, &self.head, params).unwrap();
        let review = prepared.review_bytes().unwrap();
        let capsule = prepared.capsule().clone();
        assert_eq!(&review[..3], &[3, 0, 6]);
        assert_eq!(&review[163..195], &control.statement[17].to_repr());
        assert_eq!(&review[195..211], &old_ordinal.to_repr()[..16]);
        assert_eq!(&review[211..227], &4u128.to_le_bytes());
        assert_eq!(&review[227..243], &[0; 16]);
        assert_eq!(&review[243..259], &4u128.to_le_bytes());
        assert_eq!(&review[259..291], &[0; 32]);
        assert_eq!(&review[291..323], &control.credential.body[98..130]);
        assert_eq!(&review[323..355], &[0; 32]);
        let account = self
            .account
            .to_account_address()
            .unwrap()
            .canonical_bytes()
            .unwrap();
        assert_eq!(&review[419..421], &(account.len() as u16).to_le_bytes());
        assert_eq!(&review[421..421 + account.len()], account.as_slice());
        assert_eq!(&review[421 + account.len()..], &[0, 0]);
        let recovered = NativeEngine::default().resume(&capsule, params).unwrap();
        assert_eq!(recovered.review_bytes().unwrap(), review);
        assert!(
            NativeEngine::default()
                .before_selection(&capsule, params)
                .is_err()
        );
        let selected = sign_selected(
            &prepared,
            &self.key.d,
            "isolated Unload exact selected receipt",
        )
        .unwrap();
        assert!(
            NativeEngine::default()
                .finalize(recovered, &selected, params)
                .is_err()
        );
        engine.before_selection(&capsule, params).unwrap();
        let completed = engine.finalize(prepared, &selected, params).unwrap();
        let bytes = completed.completion().output_part(3).unwrap();
        assert!(completed.completion().output_part(1).unwrap().is_empty());
        let reread = NativeEngine::default()
            .verify_completion(&capsule, completed.completion(), params)
            .unwrap();
        assert_eq!(reread.completion().output_part(3).unwrap(), bytes);
        let head = completed.head();
        if let Some(dir) = std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT") {
            let root = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join("uncharged-unload-review.bin"), &review).unwrap();
        }
        let terms = engine
            .verify_unload_claim(&bytes, params, capsule.scheme(), capsule.asset())
            .unwrap();
        assert_eq!(Fp::from_u128(terms.ordinal()), old_ordinal);
        assert_eq!(
            (
                terms.amount(),
                terms.account_payout(),
                terms.online_charge()
            ),
            (4, 4, 0)
        );
        self.set_head(
            &head,
            control.after_core,
            control.after_rest,
            control.statement,
        );
        assert_eq!(self.state.core[9], old_balance - Fp::from(4));
        assert_eq!(self.state.core[14], old_ordinal + Fp::ONE);
        *terms.nullifier()
    }

    fn send(&mut self, params: &AuthenticatedParameters, receiver: &Wallet) -> Vec<u8> {
        let origin = SendOrigin {
            core: &self.state.core,
            rest: &self.state.rest,
            statement: &self.state.statement,
            credential: &self.state.credential,
            burned: self.head.profile()[context::LINEAGE_IO + context::BURNED_TOTAL],
        };
        let request = CompletedRequest {
            body: request_from(&origin, &receiver.state.credential)
                .try_into()
                .unwrap(),
            signature: [0; 64],
        };
        let sig = bootstrap::sign(
            crate::request::commitments(&request).unwrap().message,
            &receiver.key.d,
            "Control lifecycle exact Request",
        );
        let op = send10_from(&origin, &receiver.state.credential, sig).unwrap();
        let before = self.state.core[9];
        let bytes = self.advance(params, &op);
        assert_eq!(self.state.core[9], before - Fp::from(10));
        assert!(bytes.len() <= crate::payment_wire::QR_BUDGET);
        bytes
    }
}

#[test]
#[ignore = "requires authenticated three-mode compiler originals"]
fn combined_control_predecessors_and_repeated_operations() {
    let path =
        std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let params = install_test_bundle(&path).unwrap();
    let mut a = Wallet::enroll(
        &params,
        "Control lifecycle A",
        0x71,
        "ed0120D75A980182B10AB7D54BFED3C964073A0EE172F3DAA62325AF021A68F707511A",
    );
    let mut b = Wallet::enroll(
        &params,
        "Control lifecycle B",
        0x72,
        "ed01203D4017C3E843895A92B70AA74D1B7EBC9C982CCF2EC4968CC0CD55F12AF4660C",
    );
    let originals = certified_loads(&[(&a.state.credential, 4), (&b.state.credential, 2)]).unwrap();
    let zero = control::Witness::unload(a.unload_input());
    assert!(!control::tests::witness_holds(
        zero,
        *a.head.profile(),
        &a.key
    ));
    eprintln!("lifecycle zero-balance Bootstrap -> positive Unload refused");
    let clock = 1_763_400_000_000;
    a.refresh(&params, clock);
    a.refresh(&params, clock + 1);
    a.load(&params, &originals[0]);
    a.load(&params, &originals[1]);
    let first = a.unload(&params);
    let second = a.unload(&params);
    assert_ne!(first, second);
    a.load(&params, &originals[2]);
    let third = a.unload(&params);
    assert_ne!(second, third);
    // These exercise independent possible successors of the same genuine head.
    // They never publish competing wallet actions or ledger transfers.
    a.clone().refresh(&params, clock + 2);
    let outgoing = a.send(&params, &b);
    a.clone().refresh(&params, clock + 2);
    a.clone().unload(&params);
    a.load(&params, &originals[3]);
    b.load(&params, &originals[4]);
    let refreshed = b.refresh(&params, clock + 3);
    let incoming = NativeEngine::default()
        .verify_payment(&outgoing, &params)
        .unwrap();
    let receive = receive_from(&b.bootstrap, &refreshed, b.head.profile(), &incoming).unwrap();
    let before = b.head.clone();
    b.advance(&params, &receive);
    require_credited(before.profile(), b.head.profile()).unwrap();
    assert_eq!(b.state.core[9], Fp::from(110));
    b.clone().load(&params, &originals[5]);
    b.unload(&params);
    let returned = b.send(&params, &a);
    let received = NativeEngine::default()
        .verify_payment(&returned, &params)
        .unwrap();
    assert_eq!(
        received.request().body[130..162],
        a.state.credential.body[66..98]
    );
    assert_eq!(b.state.core[9], Fp::from(96));
    eprintln!(
        "lifecycle finished, outgoing {} bytes, return {} bytes, no ledger transfers",
        outgoing.len(),
        returned.len()
    );
}

/// The relation label can remain the same across independently signed manifests.
#[test]
#[ignore = "requires the signed device bundle and a complete retained Payment"]
fn retained_payment_credit_status_requires_its_exact_manifest() {
    let bundle =
        std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let mut parameters = install_test_bundle(&bundle).unwrap();
    let original =
        std::fs::read(std::env::var_os("KAGEMUSHA_SPARTAN_RETAINED_PAYMENT").unwrap()).unwrap();
    let engine = NativeEngine::default();
    let payment = engine.verify_payment(&original, &parameters).unwrap();
    payment.require_parameters(&parameters).unwrap();
    parameters.manifest_digest[0] ^= 1;
    assert_eq!(parameters.id(), payment.parameters_id());
    assert!(matches!(
        engine.verify_credit_status_peer(&[], &[], &payment, &parameters),
        Err(Error::AuthorityMismatch(
            "verified Payment parameter manifest"
        ))
    ));
}

/// Spend an already retained Payment through both orderings of Refresh and a
/// joined Receive. Each branch uses the same local proof inputs in isolation.
#[test]
#[ignore = "requires the retained signed-profile two-wallet Payment original"]
fn refresh_around_a_joined_payment_and_immediate_send() {
    let bundle =
        std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let params = install_test_bundle(&bundle).unwrap();
    let original =
        std::fs::read(std::env::var_os("KAGEMUSHA_SPARTAN_RETAINED_PAYMENT").unwrap()).unwrap();
    let engine = NativeEngine::default();
    let incoming = engine.verify_payment(&original, &params).unwrap();
    let b = Counterparty::new(&params, incoming.payer()).unwrap();
    let c_key = WalletKey::labelled("isolated joined history peer C");
    let c_account = AccountId::new(
        "ed0120FC51CD8E6218A1A38DA47ED00230F0580816ED13BA3303AC5DEB911548908025"
            .parse()
            .unwrap(),
    );
    let asset = b.credential().body[34..66].try_into().unwrap();
    let (c, _) = software_bootstrap(
        &params,
        &c_key,
        asset,
        kagemusha_wallet_account_digest_v1(&c_account).unwrap(),
        0x63,
    )
    .unwrap();
    c.credential().enrollment_fields().unwrap();
    assert_eq!(
        incoming.request().body[130..162],
        b.credential().body[66..98]
    );
    let ExactOperationWitness::Bootstrap(boot) = b.bootstrap() else {
        unreachable!()
    };
    let enrolled = finish(
        &params,
        &b.key,
        &b.origin(&engine, &params).unwrap(),
        b.bootstrap(),
    );
    let clock = 1_763_400_000_030;
    let refresh = |core: &[Fp; 30], rest: &[Fp; 6], head: &VerifiedWalletHead, time: u64| {
        let op = ExactOperationWitness::RefreshTimeAnchor(Box::new(
            control::tests::recursive_refresh_at(
                core,
                rest,
                head.profile(),
                &b.key,
                &boot.input,
                time,
            ),
        ));
        let complete = finish(&params, &b.key, head, &op);
        (op, complete)
    };
    let (first, refreshed) = refresh(&boot.input.core, &boot.input.rest, enrolled.head(), clock);
    let receive =
        receive_from(b.bootstrap(), &first, refreshed.head().profile(), &incoming).unwrap();
    let received = finish(&params, &b.key, refreshed.head(), &receive);
    require_credited(refreshed.head().profile(), received.head().profile()).unwrap();
    let ExactOperationWitness::Receive { step, .. } = &receive else {
        unreachable!()
    };
    assert_eq!(step.after_core[9], Fp::from(10));
    let pay = |origin: SendOrigin<'_>, head: &VerifiedWalletHead| {
        let request = CompletedRequest {
            body: request_from(&origin, c.credential()).try_into().unwrap(),
            signature: [0; 64],
        };
        let sig = bootstrap::sign(
            crate::request::commitments(&request).unwrap().message,
            &c_key.d,
            "joined history C Request",
        );
        let op = send10_from(&origin, c.credential(), sig).unwrap();
        let ExactOperationWitness::Send(send) = &op else {
            unreachable!()
        };
        assert_eq!(send.after_core[9], Fp::ZERO);
        let done = finish(&params, &b.key, head, &op);
        let bytes = done.completion().output_part(1).unwrap();
        let payment = engine.verify_payment(&bytes, &params).unwrap();
        assert_eq!(
            payment.request().body[130..162],
            c.credential().body[66..98]
        );
        bytes.len()
    };
    let direct = pay(
        SendOrigin::after_receive(&receive, received.head().profile()).unwrap(),
        received.head(),
    );
    let (second, again) = refresh(
        &step.after_core,
        &step.after_rest,
        received.head(),
        clock + 1,
    );
    let ExactOperationWitness::RefreshTimeAnchor(updated) = &second else {
        unreachable!()
    };
    let after = pay(
        SendOrigin {
            core: &updated.after_core,
            rest: &updated.after_rest,
            statement: &updated.statement,
            credential: &updated.credential,
            burned: again.head().profile()[context::LINEAGE_IO + context::BURNED_TOTAL],
        },
        again.head(),
    );
    assert!(direct <= crate::payment_wire::QR_BUDGET && after <= crate::payment_wire::QR_BUDGET);
    eprintln!(
        "retained A→B Payment {} B; B Refresh→Receive→Send {} B, and Receive→Refresh→Send {} B; each B balance 10→0",
        original.len(),
        direct,
        after
    );
}
