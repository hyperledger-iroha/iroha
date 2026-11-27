//! Incoming folded-status receipt constraints for the ArchiveSent successor.
//! The quoted key uses its own derived table. The payer's wallet table remains available
//! for its own future hardware receipt.
//! One fixed relation opens either receipt form. A public form bit selects the Receive
//! receipt of this Payment (bit 0) or a later receiver Send head receipt (bit 1). The bit
//! sets the signed operation kind and subject and the receipt's Payment-digest field.
//! Every other check applies to both forms.
//!
//! The head statement is opened in-circuit for both forms. Two kinds of check apply:
//! - Exact original, a hard constraint. The private 26-word preimage opens under kgwstmt1
//!   to the public statement digest. The verifier reconstructs that digest from the
//!   natively authenticated statement, in the same way that `expected[5]` pins the raw
//!   receipt.
//! - Semantic match, total predicates in the evidence result:
//!   - the signed receipt names exactly that digest;
//!   - the receipt's sequence, predecessor and successor are the statement's;
//!   - its operation id is `kgwopid1(wallet, s[16], s[17])`;
//!   - `s[16]` is the form's kind and `s[17]` is the public subject;
//!   - the statement names the quoted scheme and asset;
//!   - in the Receive form, the effect names this Payment's credit, the retained Request's
//!     payer wallet and the Payment amount, zero-filled.
//!
//! Every predicate reads only values that the public words pin. A prover with fixed
//! authentic evidence therefore cannot reach the no-op by changing the preimage privately:
//! any other preimage fails the hard opening on both branches. Malformed public evidence
//! still derives a false result and the no-op.
use super::*;
use crate::{
    bootstrap::Receipt, certificate, credential::Credential, credential_policy, nonnative,
    operation_auth, p256, receipt, receive_originals, signature,
};
use std::collections::HashSet;

const RECEIVER_TABLE: u32 = 11;
#[derive(Clone)]
pub(in crate::native::engine) struct ReceiverReceipt {
    pub(in crate::native::engine) request: Option<crate::request::CompletedRequest>,
    pub(in crate::native::engine) opening: Option<super::credit_opening::StatusOpening>,
    pub(in crate::native::engine) credential: Credential,
    pub(in crate::native::engine) receipt: Receipt,
    // Authenticated quoted credential, receiver folded receipt/head/statement, original Payment and exact source receipt digest.
    // The final six fields are the wallet ID and payment key of the folded receiver.
    pub(in crate::native::engine) expected: [Fp; 12],
    // Public form bit: false for the Receive receipt of this Payment, true for a later
    // receiver Send head whose credit root covers its credit.
    pub(in crate::native::engine) send: bool,
    // Public credit named by the head's signed operation id. The Receive form fixes it to
    // this Payment's credit. The Send form takes it from the later Send statement.
    pub(in crate::native::engine) subject: Fp,
    // Private exact original of the head statement. It has a fixed length in both forms.
    pub(in crate::native::engine) statement: [Fp; 26],
    // Public digest of that statement, reconstructed by the verifier from the natively
    // authenticated statement. Only `statement` opens it. The folded `expected[3]` and the
    // signed receipt's field are separate semantic matches.
    pub(in crate::native::engine) statement_digest: Fp,
}

pub(in crate::native::engine) struct ReceiptResults {
    pub(in crate::native::engine) evidence: Source,
    pub(in crate::native::engine) signature: Source,
    pub(in crate::native::engine) public: [Source; 15],
    pub(in crate::native::engine) proof_digest: Source,
    pub(in crate::native::engine) statement_fields: [Source; 26],
    pub(in crate::native::engine) quoted_table: [Source; 4],
    pub(in crate::native::engine) quoted_wallet: [Source; 2],
    pub(in crate::native::engine) quoted_key: [Source; 4],
    pub(in crate::native::engine) request: Option<super::request_original::RequestResults>,
    pub(in crate::native::engine) opening: Vec<Source>,
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
    /// Public words 64..79: the twelve expected words, the form bit, the subject and the
    /// exact statement digest.
    pub(in crate::native::engine) fn public_words(&self) -> [Fp; 15] {
        std::array::from_fn(|i| match i {
            12 => Fp::from(u64::from(self.send)),
            13 => self.subject,
            14 => self.statement_digest,
            _ => self.expected[i],
        })
    }

