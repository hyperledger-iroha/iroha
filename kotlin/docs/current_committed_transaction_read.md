# Current wallet-self committed-transaction read

`CommittedTransactionInclusionBridge` is the Kotlin/Android SDK entry point. The
same native C functions are used by `CommittedTransactionQueryV1` and
`CommittedTransactionInclusionV1` in Swift. This first-release API accepts only
the current protocol-4 selective `CommittedTransaction` and v2 bridge finality
bundle; it has no retired query or status fallback.

1. Load `NetworkId` and a finality checkpoint `(block_height,
   height_context_id, block_hash)` from independently approved application
   configuration. The initial bundle of a verification chain must be the
   checkpoint-height bundle whose context id matches that pin; later bundles
   must be consecutive. A pin for the *next* height context may instead begin
   at that next height. A Torii response must never choose the trust anchor.
2. Generate and durably reserve a fresh 32-byte nonce. Build
   `committedTransactionQueryPayloadHash(networkId, walletAccountId,
   transactionHash, creationTimeMs, nonce)`, sign its 32 bytes with the wallet
   controller's Ed25519 key, and call `finalizeCommittedTransactionQuery` with
   the same arguments. The native query has exactly the wallet authority and
   entrypoint-hash predicates, default selector and parameters, and a fixed
   100-second lifetime. It rejects another account's signature.
3. POST the versioned signed query once to `/v1/query` with both `Content-Type`
   and `Accept` set to `application/x-norito`. Keep application dataspace and
   tenant headers pinned independently. Do not automatically retry this
   nonce-bearing request or follow a redirect. Treat an ambiguous response as
   pending and create a new signed query only after durable reconciliation.
   `HttpClientTransport.postSignedCommittedTransactionQuery` enforces HTTPS,
   the configured URL prefix, one-shot dispatch, exact response URL, and a
   bounded raw Norito response.
4. `candidateBlockHash(responseBytes, transactionHash)` returns `null` for a
   canonical empty committed-transaction page, which leaves the operation
   pending. A one-row response yields a bounded, **untrusted** routing hint. Fetch
   `/v1/bridge/finality/bundle/{height}` from the checkpoint height forward,
   with at most 4096 bundles and 16 MiB of JSON in total, until the carrier
   block hash matches. A 404 or a limit reached leaves the operation pending;
   no unverified row may be committed. This scan does not rely on public status
   or a second signed query. `HttpClientTransport.getBridgeFinalityBundleJson`
   fetches each exact HTTPS JSON bundle; it does not make that bundle trusted.
5. Call `verify(responseBytes, finalityBundleChainJson, networkId,
   trustedHeightContextId, transactionHash)`. The native verifier checks the
   independent network and context, every linked four-validator finality
   bundle, the exact transaction, and both input and full output Merkle
   commitments. Use `canonicalRowBytes` (or `canonicalRowHex`) only after this
   call succeeds. `resultOk=false` is authenticated rejection evidence, not a
   successful application operation. Compare `outputHashBytes` with the
   eventual server readback.

The SDK exposes byte-level signing and verification. A wallet application must
provide its authenticated Torii transport, checkpoint deployment pin, durable
nonce store, device signer, and retry journal. Until those are configured, the
wallet operation remains pending.
