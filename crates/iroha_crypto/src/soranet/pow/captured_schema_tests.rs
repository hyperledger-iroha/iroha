//! Compiler-captured identities for this module’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::Ticket>(
        "iroha_crypto::soranet::pow::Ticket",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SignedTicket>(
        "iroha_crypto::soranet::pow::SignedTicket",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::TicketRevocationSnapshot>(
        "iroha_crypto::soranet::pow::TicketRevocationSnapshot",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::TicketRevocationSnapshotEntry>(
        "iroha_crypto::soranet::pow::TicketRevocationSnapshotEntry",
    );
}
