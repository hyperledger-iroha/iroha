//! Canonical bounded private recovery encoding. This is not monetary authority.
use super::*;
use crate::{
    indexed_map::{InsertWitness, Leaf, Path, SearchWitness},
    receive_lineage::{PriorCredentialOpening, ReceiveMapWitness},
    send_controls::{Charge, QuotaWitness, Segment, Window, WindowSlot},
};
use ff::{Field, PrimeField};

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.ExactWitness.v1")]
struct Frame {
    version: u16,
    kind: u8,
    fields: Vec<[u8; 32]>,
    bytes: Vec<u8>,
}
impl Drop for Frame {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.fields);
        zeroize::Zeroize::zeroize(&mut self.bytes);
    }
}
struct Writer {
    fields: Vec<[u8; 32]>,
    bytes: Vec<u8>,
}
impl Writer {
    fn raw(&mut self, b: &[u8]) {
        self.bytes.extend_from_slice(b)
    }
    fn uint(&mut self, v: u128, n: usize) {
        self.raw(&v.to_le_bytes()[..n]);
    }
    fn field(&mut self, v: Fp) {
        self.fields.push(v.to_repr());
    }
    fn fields(&mut self, v: &[Fp]) {
        for x in v {
            self.field(*x);
        }
    }
    fn credential(&mut self, v: &Credential) {
        self.raw(&v.body);
        self.raw(&v.signature);
    }
    fn receipt(&mut self, v: &bootstrap::Receipt) {
        self.raw(&v.body);
        self.raw(&v.signature);
    }
    fn request(&mut self, v: &CompletedRequest) {
        self.raw(&v.body);
        self.raw(&v.signature);
    }
    fn leaf(&mut self, l: &Leaf) {
        self.fields(&[l.key, l.value, l.next]);
    }
    fn path<const N: usize>(&mut self, p: &Path<N>) {
        self.uint(p.index as u128, 4);
        self.fields(&p.siblings);
    }
    fn insert<const N: usize>(&mut self, x: &InsertWitness<N>) {
        self.fields(&[x.old_root, x.key, x.value]);
        self.leaf(&x.low);
        self.path(&x.low_path);
        self.path(&x.slot);
    }
    fn search<const N: usize>(&mut self, x: &SearchWitness<N>) {
        self.fields(&[x.root, x.key]);
        self.leaf(&x.leaf);
        self.path(&x.path);
    }
    fn quota(&mut self, x: &QuotaWitness) {
        for s in &x.segments {
            self.uint(s.base as u128, 1);
            for a in &s.slots {
                self.uint(a.window.kind as u128, 1);
                self.uint(a.window.start as u128, 8);
                self.uint(a.window.end as u128, 8);
                self.uint(a.window.limit, 16);
                self.fields(&a.siblings);
            }
        }
        for c in &x.charges {
            self.uint(c.slot as u128, 1);
            self.fields(&c.siblings);
            self.uint(c.used, 16);
        }
    }
    fn blob(&mut self, b: &[u8]) {
        self.uint(b.len() as u128, 4);
        self.raw(b);
    }
}
struct Reader<'a> {
    fields: &'a [[u8; 32]],
    bytes: &'a [u8],
}
impl<'a> Reader<'a> {
    fn raw(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.bytes.len() {
            return Err(Error::Malformed("recovery witness length"));
        }
        let (a, b) = self.bytes.split_at(n);
        self.bytes = b;
        Ok(a)
    }
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.raw(N)?.try_into().unwrap())
    }
    fn uint(&mut self, n: usize) -> Result<u128> {
        let mut b = [0; 16];
        b[..n].copy_from_slice(self.raw(n)?);
        Ok(u128::from_le_bytes(b))
    }
    fn field(&mut self) -> Result<Fp> {
        let (a, b) = self
            .fields
            .split_first()
            .ok_or(Error::Malformed("recovery fields"))?;
        self.fields = b;
        field(a)
    }
    fn fields<const N: usize>(&mut self) -> Result<[Fp; N]> {
        let mut out = [Fp::ZERO; N];
        for x in &mut out {
            *x = self.field()?;
        }
        Ok(out)
    }
    fn credential(&mut self) -> Result<Credential> {
        Ok(Credential {
            body: self.fixed()?,
            signature: self.fixed()?,
        })
    }
    fn receipt(&mut self) -> Result<bootstrap::Receipt> {
        Ok(bootstrap::Receipt {
            body: self.fixed()?,
            signature: self.fixed()?,
        })
    }
    fn request(&mut self) -> Result<CompletedRequest> {
        Ok(CompletedRequest {
            body: self.fixed()?,
            signature: self.fixed()?,
        })
    }
    fn leaf(&mut self) -> Result<Leaf> {
        Ok(Leaf {
            key: self.field()?,
            value: self.field()?,
            next: self.field()?,
        })
    }
    fn path<const N: usize>(&mut self) -> Result<Path<N>> {
        Ok(Path {
            index: self.uint(4)? as u32,
            siblings: self.fields()?,
        })
    }
    fn insert<const N: usize>(&mut self) -> Result<InsertWitness<N>> {
        Ok(InsertWitness {
            old_root: self.field()?,
            key: self.field()?,
            value: self.field()?,
            low: self.leaf()?,
            low_path: self.path()?,
            slot: self.path()?,
        })
    }
    fn search<const N: usize>(&mut self) -> Result<SearchWitness<N>> {
        Ok(SearchWitness {
            root: self.field()?,
            key: self.field()?,
            leaf: self.leaf()?,
            path: self.path()?,
        })
    }
    fn quota(&mut self) -> Result<QuotaWitness> {
        let mut q = QuotaWitness::default();
        for s in &mut q.segments {
            s.base = self.uint(1)? as u8;
            for a in &mut s.slots {
                a.window = Window {
                    kind: self.uint(1)? as u8,
                    start: self.uint(8)? as u64,
                    end: self.uint(8)? as u64,
                    limit: self.uint(16)?,
                };
                a.siblings = self.fields()?;
            }
        }
        for c in &mut q.charges {
            c.slot = self.uint(1)? as u8;
            c.siblings = self.fields()?;
            c.used = self.uint(16)?;
        }
        Ok(q)
    }
    fn blob(&mut self) -> Result<Arc<[u8]>> {
        let n = usize::try_from(self.uint(4)?).map_err(|_| Error::ResourceLimit)?;
        if n > MAX_RECOVERY_BYTES {
            return Err(Error::ResourceLimit);
        }
        Ok(self.raw(n)?.into())
    }
}
pub(super) fn encode_witness(witness: &ExactOperationWitness) -> Result<Vec<u8>> {
    let mut w = Writer {
        fields: vec![],
        bytes: vec![],
    };
    let kind = match witness {
        ExactOperationWitness::Bootstrap(b) => {
            w.raw(&b.scheme);
            w.raw(&b.relation);
            w.blob(&b.root.x.to_bytes_be());
            w.blob(&b.root.y.to_bytes_be());
            w.raw(&b.input.certificate.body);
            w.raw(&b.input.certificate.signature);
            w.credential(&b.input.credential);
            w.fields(&b.input.core);
            w.fields(&b.input.rest);
            w.fields(&b.input.statement);
            w.receipt(&b.input.receipt);
            1
        }
        ExactOperationWitness::Load {
            step: x,
            receipt_original,
            finality_original,
        } => {
            w.fields(&x.before_core);
            w.fields(&x.before_rest);
            w.fields(&x.after_core);
            w.fields(&x.after_rest);
            w.credential(&x.credential);
            w.raw(&x.ordinary);
            w.fields(&x.statement);
            w.receipt(&x.receipt);
            w.insert(&x.recovery);
            w.blob(receipt_original);
            w.blob(finality_original);
            2
        }
        ExactOperationWitness::Send(x) => {
            w.fields(&x.before_core);
            w.fields(&x.before_rest);
            w.fields(&x.after_core);
            w.fields(&x.after_rest);
            w.credential(&x.credential);
            w.request(&x.request);
            w.raw(&x.fee_schedule.body);
            w.raw(&x.fee_schedule.signature);
            w.quota(&x.quota);
            w.uint(x.lower as u128, 8);
            w.uint(x.upper as u128, 8);
            w.fields(&x.statement);
            w.receipt(&x.receipt);
            w.insert(&x.maps.pending);
            w.insert(&x.maps.fee);
            3
        }
        ExactOperationWitness::Receive {
            step: x,
            incoming_payment,
        } => {
            w.fields(&x.before_core);
            w.fields(&x.before_rest);
            w.fields(&x.after_core);
            w.fields(&x.after_rest);
            w.credential(&x.credential);
            w.request(&x.request);
            w.credential(&x.payer);
            w.credential(&x.quoted_receiver);
            w.fields(&x.incoming_statement);
            w.receipt(&x.incoming_receipt);
            w.field(x.prior_credential.digest);
            w.fields(&x.prior_credential.siblings);
            w.fields(&x.statement);
            w.receipt(&x.receipt);
            w.search(&x.maps.consumed_search);
            w.insert(&x.maps.consumed_insert);
            w.uint(x.maps.insert as u128, 1);
            w.search(&x.maps.credit_search);
            w.insert(&x.maps.credit_insert);
            w.blob(incoming_payment);
            4
        }
    };
    super::encode(&Frame {
        version: 1,
        kind,
        fields: w.fields,
        bytes: w.bytes,
    })
}
pub(super) fn decode_witness(bytes: &[u8]) -> Result<ExactOperationWitness> {
    let f: Frame = super::decode(bytes)?;
    if f.version != 1 {
        return Err(Error::Malformed("recovery version"));
    }
    let mut r = Reader {
        fields: &f.fields,
        bytes: &f.bytes,
    };
    let out = match f.kind {
        1 => {
            let scheme = r.fixed()?;
            let relation = r.fixed()?;
            let root = p256::Point {
                x: BigUint::from_bytes_be(&r.blob()?),
                y: BigUint::from_bytes_be(&r.blob()?),
            };
            let certificate = crate::certificate::Certificate {
                body: r.fixed()?,
                signature: r.fixed()?,
            };
            let input = bootstrap::Inputs {
                certificate,
                credential: r.credential()?,
                core: r.fields()?,
                rest: r.fields()?,
                statement: r.fields()?,
                receipt: r.receipt()?,
            };
            ExactOperationWitness::Bootstrap(Box::new(BootstrapWitness {
                scheme,
                root,
                relation,
                input,
            }))
        }
        2 => {
            let step = Box::new(crate::load::Witness {
                before_core: r.fields()?,
                before_rest: r.fields()?,
                after_core: r.fields()?,
                after_rest: r.fields()?,
                credential: r.credential()?,
                ordinary: r.fixed()?,
                statement: r.fields()?,
                receipt: r.receipt()?,
                recovery: r.insert()?,
            });
            ExactOperationWitness::Load {
                step,
                receipt_original: r.blob()?,
                finality_original: r.blob()?,
            }
        }
        3 => ExactOperationWitness::Send(Box::new(crate::send::Witness {
            before_core: r.fields()?,
            before_rest: r.fields()?,
            after_core: r.fields()?,
            after_rest: r.fields()?,
            credential: r.credential()?,
            request: r.request()?,
            fee_schedule: crate::fee_schedule::SignedFeeSchedule {
                body: r.fixed()?,
                signature: r.fixed()?,
            },
            quota: r.quota()?,
            lower: r.uint(8)? as u64,
            upper: r.uint(8)? as u64,
            statement: r.fields()?,
            receipt: r.receipt()?,
            maps: crate::send_maps::Witness {
                pending: r.insert()?,
                fee: r.insert()?,
            },
        })),
        4 => {
            let step = Box::new(crate::receive::Witness {
                before_core: r.fields()?,
                before_rest: r.fields()?,
                after_core: r.fields()?,
                after_rest: r.fields()?,
                credential: r.credential()?,
                request: r.request()?,
                payer: r.credential()?,
                quoted_receiver: r.credential()?,
                incoming_statement: r.fields()?,
                incoming_receipt: r.receipt()?,
                prior_credential: PriorCredentialOpening {
                    digest: r.field()?,
                    siblings: r.fields()?,
                },
                statement: r.fields()?,
                receipt: r.receipt()?,
                maps: ReceiveMapWitness {
                    consumed_search: r.search()?,
                    consumed_insert: r.insert()?,
                    insert: match r.uint(1)? {
                        0 => false,
                        1 => true,
                        _ => return Err(Error::Malformed("recovery insertion bit")),
                    },
                    credit_search: r.search()?,
                    credit_insert: r.insert()?,
                },
            });
            ExactOperationWitness::Receive {
                step,
                incoming_payment: r.blob()?,
            }
        }
        _ => return Err(Error::Malformed("recovery operation kind")),
    };
    if !r.fields.is_empty() || !r.bytes.is_empty() || encode_witness(&out)? != bytes {
        return Err(Error::Malformed("recovery canonical original"));
    }
    Ok(out)
}

