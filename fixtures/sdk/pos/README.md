# Signed POS example fixture

`manifest_v1.json` is shared by Swift and Android example unit tests. Its
operator is the public RFC8032 testvector1 key; all data is synthetic. It does
not establish device or monetary-authority qualification.

The envelope contains exactly `operator_signature` (128 lowercase hex digits)
and `payload_base64` (canonical padded base64), sorted in compact ASCII JSON
with one final LF. Its signature covers the exact payload bytes. That payload
uses schema `iroha.example.pos-manifest.v1` and compact, sorted UTF-8 typed JSON
without slash escaping or a final LF. All displayed fields come from it.
Unknown/duplicate fields, alternate number spellings and unsigned outer fields
are rejected. The operator is a canonical single Ed25519 I105 account.
