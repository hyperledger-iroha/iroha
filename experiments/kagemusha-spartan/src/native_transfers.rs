//! Construct offline transfers and the signed TimeAnchor refresh from a selected wallet head,
//! immutable public originals, and authenticated persistent map openings. No private wallet
//! witness is supplied by an app.
use super::*;
use crate::{
    bootstrap_context as bc, fee_schedule,
    indexed_map::{self, InsertWitness, Path, SearchWitness},
    monetary_state, request,
};
use iroha_data_model::kagemusha::{
    KagemushaWalletIndexedInsertV1 as PublicInsert, KagemushaWalletIndexedLeafV1 as PublicLeaf,
    KagemushaWalletIndexedOpeningV1 as PublicPath,
};
use iroha_pasta::poseidon;

/// Independent indexed trees addressed by roots in the selected wallet head.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SpartanWalletMap {
    Pending,
    Fee,
    Consumed,
    Credit,
    Recovery,
}

/// Wallet-owned IMT store. An insertion stages an immutable successor descriptor. Only
/// the actual native-released successor can subsequently select that root for spending.
pub trait SpartanWalletMaps {
    fn member_or_low(
        &mut self,
        map: SpartanWalletMap,
        root: [u8; 32],
        key: [u8; 32],
    ) -> Result<(PublicLeaf, PublicPath)>;
    fn insert(
        &mut self,
        map: SpartanWalletMap,
        root: [u8; 32],
        key: [u8; 32],
        value: [u8; 32],
    ) -> Result<PublicInsert>;
    /// Stage ArchiveSent's removal of a present pending-outgoing key (wallet wire v1:
    /// relink the predecessor, then clear the slot). Like an insertion, the staged root
    /// gains no authority unless a released successor selects it. A store without
    /// removal staging refuses, so no ArchiveSent witness can be formed from it.
    fn remove(
        &mut self,
        map: SpartanWalletMap,
        root: [u8; 32],
        key: [u8; 32],
    ) -> Result<iroha_data_model::kagemusha::KagemushaWalletIndexedRemoveV1> {
        let _ = (map, root, key);
        Err(Error::Unavailable("wallet pending-outgoing removal staging"))
    }
}

#[path = "native_archive_sent.rs"]
mod archive_sent;

fn leaf(public: PublicLeaf) -> Result<indexed_map::Leaf> {
    Ok(indexed_map::Leaf {
        key: field(&public.key)?,
        value: field(&public.value)?,
        next: field(&public.next_key)?,
    })
}
fn path(public: &PublicPath) -> Result<Path<32>> {
    let mut siblings = [Fp::ZERO; 32];
    for (v, raw) in siblings.iter_mut().zip(&public.siblings) {
        *v = field(raw)?;
    }
    Ok(Path {
        index: public.slot,
        siblings,
    })
}
fn insert(
    maps: &mut dyn SpartanWalletMaps,
    which: SpartanWalletMap,
    root: Fp,
    key: Fp,
    value: Fp,
) -> Result<(InsertWitness<32>, Fp)> {
    let plan = maps.insert(which, root.to_repr(), key.to_repr(), value.to_repr())?;
    let next = plan
        .verify(&root.to_repr(), &key.to_repr(), &value.to_repr())
        .map_err(|_| Error::AuthorityMismatch("wallet transfer map insertion"))?;
    Ok((
        InsertWitness {
            old_root: root,
            key,
            value,
            low: leaf(plan.low)?,
            low_path: path(&plan.low_opening)?,
            slot: path(&plan.slot_opening)?,
        },
        field(&next)?,
    ))
}
fn search(
    maps: &mut dyn SpartanWalletMaps,
    which: SpartanWalletMap,
    root: Fp,
    key: Fp,
) -> Result<SearchWitness<32>> {
    use iroha_data_model::kagemusha::{
        kagemusha_wallet_indexed_verify_membership_v1 as member,
        kagemusha_wallet_indexed_verify_non_membership_v1 as absent,
    };
    let (opened, route) = maps.member_or_low(which, root.to_repr(), key.to_repr())?;
    if opened.key == key.to_repr() {
        member(&root.to_repr(), &opened, &route)
    } else {
        absent(&root.to_repr(), &key.to_repr(), &opened, &route)
    }
    .map_err(|_| Error::AuthorityMismatch("wallet transfer map opening"))?;
    Ok(SearchWitness {
        root,
        key,
        leaf: leaf(opened)?,
        path: path(&route)?,
    })
}

