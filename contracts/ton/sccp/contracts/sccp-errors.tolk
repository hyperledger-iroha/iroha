// Exit codes of the SCCP v1 TON contracts.
//
// Names mirror the Solidity custom errors of §5.2.2 where a counterpart
// exists; the remaining codes cover TON-only rules of §5.3. The numeric
// values are part of the tested surface (`tests/unit/*.test.tolk`).

enum SccpErrors {
    /// TEP-74 wallet balance too small.
    BalanceError = 47
    /// Attached value is below the required value (§5.3.5).
    NotEnoughValue = 48
    /// Unknown or malformed message.
    InvalidMessage = 49
    /// TEP-74 sender is not the wallet owner.
    NotOwner = 73
    /// Sender is not the canonical wallet of the claimed owner.
    NotValidWallet = 74
    /// Address is not `addr_std` in workchain 0 without anycast.
    WrongWorkchain = 333

    /// `GLOBALID` differs from TON mainnet.
    WrongChain = 400
    /// Finalization refused while the Parliament pause is set.
    MintingIsPaused = 401
    /// Attestation roster is neither the current nor the graced previous roster.
    RosterNotAccepted = 402
    /// Roster preimage violates §3.7 (n, t, ordering, digest).
    BadRoster = 403
    /// Roster validity bounds of §5.1.5 are violated.
    BadRosterValidity = 404
    /// A signature is malformed or recovers to the wrong member.
    BadSignatures = 405
    /// Fewer than `t` valid signatures.
    TooFewSignatures = 406
    /// Merkle or history path does not reproduce the attested root.
    BadProof = 407
    /// Payload violates §3.2 or does not target this deployment.
    BadPayload = 408
    /// Cap check of §5.1.4 failed.
    SupplyCapExceeded = 410
    /// Rotation rules of §5.1.5 are violated.
    BadRotation = 411
    /// Recipient is not a valid account for this deployment.
    BadRecipient = 412
    /// Amount is zero or out of range.
    BadAmount = 413
    /// Finalization after `deadline_ms`.
    DeadlinePassed = 414
    /// Void before `deadline_ms`.
    DeadlineNotReached = 415
    /// `voidFrozen` while a roster is still valid.
    NotFrozen = 416
    /// Control nonce not strictly above the stored one.
    StaleControl = 417
    /// Operation before `sccp_init`.
    NotInitialized = 418
    /// Second `sccp_init`.
    AlreadyInitialized = 419
    /// Attestation statement violates the §3.6.1 invariants.
    BadAttestation = 420
    /// Snake cell or chunked list is not in its canonical shape.
    BadSnake = 421
    /// Bucket index is more than `SCCP_MAX_DEPLOY_AHEAD` past the deployed ones.
    BucketNotDeployed = 422
    /// Sender is not the minter.
    NotMinter = 423
    /// Sender is not the canonical bucket.
    NotBucket = 424
    /// Nonce argument mismatch or nonce outside this bucket.
    BadNonce = 425
    /// Bucket range or deployment count out of bounds.
    BadRange = 426
    /// Burn custom payload is missing or is not `sccp_burn_to_taira`.
    BadBurnPayload = 427
    /// Stored configuration or initial data is not canonical.
    BadConfig = 429
}
