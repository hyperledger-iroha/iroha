# SCCP TAIRA Contracts

This directory contains the TAIRA-side source executable for the
`taira_tron_xor` route.

`TairaXorSccpBurnRecord.ko` burns configured XOR from the transaction authority
and queues a prebuilt `RecordSccpMessage` instruction. Compile it with the IVM
ZK mode bit forced and submit the derived payload as `Executable::IvmProved`.
Core admission rejects record-only overlays for this route: every recorded
TAIRA -> TRON XOR message must be paired with enough same-overlay, whole-unit
XOR burn from the payload sender.