    // Receive-form effect of the opened statement: this Payment's credit (through the
    // subject), the retained Request's payer wallet and the Payment amount, zero-filled.
    // The Send form's effect belongs to the later Send. Only the covering credit opening
    // binds this Payment there. The result is ANDed into the evidence.
    fn append_receive_effect(
        &self,
        w: &mut Wiring<Fp>,
        family: usize,
        statement: [Source; 26],
        form: Source,
        payer: Option<[Source; 2]>,
        amount: Source,
        evidence: Source,
    ) -> Source {
        use credential_policy::{all, negate, zero_predicate};
        let mut c = Cells::new(family);
        c.a.name = "ArchiveSent Receive-form statement effect";
        let [prior, send_form] = [evidence, form].map(|bit| {
            let k = c.bit(bit.value(w) == Fp::ONE);
            c.eq(own(k, 1), bit.linear());
            k
        });
        // Without a Request (component tests only) the payer wallet has no source.
        let mut terms = vec![(statement[20], Some(amount))];
        if let Some(payer) = payer {
            terms.push((statement[18], Some(payer[0])));
            terms.push((statement[19], Some(payer[1])));
        }
        terms.extend(statement[21..].iter().map(|&field| (field, None)));
        let matches: Vec<_> = terms
            .into_iter()
            .map(|(field, expected)| {
                let (lin, value) = match expected {
                    Some(e) => (e.linear(), e.value(w)),
                    None => (vec![], Fp::ZERO),
                };
                zero_predicate(
                    &mut c,
                    plus(&field.linear(), &scaled(&lin, -Fp::ONE)),
                    field.value(w) - value,
                )
            })
            .collect();
        let effect = all(&mut c, &matches);
        let receive_form = negate(&mut c, send_form);
        let missing = negate(&mut c, effect);
        let violated = all(&mut c, &[receive_form, missing]);
        let required = negate(&mut c, violated);
        let out = all(&mut c, &[prior, required]);
        let local = w.push(c.a);
        source(local, out)
    }

    pub(in crate::native::engine) fn append(
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
        // The private fixed-length head statement and its kgwstmt1 digest.
        let opened = receive_originals::append_statement(w, family, &self.statement);
        let s = opened.fields();

        let mut c = Cells::new(family);
        c.a.name = "ArchiveSent receiver receipt and exact Payment bindings";
        let expected = self.expected.map(|v| c.cell(v, 255));
        let form = c.bit(self.send);
        let subject = c.cell(self.subject, 255);
        let statement_digest = c.cell(self.statement_digest, 255);
        // Exact original. The preimage must open the verifier's digest. This is a hard
        // constraint, never a result: substituting a private preimage under authentic
        // public evidence is unsatisfiable on both branches.
        c.eq(opened.digest().linear(), own(statement_digest, 1));
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
        // As natively: the head receipt's operation id is recomputed from the statement's
        // own kind and credit. Two predicates below tie those words to the form and subject.
        let op_inputs = [
            (wallet[0].value(w), wallet[0].linear()),
            (wallet[1].value(w), wallet[1].linear()),
            (s[16].value(w), s[16].linear()),
            (s[17].value(w), s[17].linear()),
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
        // Semantic match of the opened statement, for both forms. The signed receipt names
        // this exact statement and carries its sequence, predecessor and successor. The
        // statement's kind is the form's and its credit is the public subject. Its scheme and
        // asset are the quoted credential's, which the retained Request carries.
        let kind = (
            plus(
                &constant(vec![receive]),
                &scaled(&own(form, 1), send - receive),
            ),
            if self.send { send } else { receive },
        );
        let scheme = quoted.scheme();
        let asset = quoted.asset();
        for (left, right) in [
            (
                original.statement_digest(),
                (own(statement_digest, 1), self.statement_digest),
            ),
            (original.sequence(), (s[9].linear(), s[9].value(w))),
            (original.predecessor(), (s[14].linear(), s[14].value(w))),
            (original.successor(), (s[15].linear(), s[15].value(w))),
            (s[16], kind),
            (s[17], (own(subject, 1), self.subject)),
            (s[3], (scheme[0].linear(), scheme[0].value(w))),
            (s[4], (scheme[1].linear(), scheme[1].value(w))),
            (s[5], (asset[0].linear(), asset[0].value(w))),
            (s[6], (asset[1].linear(), asset[1].value(w))),
        ] {
            evidence.push(credential_policy::zero_predicate(
                &mut c,
                plus(&left.linear(), &scaled(&right.0, -Fp::ONE)),
                left.value(w) - right.1,
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
        let evidence = self.append_receive_effect(
            w,
            family,
            s,
            source(local, form),
            request.as_ref().map(|request| request.payer),
            descriptor[4],
            source(local, evidence),
        );
        let (evidence, opening) = match &self.opening {
            Some(opening) => {
                opening.append(w, family, credit, source(local, expected[4]), evidence)
            }
            None => (evidence, Vec::new()),
        };
        let ports: [usize; 15] = std::array::from_fn(|i| match i {
            12 => form,
            13 => subject,
            14 => statement_digest,
            _ => expected[i],
        });
        ReceiptResults {
            evidence,
            opening,
            signature: signed,
            request,
            public: ports.map(|i| source(local, i)),
            proof_digest: original.proof_digest(),
            statement_fields: s,
            quoted_table: quoted.table_commitment(),
            quoted_wallet: quoted.wallet(),
            quoted_key: quoted.payment_key(),
        }
    }
}
