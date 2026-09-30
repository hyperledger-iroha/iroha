# SoraFS readiness fixture verification review

The previous exact test source is retained as `.py.txt`; [its SHA-256 record](preimage.json) identifies the removed module-global, unbounded cache of signature verification verdicts. The current fixture keeps the bounded memo of immutable deterministic public keys, every real signature, and the actual maintained `release_evidence_crypto.verify_ed25519` function for every evidence check. A positive and changed-message negative control repeats through that exact production owner. No production verifier, receipt/frame custody, evidence mutation or admission assertion is changed.

The already-loaded complete script run retains its own prior module state and is allowed to finish naturally. The final current-source complete run must exercise the restored direct verifier.
