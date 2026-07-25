//! Common finalized-chain metadata for authoritative SoraFS query pages.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::ChainId;

/// Chain identity and ledger time shared by every record in a finalized query page.
///
/// Domain-specific cursors continue to carry the exact finalized height and block
/// hash. This context prevents otherwise well-formed pages from being replayed
/// across chains and gives consumers the authoritative block time without relying
/// on a daemon-local clock.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SorafsFinalizedPageContextV1 {
    /// Chain identifier bound to the immutable state view.
    pub chain_id: ChainId,
    /// Creation time of the finalized anchor block in Unix milliseconds.
    pub finalized_at_unix_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalized_page_context_has_canonical_norito_and_json_round_trips() {
        let context = SorafsFinalizedPageContextV1 {
            chain_id: ChainId::from("sorafs-reference-v1"),
            finalized_at_unix_ms: 1_717_171_717_000,
        };

        let encoded = norito::to_bytes(&context).expect("encode finalized page context");
        let decoded: SorafsFinalizedPageContextV1 =
            norito::decode_from_bytes(&encoded).expect("decode finalized page context");
        assert_eq!(decoded, context);
        assert_eq!(
            norito::to_bytes(&decoded).expect("re-encode finalized page context"),
            encoded
        );

        #[cfg(feature = "json")]
        {
            let json =
                norito::json::to_vec(&context).expect("encode finalized page context as JSON");
            let decoded: SorafsFinalizedPageContextV1 =
                norito::json::from_slice(&json).expect("decode finalized page context JSON");
            assert_eq!(decoded, context);
            assert_eq!(
                norito::json::to_vec(&decoded).expect("re-encode finalized page context JSON"),
                json
            );
        }
    }
}