fn integer(v: Fp) -> Result<u128> {
    let bytes = v.to_repr();
    if bytes[16..] != [0; 16] {
        return Err(Error::Malformed("wallet transfer unsigned integer"));
    }
    Ok(u128::from_le_bytes(bytes[..16].try_into().unwrap()))
}
fn u64_word(v: Fp) -> Result<u64> {
    integer(v)?
        .try_into()
        .map_err(|_| Error::Malformed("wallet transfer u64"))
}
fn add(v: Fp, amount: u128) -> Result<Fp> {
    Ok(Fp::from_u128(integer(v)?.checked_add(amount).ok_or(
        Error::Malformed("wallet transfer counter overflow"),
    )?))
}
fn checked_source<'a>(
    prepared: &'a PreparedOperation,
    head: &VerifiedWalletHead,
    parameters: &AuthenticatedParameters,
) -> Result<(&'a [Fp; 30], &'a [Fp; 6])> {
    head.require_parameters(parameters)?;
    let (core, rest) = prepared.successor_opening();
    let commitment = monetary_state::commitment(core, rest);
    if head.enrollment_marker.is_some()
        || head.pair.is_empty()
        || (head.custody_digest != [0; 32] && &head.custody_digest != prepared.capsule_digest())
        || prepared.manifest_digest != *parameters.manifest_digest()
        || head.profile()[full::HEAD_IO] != commitment
        || commitment.to_repr() != *prepared.capsule.successor()
        || !exact_credential(prepared.credential(), head.credential())
        || head.profile()[bc::LINEAGE_IO + bc::LIFECYCLE] != core[0]
    {
        return Err(Error::AuthorityMismatch(
            "wallet transfer selected predecessor",
        ));
    }
    Ok((core, rest))
}
fn op_id(c: &Credential, tag: u64, credit: Fp) -> Fp {
    let w = payment_wire::limbs(&c.body[66..98]);
    poseidon::hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[w[0], w[1], Fp::from(tag), credit],
    )
}
fn receipt(c: &Credential, s: &[Fp; 26], tag: u64, payment: Fp) -> Result<bootstrap::Receipt> {
    let mut body = [0; 338];
    body[0] = 1;
    for (dst, src) in [(2, 2), (34, 66), (66, 259)] {
        body[dst..dst + 32].copy_from_slice(&c.body[src..src + 32]);
    }
    body[98..114].copy_from_slice(&integer(s[9])?.to_le_bytes());
    for (dst, val) in [
        (114, op_id(c, tag, s[17])),
        (146, s[14]),
        (178, s[15]),
        (210, crate::statement::digest(s)),
        (306, payment),
    ] {
        body[dst..dst + 32].copy_from_slice(&val.to_repr());
    }
    Ok(bootstrap::Receipt {
        body,
        signature: [0; 64],
    })
}
fn completed(
    open: &OpenRequestPeer<'_>,
    payer: &Credential,
    core: &[Fp; 30],
) -> Result<CompletedRequest> {
    let mut req = CompletedRequest::from_tape(open.request).map_err(Error::Malformed)?;
    req.body[66..130].copy_from_slice(&payer.body[66..130]);
    req.body[194..210].copy_from_slice(&integer(core[12])?.to_le_bytes());
    if req.body[66..98] == req.body[130..162]
        || payer.body[130..195] == open.receiver_credential[130..195]
    {
        return Err(Error::AuthorityMismatch(
            "wallet transfer distinct wallets and keys",
        ));
    }
    Ok(req)
}
impl PreparedOperation {
    /// Time-enabled Send must pin the protected same-boot interval before building its witness.
    pub fn successor_requires_send_time(&self) -> Result<bool> {
        let mask = u64_word(self.successor_opening().0[22])?;
        if mask > 3 {
            return Err(Error::Malformed("wallet Send controls"));
        }
        Ok(mask != 0)
    }
}
impl NativeEngine {
    /// Identity of the prospective debit under the actually selected head. Needed by the
    /// protected time provider to freeze an interval before forming the exact Send review.
    pub fn wallet_send_operation_id(
        &self,
        prepared: &PreparedOperation,
        head: &VerifiedWalletHead,
        open_wire: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<[u8; 32]> {
        let (core, _) = checked_source(prepared, head, parameters)?;
        let open = self.authenticate_open_request_peer(open_wire, parameters)?;
        let req = completed(&open, head.credential(), core)?;
        let credit = request::commitments(&req).map_err(Error::Malformed)?.credit;
        Ok(op_id(head.credential(), 3, credit).to_repr())
    }
    /// Prepare exact private Send inputs. Applications supply only the signed public peer Request.
    pub fn wallet_send_witness(
        &self,
        prepared: &PreparedOperation,
        head: &VerifiedWalletHead,
        open_wire: &[u8],
        maps: &mut dyn SpartanWalletMaps,
        parameters: &AuthenticatedParameters,
    ) -> Result<ExactOperationWitness> {
        self.require_minimum_memory(parameters, full::SEND)?;
        let (before, rest) = checked_source(prepared, head, parameters)?;
        let c = head.credential();
        let open = self.authenticate_open_request_peer(open_wire, parameters)?;
        let request = completed(&open, c, before)?;
        let rq = request::commitments(&request).map_err(Error::Malformed)?;
        let fee = integer(rq.words[request::word::FEE])?;
        let amount = integer(rq.words[request::word::AMOUNT])?;
        let gross = amount
            .checked_add(fee)
            .ok_or(Error::Malformed("wallet Send amount plus fee"))?;
        let lineage = &head.profile()[bc::LINEAGE_IO..];
        let burned = integer(lineage[bc::BURNED_TOTAL])?;
        let balance = integer(before[9])?;
        let after_balance = balance
            .checked_sub(gross)
            .filter(|remaining| *remaining >= burned)
            .ok_or(Error::AuthorityMismatch(
                "wallet Send insufficient spendable value",
            ))?;
        if rq.words[request::word::FEE_SCHEDULE] != rest[2]
            || rq.words[request::word::SCHEME_POLICY] != rest[1]
            || rq.words[request::word::POLICY_EPOCH] != before[27]
        {
            return Err(Error::AuthorityMismatch("wallet Send held Request policy"));
        }
        let controls = u64_word(before[22])?;
        if controls & 1 != 0 {
            return Err(Error::Unavailable(
                "protected quota window and usage openings",
            ));
        }
        let floor = u64_word(before[28])?.max(u64_word(rq.words[request::word::ACCEPTED_TIME])?);
        let (lower, upper) = match controls {
            0 => (floor, floor),
            2 => {
                let operation = op_id(c, 3, rq.credit).to_repr();
                let (anchor, clock) = self
                    .time
                    .as_ref()
                    .ok_or(Error::Unavailable("protected same-boot Send time"))?
                    .prepared(c.body[66..98].try_into().unwrap(), &operation)?;
                if anchor.anchor.body.wallet_id != c.body[66..98]
                    || anchor.anchor.body.scheme_id != c.body[2..34]
                    || anchor.anchor.time_anchor_digest() != rest[5].to_repr()
                {
                    return Err(Error::AuthorityMismatch("wallet Send time anchor"));
                }
                let interval = anchor
                    .interval_at(&clock, u64_word(before[25])?)
                    .map_err(|e| Error::Rejected(e.to_string()))?;
                let lower = floor.max(interval.lower_ms);
                let upper = lower.max(interval.upper_ms);
                if upper >= u64_word(before[26])? {
                    return Err(Error::Rejected("wallet Send lease expired".into()));
                }
                (lower, upper)
            }
            _ => return Err(Error::Malformed("wallet Send controls")),
        };
        let mut s = *terms(&prepared.witness).1;
        let mut after = *before;
        s[7] = credential::digest(c);
        s[8] = before[0];
        s[9] = add(before[11], 1)?;
        s[10] = before[13];
        s[11] = before[22];
        s[12] = lineage[bc::BURNED_TOTAL];
        s[13] = lineage[bc::PENDING_ROOT];
        s[14] = monetary_state::commitment(before, rest);
        s[16] = Fp::from(3);
        s[17] = rq.credit;
        s[18..20].copy_from_slice(&payment_wire::limbs(&request.body[130..162]));
        s[20] = before[12];
        s[21] = Fp::from_u128(amount);
        s[22] = Fp::from_u128(fee);
        s[23] = rq.digest;
        s[24] = Fp::from(lower);
        s[25] = Fp::from(upper);
        let pvalue = poseidon::hash_with_domain(crate::send_maps::PENDING_DOMAIN, &s[17..24]);
        let (pending, pending_root) =
            insert(maps, SpartanWalletMap::Pending, s[13], rq.credit, pvalue)?;
        let fvalue =
            poseidon::hash_with_domain(crate::send_maps::FEE_DOMAIN, &[rq.credit, s[22], rest[2]]);
        let (fee_open, fee_root) = if fee != 0 {
            insert(maps, SpartanWalletMap::Fee, before[20], rq.credit, fvalue)?
        } else {
            (indexed_map::empty_insert(Fp::ONE, Fp::ONE).0, before[20])
        };
        after[9] = Fp::from_u128(after_balance);
        after[10] = s[12];
        after[11] = s[9];
        after[12] = add(before[12], 1)?;
        let mut chain = vec![before[15]];
        chain.extend_from_slice(&s[17..24]);
        after[15] = poseidon::hash_with_domain(crate::send_arithmetic::CHAIN_DOMAIN, &chain);
        after[18] = pending_root;
        after[20] = fee_root;
        after[28] = Fp::from(lower);
        after[29] = super::funding::fresh_state_nonce(Some(before[29]))?;
        s[15] = monetary_state::commitment(&after, rest);
        let schedule = if open.fee_schedule.is_empty() {
            fee_schedule::SignedFeeSchedule {
                body: [0; 191],
                signature: [0; 64],
            }
        } else {
            fee_schedule::SignedFeeSchedule {
                body: open.fee_schedule[..191].try_into().unwrap(),
                signature: open.fee_schedule[191..].try_into().unwrap(),
            }
        };
        Ok(ExactOperationWitness::Send(Box::new(
            crate::send::Witness {
                before_core: *before,
                before_rest: *rest,
                after_core: after,
                after_rest: *rest,
                credential: c.clone(),
                request,
                request_originals: request::Originals {
                    receiver: read_credential(open.receiver_credential)?,
                    certificates: open
                        .certificates
                        .chunks_exact(172)
                        .map(read_certificate)
                        .collect::<Result<Vec<_>>>()?,
                },
                fee_schedule: schedule,
                quota: crate::send_controls::QuotaWitness::default(),
                lower,
                upper,
                statement: s,
                receipt: receipt(c, &s, 3, Fp::ZERO)?,
                maps: crate::send_maps::Witness {
                    pending,
                    fee: fee_open,
                },
            },
        )))
    }
}

/// Stable action identity. Native has to check the original Request before using it.
pub fn wallet_send_replay_key(open_wire: &[u8]) -> Result<[u8; 32]> {
    let open = decode_open_request_peer(open_wire)?;
    let req = CompletedRequest::from_tape(open.request).map_err(Error::Malformed)?;
    Ok(request::commitments(&req)
        .map_err(Error::Malformed)?
        .digest
        .to_repr())
}
/// Exact transport identity for receiver reconciliation, independent of the current head.
pub fn wallet_receive_replay_key(payment_wire: &[u8]) -> Result<[u8; 32]> {
    payment_wire::PeerOriginals::Payment
        .decode(payment_wire)
        .map_err(Error::Rejected)?;
    Ok(super::super::custody_digest(
        b"spartan-received-payment-action-v1",
        payment_wire,
    ))
}
impl ExactOperationWitness {
    /// Scope the durable action record to an immutable quote or exact received Payment.
    pub fn transfer_replay_key(&self) -> Result<Option<[u8; 32]>> {
        match self {
            Self::Send(step) => Ok(Some(
                request::commitments(&step.request)
                    .map_err(Error::Malformed)?
                    .digest
                    .to_repr(),
            )),
            Self::Receive {
                incoming_payment, ..
            } => wallet_receive_replay_key(incoming_payment).map(Some),
            _ => Ok(None),
        }
    }
}

impl NativeEngine {
    /// Construct a new Receive from the exact incoming carrier and the
    /// original Request retained under this wallet's protected request ID.
    pub fn wallet_receive_witness(
        &self,
        prepared: &PreparedOperation,
        head: &VerifiedWalletHead,
        payment_wire: &[u8],
        hosted_open_wire: &[u8],
        maps: &mut dyn SpartanWalletMaps,
        parameters: &AuthenticatedParameters,
    ) -> Result<ExactOperationWitness> {
        self.require_minimum_memory(parameters, full::RECEIVE)?;
        let (before, rest) = checked_source(prepared, head, parameters)?;
        let c = head.credential();
        let hosted = self.authenticate_open_request_peer(hosted_open_wire, parameters)?;
        let quoted = read_credential(hosted.receiver_credential)?;
        if !exact_credential(c, &quoted) {
            return Err(Error::Unavailable(
                "protected prior receiver credential opening",
            ));
        }
        let incoming = self.incoming_candidate(payment_wire, parameters)?;
        let r = &incoming.originals.request;
        if hosted.request[..66] != r.body[..66]
            || hosted.request[130..194] != r.body[130..194]
            || hosted.request[210..418] != r.body[210..]
            || hosted.request[418..] != r.signature
        {
            return Err(Error::AuthorityMismatch(
                "incoming Payment and hosted open Request",
            ));
        }
        let rq = request::commitments(r).map_err(Error::Malformed)?;
        let amount = integer(rq.words[request::word::AMOUNT])?;
        let mut after = *before;
        after[9] = add(before[9], amount)?;
        after[11] = add(before[11], 1)?;
        after[29] = super::funding::fresh_state_nonce(Some(before[29]))?;
        after[16] = poseidon::hash_with_domain(
            crate::receive::CHAIN_DOMAIN,
            &[
                before[16],
                rq.credit,
                rq.words[request::word::PAYER_WALLET[0]],
                rq.words[request::word::PAYER_WALLET[1]],
                Fp::from_u128(amount),
            ],
        );
        let consumed = search(maps, SpartanWalletMap::Consumed, before[17], rq.credit)?;
        let credit_root = head.profile[bc::LINEAGE_IO + bc::CREDIT_ROOT];
        let credit = search(maps, SpartanWalletMap::Credit, credit_root, rq.credit)?;
        // Exact selected retries were resolved by custody. An unselected attempt cannot publish
        // the defensive Burn branch, or overwrite the first digest recorded for an existing credit.
        if !incoming.valid || consumed.leaf.key == rq.credit || credit.leaf.key == rq.credit {
            return Err(Error::Rejected("incoming Payment cannot credit this receiver".into()));
        }
        let value = poseidon::hash_with_domain(crate::receive_lineage::CONSUMED_DOMAIN,
            &[rq.credit, Fp::from_u128(amount), after[11]]);
        let (consumed_insert, consumed_root) = insert(maps, SpartanWalletMap::Consumed, before[17], rq.credit, value)?;
        after[17] = consumed_root;
        let value = poseidon::hash_with_domain(crate::receive_lineage::CREDIT_DOMAIN,
            &[rq.credit, incoming.payment_digest, Fp::ZERO]);
        let (credit_insert, _) = insert(maps, SpartanWalletMap::Credit, credit_root, rq.credit, value)?;
        let mut s = *terms(&prepared.witness).1;
        s[7] = credential::digest(c);
        s[8] = before[0];
        s[9] = after[11];
        s[10] = after[13];
        s[11] = before[22];
        s[12] = Fp::ZERO;
        s[13] = Fp::ZERO;
        s[14] = monetary_state::commitment(before, rest);
        s[15] = monetary_state::commitment(&after, rest);
        s[16] = Fp::from(4);
        s[17..].fill(Fp::ZERO);
        s[17] = rq.credit;
        s[18..20].copy_from_slice(&payment_wire::limbs(&r.body[66..98]));
        s[20] = Fp::from_u128(amount);
        let mut empty =
            poseidon::hash_with_domain(crate::receive_lineage::PRIOR_LEAF_DOMAIN, &[Fp::ZERO]);
        let siblings = std::array::from_fn(|_| {
            let v = empty;
            empty = poseidon::hash_with_domain(crate::receive_lineage::PRIOR_NODE_DOMAIN, &[v, v]);
            v
        });
        Ok(ExactOperationWitness::Receive {
            step: Box::new(crate::receive::Witness {
                before_core: *before,
                before_rest: *rest,
                after_core: after,
                after_rest: *rest,
                credential: c.clone(),
                request: r.clone(),
                payer: incoming.originals.payer,
                quoted_receiver: quoted,
                incoming_statement: incoming.originals.statement,
                incoming_receipt: incoming.originals.receipt,
                prior_credential: crate::receive_lineage::PriorCredentialOpening {
                    digest: Fp::ZERO,
                    siblings,
                },
                statement: s,
                receipt: receipt(c, &s, 4, incoming.payment_digest)?,
                maps: crate::receive_lineage::ReceiveMapWitness {
                    consumed_search: consumed,
                    consumed_insert,
                    insert: true,
                    credit_search: credit,
                    credit_insert,
                },
            }),
            incoming_payment: payment_wire.into(),
        })
    }
}

impl NativeEngine {
    /// Construct the signed RefreshTimeAnchor that follows the selected released head.
    ///
    /// `anchor_original` is the signed 202-byte TimeAnchor (`body || r || s`) and
    /// `certificate_original` its 172-byte TimeAnchor-role signer certificate. Both are
    /// authenticated for this wallet under the installed scheme root before OS entropy is
    /// sampled. The successor changes only the sequence, `floor = max(floor, T)`, the
    /// nonce and the held anchor digest `d`; the operation identity is
    /// `P(kgwopid1, wallet, 7, d)`. A cached or peer-carried anchor authenticates the same
    /// way. Nothing here records OS time or a direct-exchange observation, so a
    /// time-controlled Send still needs the protected direct exchange of the held anchor.
    ///
    /// # Errors
    /// Rejects a foreign scope or head, malformed or unauthenticated originals, another
    /// wallet's anchor, the currently held anchor, sequence overflow and unavailable entropy.
    pub fn wallet_refresh_time_anchor_witness(
        &self,
        prepared: &PreparedOperation,
        head: &VerifiedWalletHead,
        anchor_original: &[u8],
        certificate_original: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<ExactOperationWitness> {
        self.require_minimum_memory(parameters, full::CONTROL)?;
        let source = checked_source(prepared, head, parameters)?;
        let p = &parameters.inner;
        refresh_witness(
            head.credential(),
            source,
            (&p.scheme, &p.relation, &p.root),
            anchor_original,
            certificate_original,
            |previous| super::funding::fresh_state_nonce(Some(previous)),
        )
    }
}

/// The RefreshTimeAnchor after the checked opening. `nonce` receives the predecessor nonce
/// only after both originals authenticate and the anchor differs from the held one.
fn refresh_witness(
    c: &Credential,
    (before, rest): (&[Fp; 30], &[Fp; 6]),
    (scheme, relation, root): (&[u8; 32], &[u8; 32], &p256::Point),
    anchor_original: &[u8],
    certificate_original: &[u8],
    nonce: impl FnOnce(Fp) -> Result<Fp>,
) -> Result<ExactOperationWitness> {
    use crate::time_anchor::{self, Authority, TimeAnchor};
    if anchor_original.len() != time_anchor::BODY_BYTES + time_anchor::SIGNATURE_BYTES {
        return Err(Error::Malformed("TimeAnchor original length"));
    }
    let (body, signature) = anchor_original.split_at(time_anchor::BODY_BYTES);
    let authority = Authority {
        certificate: read_certificate(certificate_original)?,
        anchor: TimeAnchor {
            body: body.try_into().expect("checked TimeAnchor body"),
            signature: signature.try_into().expect("checked TimeAnchor signature"),
        },
    };
    let wallet: [u8; 32] = c.body[66..98].try_into().expect("fixed credential wallet");
    let verified = time_anchor::authenticate(&authority, scheme, &wallet, root)
        .map_err(Error::AuthorityMismatch)?;
    if verified.digest == rest[5] {
        return Err(Error::AuthorityMismatch("time anchor must differ from the held one"));
    }
    let fresh = nonce(before[29])?;
    if fresh == before[29] {
        return Err(Error::Unavailable("fresh state nonce"));
    }
    let (after, after_rest) =
        time_anchor::refresh(before, rest, &verified, fresh).map_err(Error::AuthorityMismatch)?;
    let s = time_anchor::statement(
        super::funding::limbs(relation),
        (before, rest),
        (&after, &after_rest),
        verified.digest,
    );
    let operation = op_id(c, time_anchor::REFRESH_TAG, verified.digest);
    let unsigned = super::funding::unsigned_receipt(c, &s, operation);
    Ok(ExactOperationWitness::RefreshTimeAnchor(Box::new(
        crate::control::Witness::refresh(
            *before,
            *rest,
            after,
            after_rest,
            c.clone(),
            s,
            unsigned,
            authority,
        ),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::kagemusha::KagemushaWalletIndexedTreeV1;
    struct OneMap(KagemushaWalletIndexedTreeV1);
    impl SpartanWalletMaps for OneMap {
        fn member_or_low(
            &mut self,
            which: SpartanWalletMap,
            root: [u8; 32],
            key: [u8; 32],
        ) -> Result<(PublicLeaf, PublicPath)> {
            if which != SpartanWalletMap::Credit || self.0.root() != root {
                return Err(Error::AuthorityMismatch("test selected root"));
            }
            self.0
                .membership(&key)
                .or_else(|_| self.0.non_membership(&key))
                .map_err(|e| Error::Rejected(e.to_string()))
        }
        fn insert(
            &mut self,
            which: SpartanWalletMap,
            root: [u8; 32],
            key: [u8; 32],
            value: [u8; 32],
        ) -> Result<PublicInsert> {
            if which != SpartanWalletMap::Credit || self.0.root() != root {
                return Err(Error::AuthorityMismatch("test selected root"));
            }
            self.0
                .insert(key, value)
                .map_err(|e| Error::Rejected(e.to_string()))
        }
    }
    #[test]
    fn wallet_owned_map_staging_uses_genuine_intermediate_paths_beyond_empty() {
        let mut store = OneMap(KagemushaWalletIndexedTreeV1::new());
        let empty = field(&store.0.root()).unwrap();
        assert_eq!(
            empty,
            indexed_map::empty_insert::<32>(Fp::ONE, Fp::ONE).0.old_root
        );
        let mut root = empty;
        for (key, value) in [(7, 81), (4, 82), (12, 83), (5, 84)] {
            let (old, root_before) = (Fp::from(key), root);
            let absent = search(&mut store, SpartanWalletMap::Credit, root_before, old).unwrap();
            assert_ne!(absent.leaf.key, old);
            let (opening, next) = insert(
                &mut store,
                SpartanWalletMap::Credit,
                root,
                old,
                Fp::from(value),
            )
            .unwrap();
            assert_eq!(opening.old_root, root_before);
            root = next;
            assert_eq!(root.to_repr(), store.0.root());
            let member = search(&mut store, SpartanWalletMap::Credit, root, old).unwrap();
            assert_eq!(member.leaf.value, Fp::from(value));
            assert!(insert(&mut store, SpartanWalletMap::Credit, root, old, Fp::ONE).is_err());
        }
        assert!(search(&mut store, SpartanWalletMap::Credit, empty, Fp::from(7)).is_err());
        assert!(search(&mut store, SpartanWalletMap::Consumed, root, Fp::from(7)).is_err());
    }

    #[test]
    fn signed_public_time_anchor_refresh_is_exact_and_authenticates_before_entropy() {
        use crate::{
            certificate::{self, Certificate, Role},
            full::WalletKey,
            ipa::Gens,
            operation_auth::tests as harness,
            time_anchor::{self, TimeAnchor},
        };
        use iroha_pasta::Eq as Vesta;
        let key = WalletKey::labelled("public TimeAnchor refresh wallet");
        let (ctx, boot) = bootstrap::fixture(&key, &Gens::<Vesta>::new(4096));
        let scheme = ctx.root.scheme;
        let wallet: [u8; 32] = boot.credential.body[66..98].try_into().unwrap();
        let root = BigUint::from_bytes_be(&crate::native::engine::TEST_ROOT_SECRET);
        let d = p256::scalar_below("public TimeAnchor refresh signer", &p256::n());
        let q = p256::mul(&p256::generator(), &d).unwrap();
        let issue = |role: Role| certificate::issue_fixture(&scheme, role as u8, &q, 9, &root);
        let original = |c: &Certificate| [c.body.as_slice(), c.signature.as_slice()].concat();
        let signed = |owner: &[u8; 32], time: u64, c: &Certificate| {
            let mut a = TimeAnchor {
                body: [0; time_anchor::BODY_BYTES],
                signature: [0; time_anchor::SIGNATURE_BYTES],
            };
            a.body[..2].copy_from_slice(&1u16.to_le_bytes());
            a.body[2..34].copy_from_slice(&scheme);
            a.body[34..66].copy_from_slice(owner);
            a.body[66..98].fill(0x4e);
            a.body[98..106].copy_from_slice(&time.to_le_bytes());
            a.body[106..].copy_from_slice(&certificate::certificate_digest(c).to_repr());
            a.signature = bootstrap::sign(time_anchor::message(&a), &d, "public TimeAnchor");
            [a.body.as_slice(), a.signature.as_slice()].concat()
        };
        let asked = std::cell::Cell::new(0);
        let build = |anchor: &[u8], issuer: &[u8], rest: &[Fp; 6]| {
            refresh_witness(
                &boot.credential,
                (&boot.core, rest),
                (&scheme, &ctx.relation, &ctx.root.root),
                anchor,
                issuer,
                |previous| {
                    asked.set(asked.get() + 1);
                    Ok(previous + Fp::ONE)
                },
            )
        };
        let time = 1_763_400_000_000;
        let cert = issue(Role::TimeAnchor);
        let issuer = original(&cert);
        let anchor = signed(&wallet, time, &cert);
        let built = build(&anchor, &issuer, &boot.rest).unwrap();
        let ExactOperationWitness::RefreshTimeAnchor(step) = &built else {
            unreachable!()
        };
        let digest = time_anchor::digest(&step.authority.anchor);
        assert_eq!(asked.get(), 1);
        assert_eq!(step.mode, crate::control::Mode::RefreshTimeAnchor);
        assert_eq!(step.authority.anchor.body[..], anchor[..time_anchor::BODY_BYTES]);
        assert_eq!(step.after_core[11], boot.core[11] + Fp::ONE);
        assert_eq!(step.after_core[28], Fp::from(time));
        assert_eq!(step.after_core[29], boot.core[29] + Fp::ONE);
        assert_eq!(step.after_rest[5], digest);
        for i in (0..30).filter(|i| !time_anchor::CHANGED_CORE.contains(i)) {
            assert_eq!(step.after_core[i], boot.core[i], "preserved Core {i}");
        }
        assert_eq!(step.after_rest[..5], boot.rest[..5]);
        let s = &step.statement;
        assert_eq!(
            (s[16], s[17], s[18], s[19]),
            (Fp::from(7), Fp::from(4), digest, Fp::from(time))
        );
        let operation = poseidon::hash_with_domain(
            u64::from_le_bytes(*b"kgwopid1"),
            &[boot.core[5], boot.core[6], Fp::from(7), digest],
        );
        assert_eq!(step.receipt.body[114..146], operation.to_repr());
        assert!(step.receipt.body[242..].iter().all(|b| *b == 0));
        assert!(step.receipt.signature.iter().all(|b| *b == 0));
        // The Control preparation relation `Engine::prepare` checks holds for it.
        let mut input = (**step).clone();
        input.receipt.body[242..274].copy_from_slice(&Fp::from(987).to_repr());
        input.receipt.body[274..306].fill(0x55);
        let mut w = full::Wiring::default();
        let family = w.family(0, 1);
        let prior = harness::predecessor(&mut w, family, bootstrap::public_profile(&ctx, &boot));
        let prepared =
            crate::control::append_preparation(&mut w, family, &ctx.root.root, &input, &prior)
                .unwrap();
        harness::expose(&mut w, family, 4, &[prepared.head(), prepared.body().message()]);
        assert!(harness::holds(&harness::finish(w, 16)));
        // A retry keeps the operation identity custody binds for exact replay.
        let again = build(&anchor, &issuer, &boot.rest).unwrap();
        let ExactOperationWitness::RefreshTimeAnchor(retry) = &again else {
            unreachable!()
        };
        assert_eq!(retry.receipt.body[114..146], step.receipt.body[114..146]);
        // Every refusal precedes entropy and leaves nothing to retain.
        let asked_before = asked.get();
        let mut held = boot.rest;
        held[5] = digest;
        let mut tampered = anchor.clone();
        tampered[time_anchor::BODY_BYTES] ^= 1;
        let foreign = signed(&[0x77; 32], time, &cert);
        let enrollment = issue(Role::Enrollment);
        let wrong_role = signed(&wallet, time, &enrollment);
        let wrong_issuer = original(&enrollment);
        let truncated = anchor[..anchor.len() - 1].to_vec();
        let short_issuer = issuer[..issuer.len() - 1].to_vec();
        for (name, a, c, rest) in [
            ("held anchor", &anchor, &issuer, &held),
            ("tampered signature", &tampered, &issuer, &boot.rest),
            ("another wallet", &foreign, &issuer, &boot.rest),
            ("Enrollment-role signer", &wrong_role, &wrong_issuer, &boot.rest),
            ("truncated anchor", &truncated, &issuer, &boot.rest),
            ("truncated issuer", &anchor, &short_issuer, &boot.rest),
        ] {
            assert!(build(a, c, rest).is_err(), "{name}");
        }
        assert_eq!(asked.get(), asked_before);
    }
}
