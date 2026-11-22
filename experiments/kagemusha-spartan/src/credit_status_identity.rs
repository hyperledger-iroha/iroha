//! Wire-v1 object identity of an already decided folded CreditStatus.
use super::*;
use crate::native::funding::uint;
use iroha_data_model::kagemusha::{
    KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaWalletCreditOpeningV1,
    KagemushaWalletCreditStatusV1, KagemushaWalletEffectV1, KagemushaWalletLifecycleV1,
    KagemushaWalletLineagePublicV1, KagemushaWalletLineageV1, KagemushaWalletReceiptV1,
    KagemushaWalletStateCommitmentV1, KagemushaWalletStatementV1,
};

fn lifecycle(value: Fp) -> Result<KagemushaWalletLifecycleV1> {
    match uint(value)? {
        1 => Ok(KagemushaWalletLifecycleV1::Active),
        2 => Ok(KagemushaWalletLifecycleV1::Retiring),
        _ => Err(Error::Malformed("CreditStatus lifecycle")),
    }
}

fn integer<T: TryFrom<u128>>(value: Fp) -> Result<T> {
    uint(value)?
        .try_into()
        .map_err(|_| Error::Malformed("CreditStatus integer width"))
}

fn commitment(value: Fp) -> KagemushaWalletStateCommitmentV1 {
    KagemushaWalletStateCommitmentV1 { value: value.to_repr() }
}

impl VerifiedSentDelivery<'_> {
    /// Hash the wire object's exact statement, proof digest, receipt, lineage and opening.
    /// Both compact transports restore the selected public words and original proof bytes.
    pub(crate) fn status_digest(&self) -> Result<[u8; 32]> {
        self.wire_status()?.credit_status_digest()
            .map_err(|_| Error::AuthorityMismatch("verified CreditStatus wire identity"))
    }

    pub(in crate::native) fn wire_status(&self) -> Result<KagemushaWalletCreditStatusV1> {
        self.receiver_evidence().wire_status(
            self.payment().statement()[17], self.payment().digest(), self.outcome(),
        )
    }
}

impl crate::native::sent_delivery::ReceiverEvidence {
    pub(in crate::native) fn wire_status(
        &self, credit: Fp, payment: Fp, outcome: VerifiedCreditStatus,
    ) -> Result<KagemushaWalletCreditStatusV1> {
        let e = self;
        let s = &e.statement;
        let context = &e.profile[bc::CONTEXT_IO..bc::CONTEXT_IO + bc::FIELDS];
        let lineage = &e.profile[bc::LINEAGE_IO..bc::LINEAGE_IO + bc::LINEAGE_FIELDS];
        let effect = match integer::<u8>(s[16])? {
            3 => KagemushaWalletEffectV1::Send {
                credit_id: s[17].to_repr(),
                receiver_wallet_id: identity(&s[18..20])?,
                send_ordinal: integer(s[20])?,
                amount: integer(s[21])?,
                fee: integer(s[22])?,
                request: s[23].to_repr(),
                accepted_lower_ms: integer(s[24])?,
                accepted_upper_ms: integer(s[25])?,
            },
            4 => KagemushaWalletEffectV1::Receive {
                credit_id: s[17].to_repr(),
                payer_wallet_id: identity(&s[18..20])?,
                amount: integer(s[20])?,
            },
            _ => return Err(Error::Malformed("CreditStatus head kind")),
        };
        let statement = KagemushaWalletStatementV1 {
            version: integer(s[0])?,
            scheme_id: identity(&s[3..5])?,
            relation_id: identity(&s[1..3])?,
            credential_digest: s[7].to_repr(),
            asset_digest: identity(&s[5..7])?,
            lifecycle: lifecycle(s[8])?,
            sequence: integer(s[9])?,
            next_load: integer(s[10])?,
            enabled_controls: integer(s[11])?,
            lineage_burned_total: integer(s[12])?,
            lineage_pending_outgoing_root: s[13].to_repr(),
            predecessor: commitment(s[14]),
            successor: commitment(s[15]),
            effect,
        };
        let body = &e.receipt.body;
        let receipt = KagemushaWalletReceiptV1 {
            version: u16::from_le_bytes(body[..2].try_into().expect("fixed receipt version")),
            operation_id: body[114..146].try_into().expect("fixed operation id"),
            capsule_commitment: body[274..306].try_into().expect("fixed capsule commitment"),
            payment_digest: body[306..338].try_into().expect("fixed Payment digest"),
            signature: KagemushaDeviceSignatureV1::from_raw_bytes(&e.receipt.signature)
                .map_err(|_| Error::Malformed("CreditStatus receipt signature"))?,
        };
        let public = KagemushaWalletLineagePublicV1 {
            version: 1,
            scheme_id: identity(&context[bc::SCHEME..bc::SCHEME + 2])?,
            relation_id: identity(&context[bc::RELATION..bc::RELATION + 2])?,
            head: commitment(e.profile[full::HEAD_IO]),
            wallet_id: identity(&context[bc::WALLET..bc::WALLET + 2])?,
            credential_digest: context[bc::CREDENTIAL].to_repr(),
            payment_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(&e.credential.body[130..195])
                .map_err(|_| Error::Malformed("CreditStatus payment key"))?,
            lifecycle: lifecycle(lineage[bc::LIFECYCLE])?,
            policy_epoch: integer(lineage[bc::POLICY_EPOCH])?,
            enabled_controls: integer(lineage[bc::ENABLED_CONTROLS])?,
            burned_total: integer(lineage[bc::BURNED_TOTAL])?,
            pending_outgoing_root: lineage[bc::PENDING_ROOT].to_repr(),
            credit_digest_root: lineage[bc::CREDIT_ROOT].to_repr(),
        };
        Ok(KagemushaWalletCreditStatusV1 {
            version: 1,
            statement,
            proof_digest: body[242..274].try_into().expect("fixed proof digest"),
            receipt,
            lineage: KagemushaWalletLineageV1 { public, proof: e.exact_pair.clone() },
            opening: KagemushaWalletCreditOpeningV1 {
                credit_id: credit.to_repr(),
                payment_digest: payment.to_repr(),
                burned: outcome == VerifiedCreditStatus::Burned,
                next_key: e.next.to_repr(),
                slot: e.slot,
                siblings: e.siblings.iter().flat_map(|word| word.to_repr()).collect(),
            },
        })
    }
}