/// Scrub decoded private monetary openings when the last retained owner drops.
pub(super) fn wipe(witness: &mut ExactOperationWitness) {
    use zeroize::Zeroize;
    fn leaf(x: &mut Leaf) {
        x.key.zeroize();
        x.value.zeroize();
        x.next.zeroize();
    }
    fn path<const N: usize>(x: &mut Path<N>) {
        x.index.zeroize();
        x.siblings.zeroize();
    }
    fn insert<const N: usize>(x: &mut InsertWitness<N>) {
        x.old_root.zeroize();
        x.key.zeroize();
        x.value.zeroize();
        leaf(&mut x.low);
        path(&mut x.low_path);
        path(&mut x.slot);
    }
    fn search<const N: usize>(x: &mut SearchWitness<N>) {
        x.root.zeroize();
        x.key.zeroize();
        leaf(&mut x.leaf);
        path(&mut x.path);
    }
    match witness {
        ExactOperationWitness::Bootstrap(x) => {
            x.input.core.zeroize();
            x.input.rest.zeroize();
            x.input.statement.zeroize();
        }
        ExactOperationWitness::Load { step: x, .. } => {
            x.before_core.zeroize();
            x.before_rest.zeroize();
            x.after_core.zeroize();
            x.after_rest.zeroize();
            x.statement.zeroize();
            x.ordinary.zeroize();
            insert(&mut x.recovery);
        }
        ExactOperationWitness::Send(x) => {
            x.before_core.zeroize();
            x.before_rest.zeroize();
            x.after_core.zeroize();
            x.after_rest.zeroize();
            x.statement.zeroize();
            x.lower.zeroize();
            x.upper.zeroize();
            for segment in &mut x.quota.segments {
                segment.base.zeroize();
                for slot in &mut segment.slots {
                    slot.window.kind.zeroize();
                    slot.window.start.zeroize();
                    slot.window.end.zeroize();
                    slot.window.limit.zeroize();
                    slot.siblings.zeroize();
                }
            }
            for charge in &mut x.quota.charges {
                charge.slot.zeroize();
                charge.used.zeroize();
                charge.siblings.zeroize();
            }
            insert(&mut x.maps.pending);
            insert(&mut x.maps.fee);
        }
        ExactOperationWitness::Receive { step: x, .. } => {
            x.before_core.zeroize();
            x.before_rest.zeroize();
            x.after_core.zeroize();
            x.after_rest.zeroize();
            x.statement.zeroize();
            x.prior_credential.digest.zeroize();
            x.prior_credential.siblings.zeroize();
            search(&mut x.maps.consumed_search);
            insert(&mut x.maps.consumed_insert);
            search(&mut x.maps.credit_search);
            insert(&mut x.maps.credit_insert);
        }
    }
}
