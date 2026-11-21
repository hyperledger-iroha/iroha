//! Incoming folded-status receipt constraints for the ArchiveSent successor.
//! The quoted key uses its own derived table. The payer's wallet table remains available
//! for its own future hardware receipt.
//! One fixed relation opens either receipt form. A public form bit selects the Receive
//! receipt of this Payment (bit 0) or a later receiver Send head receipt (bit 1). The bit
//! sets the signed operation kind and subject and the receipt's Payment-digest field.
//! Every other check applies to both forms.
use super::*;
use crate::{
    bootstrap::Receipt, certificate, credential::Credential, credential_policy, nonnative,
    operation_auth, p256, receipt, signature,
};
use std::collections::HashSet;

const RECEIVER_TABLE: u32 = 11;
#[derive(Clone)]
pub(super) struct ReceiverReceipt {
    pub(super) request: Option<crate::request::CompletedRequest>,
    pub(super) opening: Option<super::credit_opening::StatusOpening>,
    pub(super) credential: Credential,
    pub(super) receipt: Receipt,
    // Authenticated quoted credential, receiver folded receipt/head/statement, original Payment and exact source receipt digest.
    // The final six fields are the wallet ID and payment key of the folded receiver.
    pub(super) expected: [Fp; 12],
    // Public form bit: false for the Receive receipt of this Payment, true for a later
    // receiver Send head whose credit root covers its credit.
    pub(super) send: bool,
    // Public credit named by the head's signed operation id. The Receive form fixes it to
    // this Payment's credit. The Send form takes it from the later Send statement.
    pub(super) subject: Fp,
}

pub(super) struct ReceiptResults {
    pub(super) evidence: Source,
    pub(super) signature: Source,
    pub(super) public: [Source; 14],
    pub(super) request: Option<super::request_original::RequestResults>,
    pub(super) opening: Vec<Source>,
}

fn receiver_table(
    w: &mut Wiring<Fp>,
    family: usize,
    key: &operation_auth::CredentialOpening,
) -> p256::Point {
    let q = key.payment_point(w);
    let mut key_cells = Cells::new(family);
    key_cells.a.name = "ArchiveSent quoted receiver key limbs";
    let mut coordinate = |offset, value: &num_bigint::BigUint| {
        let bytes: Vec<_> = (offset..offset + 32)
            .rev()
            .map(|i| {
                let byte = key.byte(i);
                let k = key_cells.byte(byte.value(w).to_repr()[0]);
                key_cells.eq(own(k, 1), byte.linear());
                k
            })
            .collect();
        key_cells.value(value, &bytes)
    };
    let x = coordinate(131, &q.x);
    let y = coordinate(163, &q.y);
    let key = nonnative::KeyLimbSource {
        array: w.push(key_cells.a),
        x,
        y,
        instance: 0,
    };
    let table = w.push(Array {
        name: "ArchiveSent derived receiver signature table",
        family,
        rho: 9,
        r_count: nonnative::WALLET_ROWS,
        fixed: false,
        lookups: vec![],
        widths: vec![9, 88, 88, 80, 88, 88, 80],
        rows: vec![],
        values: nonnative::wallet_table_values(&q),
    });
    w.tables.push((table, RECEIVER_TABLE));
    let first = w.arrays.len();
    let derived = nonnative::key_tables(first, family, &q);
    w.arrays.extend(derived.arrays);
    w.tables.extend(derived.tables);
    nonnative::bind_derived_key_table(&mut w.arrays, first, table, key);
    certificate::install_fixed_key_table(
        w,
        family,
        "ArchiveSent P-256 generator",
        nonnative::POINT_TABLE,
        &p256::generator(),
    );
    q
}

impl ReceiverReceipt {
    /// Public words 64..78: the twelve expected words, the form bit and the subject.
    pub(super) fn public_words(&self) -> [Fp; 14] {
        std::array::from_fn(|i| match i {
            12 => Fp::from(u64::from(self.send)),
            13 => self.subject,
            _ => self.expected[i],
        })
    }

    pub(super) fn append(
        &self,
        w: &mut Wiring<Fp>,
        family: usize,
        descriptor: [Source; 7],
        other_evidence: Source,
    ) -> ReceiptResults {
        let credit = descriptor[0];
        let quoted = operation_auth::append_opening_total(w, family, &self.credential);
        let original = receipt::append_opening_total(w, family, &self.receipt);
        let q = receiver_table(w, family, &quoted);
        let signature_bytes = std::array::from_fn(|i| original.byte(receipt::BODY_BYTES + i));
        let signed = signature::append_signature_total(
            w,
            family,
            "ArchiveSent folded receiver receipt ECDSA",
            original.message(),
            &signature_bytes,
            signature::TotalKey {
                table: RECEIVER_TABLE,
                q: &q,
            },
            vec![],
        )
        .valid();

        let mut c = Cells::new(family);
        c.a.name = "ArchiveSent receiver receipt and exact Payment bindings";
        let expected = self.expected.map(|v| c.cell(v, 255));
        let form = c.bit(self.send);
        let subject = c.cell(self.subject, 255);
        let receive_form = plus(&constant(vec![Fp::ONE]), &scaled(&own(form, 1), -Fp::ONE));
        // A Receive receipt names this Payment's credit. Only a later Send names its own.
        c.a.rows.push([
            receive_form.clone(),
            plus(&own(subject, 1), &scaled(&credit.linear(), -Fp::ONE)),
            vec![],
        ]);
        // The Receive receipt carries this Payment's digest. A Send head receipt carries
        // zero, and only its credit opening binds this Payment.
        let carried_value = if self.send {
            Fp::ZERO
        } else {
            self.expected[4]
        };
        let carried = c.cell(carried_value, 255);
        c.a.rows
            .push([receive_form, own(expected[4], 1), own(carried, 1)]);
        // The locally authenticated Request must supply the exact quoted receiver credential.
        c.eq(quoted.digest().linear(), own(expected[0], 1));
        c.eq(quoted.valid().linear(), constant(vec![Fp::ONE]));
        // The raw receipt is fixed by the original evidence independently of its
        // agreement with the incoming folded head. A prover cannot substitute a
        // failing signature to elect a no-op on authentic delivery.
        c.eq(original.digest().linear(), own(expected[5], 1));
        let receive = Fp::from(u64::from(KagemushaWalletOperationKindV1::Receive.tag()));
        let send = Fp::from(u64::from(KagemushaWalletOperationKindV1::Send.tag()));
        let wallet = quoted.wallet();
        let op_inputs = [
            (wallet[0].value(w), wallet[0].linear()),
            (wallet[1].value(w), wallet[1].linear()),
            (
                if self.send { send } else { receive },
                plus(&constant(vec![receive]), &scaled(&own(form, 1), send - receive)),
            ),
            (self.subject, own(subject, 1)),
        ];
        let operation = poseidon::hash_with_domain(
            u64::from_le_bytes(*b"kgwopid1"),
            &op_inputs.clone().map(|(v, _)| v),
        );
        let op_hash = [
            c.cell(Fp::ZERO, 255),
            c.cell(operation, 255),
            c.cell(Fp::ZERO, 255),
        ];
        let mut evidence = vec![];
        for bit in [other_evidence, original.valid()] {
            let k = c.bit(bit.value(w) == Fp::ONE);
            c.eq(own(k, 1), bit.linear());
            evidence.push(k);
        }
        for (source, field, value) in [
            (original.digest(), expected[1], self.expected[1]),
            (original.successor(), expected[2], self.expected[2]),
            (original.statement_digest(), expected[3], self.expected[3]),
            (original.payment_digest(), carried, carried_value),
            (original.operation_id(), op_hash[1], operation),
        ] {
            evidence.push(credential_policy::zero_predicate(
                &mut c,
                plus(&source.linear(), &scaled(&own(field, 1), -Fp::ONE)),
                source.value(w) - value,
            ));
        }
        for (left, right) in original
            .wallet()
            .into_iter()
            .zip(quoted.wallet())
            .chain(original.scheme().into_iter().zip(quoted.scheme()))
        {
            evidence.push(credential_policy::zero_predicate(
                &mut c,
                plus(&left.linear(), &scaled(&right.linear(), -Fp::ONE)),
                left.value(w) - right.value(w),
            ));
        }
        for (k, claim) in quoted
            .wallet()
            .into_iter()
            .chain(quoted.payment_key())
            .enumerate()
        {
            let i = 6 + k;
            evidence.push(credential_policy::zero_predicate(
                &mut c,
                plus(&claim.linear(), &scaled(&own(expected[i], 1), -Fp::ONE)),
                claim.value(w) - self.expected[i],
            ));
        }
        let evidence = credential_policy::all(&mut c, &evidence);
        let local = w.push(c.a);
        certificate::hash_relation(
            w,
            family,
            local,
            u64::from_le_bytes(*b"kgwopid1"),
            &op_inputs,
            op_hash,
            &mut SpikeRng::new(b"ArchiveSent incoming operation"),
        );
        let request = self.request.as_ref().map(|input| {
            super::request_original::append(
                w,
                family,
                input,
                &quoted,
                signature::TotalKey {
                    table: RECEIVER_TABLE,
                    q: &q,
                },
                descriptor,
                signed,
            )
        });
        let signed = request
            .as_ref()
            .map_or(signed, |request| request.signatures);
        let mut seen = HashSet::new();
        w.tables.retain(|(_, id)| seen.insert(*id));
        let (evidence, opening) = match &self.opening {
            Some(opening) => opening.append(
                w,
                family,
                credit,
                source(local, expected[4]),
                source(local, evidence),
            ),
            None => (source(local, evidence), Vec::new()),
        };
        let ports: [usize; 14] = std::array::from_fn(|i| match i {
            12 => form,
            13 => subject,
            _ => expected[i],
        });
        ReceiptResults {
            evidence,
            opening,
            signature: signed,
            request,
            public: ports.map(|i| source(local, i)),
        }
    }
}
