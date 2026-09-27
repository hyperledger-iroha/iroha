// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.31;

/// @title SccpTairaXor
/// @notice SCCP v1 destination for Taira XOR on Ethereum, BSC and TRON
/// (`specs/sccp.md` §5.1 and §5.2). The contract is the ERC-20/BEP-20/TRC-20
/// token itself with the bridge built in. It has no owner, guardian, admin
/// key, setter, upgrade path or external call. Taira-to-destination messages
/// are authorized by bridge-roster attestations over committed Taira blocks;
/// the minting pause changes only through `applyControl` with a Parliament
/// control leaf of an attested Taira block.
/// @dev Compiled by solc 0.8.31 (ETH/BSC) and tronprotocol tv_0.8.31 (TRON)
/// with the legacy pipeline and `evmVersion: cancun` (§5.5). The source
/// avoids the 0.8.31 legacy-pipeline bug patterns: it never deletes memory
/// `bytes` elements and declares no custom storage layout.
contract SccpTairaXor {
    // ---------------------------------------------------------------------
    // ABI structures (§5.2.2)
    // ---------------------------------------------------------------------

    /// @notice Bridge attestation statement over one committed Taira block (§3.6.1).
    struct AttestationV1 {
        uint64 height;
        uint64 epoch;
        uint64 timestampMs;
        bytes32 blockHash;
        bytes32 sccpRoot;
        uint32 messageCount;
        bytes32 historyRoot;
        uint64 historySize;
        bytes32 rosterDigest;
        bytes32 nextRosterDigest;
    }

    /// @notice Bridge roster generation; `members` packs `n` 20-byte addresses in §3.7 order.
    struct RosterV1 {
        uint64 generation;
        uint64 validFromMs;
        uint64 validUntilMs;
        uint8 threshold;
        bytes members;
    }

    /// @notice Signature set: bit `i` addresses roster member `i`; 65 bytes per set bit (§3.8).
    struct SignaturesV1 {
        uint32 signerBitmap;
        bytes signatures;
    }

    /// @notice Transfer payload with its position in the block commitment tree (§3.4).
    struct MessageProofV1 {
        bytes payload;
        uint32 leafIndex;
        bytes32[] path;
    }

    /// @notice Older SCCP-bearing block proven against the attested history root (§3.5).
    struct HistoryProofV1 {
        uint64 height;
        bytes32 sccpRoot;
        uint32 messageCount;
        uint64 leafIndex;
        bytes32[] path;
    }

    /// @notice One roster rotation signed by the current roster (§5.1.5).
    struct RotationV1 {
        AttestationV1 attestation;
        RosterV1 current;
        SignaturesV1 signatures;
        RosterV1 next;
    }

    /// @notice Parliament control leaf fields with its position in the commitment tree (§5.1.6).
    struct ControlProofV1 {
        uint64 controlNonce;
        bool paused;
        uint32 leafIndex;
        bytes32[] path;
    }

    // ---------------------------------------------------------------------
    // Events and errors (§5.2.2)
    // ---------------------------------------------------------------------

    /// @notice ERC-20 transfer; mints come from and burns go to the zero address.
    event Transfer(address indexed from, address indexed to, uint256 value);
    /// @notice ERC-20 allowance change.
    event Approval(address indexed owner, address indexed spender, uint256 value);
    /// @notice Inbound source event consumed by Taira (§4.12.2).
    event SccpTransferToTaira(bytes32 indexed messageId, address indexed sender, uint64 nonce, bytes payload);
    /// @notice A Taira transfer minted on this destination.
    event SccpFinalized(bytes32 indexed messageId, uint64 indexed nonce, address indexed recipient, uint256 tokenAmount);
    /// @notice A Taira transfer nonce voided without minting; `messageId` is zero for `voidFrozen`.
    event SccpVoided(bytes32 indexed messageId, uint64 indexed nonce);
    /// @notice A new roster generation was installed.
    event SccpRosterRotated(uint64 indexed generation, bytes32 digest, uint64 validUntilMs);
    /// @notice A Parliament control leaf was applied.
    event SccpControlApplied(uint64 indexed controlNonce, bool paused);

    /// @notice The chain, network tag or Taira network identity is not this deployment's.
    error WrongChain();
    /// @notice Finalization is refused while the Parliament pause is applied.
    error MintingIsPaused();
    /// @notice The attestation is not signed by the current or the previous unexpired roster.
    error RosterNotAccepted();
    /// @notice The supplied roster violates §3.7 or does not hash to the claimed digest.
    error BadRoster();
    /// @notice The roster validity window violates the §5.1.5 bounds.
    error BadRosterValidity();
    /// @notice A signature, the signer bitmap or a recovered signer is invalid.
    error BadSignatures();
    /// @notice Fewer valid signatures than the roster threshold.
    error TooFewSignatures();
    /// @notice An attestation invariant or a Merkle or history path does not verify.
    error BadProof();
    /// @notice The transfer payload violates §3.2 for this destination.
    error BadPayload();
    /// @notice The outbound nonce is already consumed by a mint or a void.
    error AlreadyConsumed(uint64 nonce);
    /// @notice The mint would exceed the immutable supply cap.
    error SupplyCapExceeded();
    /// @notice The rotation does not chain from the current roster (§5.1.5).
    error BadRotation();
    /// @notice The payload recipient is not valid for this destination's codec.
    error BadRecipient();
    /// @notice An amount or a nonce count is outside its admitted range.
    error BadAmount();
    /// @notice The supplied nonce differs from the expected one.
    error BadNonce(uint64 expected);
    /// @notice TRON burns and voids require `msg.sender == tx.origin`.
    error DirectCallerRequired();
    /// @notice `transferToTaira` calldata is not the canonical ABI encoding (§5.1.7).
    error NonCanonicalCalldata();
    /// @notice The mint deadline of the message has passed.
    error DeadlinePassed();
    /// @notice The mint deadline of the message has not passed yet.
    error DeadlineNotReached();
    /// @notice The current or the previous roster has not expired.
    error NotFrozen();
    /// @notice The control nonce is not above the last applied control nonce.
    error StaleControl();
    /// @notice ERC-6093: the sender balance is below the requested amount.
    error ERC20InsufficientBalance(address sender, uint256 balance, uint256 needed);
    /// @notice ERC-6093: the allowance is below the requested amount.
    error ERC20InsufficientAllowance(address spender, uint256 allowance, uint256 needed);
    /// @notice ERC-6093: the token sender is the zero address.
    error ERC20InvalidSender(address sender);
    /// @notice ERC-6093: the token receiver is the zero address or this contract.
    error ERC20InvalidReceiver(address receiver);
    /// @notice ERC-6093: the approved spender is the zero address.
    error ERC20InvalidSpender(address spender);

    // ---------------------------------------------------------------------
    // Constants (§0, §3.6, §3.9)
    // ---------------------------------------------------------------------

    uint256 private constant SECP256K1_N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141;
    uint256 private constant SECP256K1_HALF_N = 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0;
    uint256 private constant ADDRESS_MASK = 0x00ffffffffffffffffffffffffffffffffffffffff;

    bytes32 private constant DOMAIN_TYPEHASH = keccak256("EIP712Domain(string name,string version,bytes32 salt)");
    bytes32 private constant NAME_HASH = keccak256("SCCP");
    bytes32 private constant VERSION_HASH = keccak256("1");
    bytes32 private constant ATTESTATION_TYPEHASH = keccak256(
        "SccpAttestation(uint64 height,uint64 epoch,uint64 timestampMs,bytes32 blockHash,bytes32 sccpRoot,"
        "uint32 messageCount,bytes32 historyRoot,uint64 historySize,bytes32 rosterDigest,bytes32 nextRosterDigest)"
    );

    uint64 private constant PREVIOUS_ROSTER_GRACE_MS = 86_400_000;
    uint64 private constant MAX_ROSTER_VALIDITY_MS = 2_592_000_000;
    uint64 private constant MAX_CLOCK_SKEW_MS = 3_600_000;

    uint8 private constant TAG_TAIRA = 0x40;
    uint8 private constant TAG_ETHEREUM = 0x41;
    uint8 private constant TAG_BSC = 0x42;
    uint8 private constant TAG_TRON = 0x43;
    uint256 private constant TRON_CHAIN_ID = 0x2b6653dc;

    uint256 private constant MIN_ROSTER_MEMBERS = 4;
    uint256 private constant MAX_ROSTER_MEMBERS = 31;
    uint256 private constant MAX_BLOCK_LEAVES = 512;
    uint256 private constant MAX_HISTORY_PATH = 32;
    uint256 private constant MAX_ROTATIONS_PER_CALL = 16;
    uint256 private constant MAX_VOID_FROZEN_RANGE = 256;
    uint256 private constant MAX_TAIRA_ACCOUNT_BYTES = 1024;
    uint256 private constant MAX_PAYLOAD_BYTES = 4096;
    uint256 private constant MIN_PAYLOAD_BYTES = 59;
    uint256 private constant PAYLOAD_KIND_TRANSFER = 0x02;
    uint256 private constant PAYLOAD_VERSION = 0x01;
    uint256 private constant CODEC_CANONICAL_TEXT = 1;
    uint256 private constant CODEC_EVM_ADDRESS20 = 2;
    uint256 private constant CODEC_TAIRA_ACCOUNT = 3;
    uint256 private constant CODEC_TRON_ADDRESS21 = 5;
    uint256 private constant TRON_ADDRESS_PREFIX = 0x41;
    uint256 private constant ASSET_ID_XOR = 0x786f72; // "xor"
    uint256 private constant CANONICAL_TRANSFER_HEAD = 0x84; // 4 + 0x80

    // ---------------------------------------------------------------------
    // Storage (§5.2.3); declaration order fixes the packed slots A..D.
    // ---------------------------------------------------------------------

    bytes32 private rosterDigest; // slot A
    uint64 private rosterGeneration; // slot B
    uint64 private rosterValidUntilMs; // slot B
    /// @notice Nonce of the last applied Parliament control, 0 initially.
    uint64 public controlNonce; // slot B
    /// @notice Minting pause set only by applied Parliament controls.
    bool public mintingPaused; // slot B
    bytes32 private prevRosterDigest; // slot C
    uint64 private prevRosterValidUntilMs; // slot D
    /// @notice Monotone count of finalizations, voids, burns and applied controls (not rotations).
    uint64 public opCount; // slot D
    mapping(uint256 => uint256) private consumedBitmap; // word = nonce >> 8, bit = nonce & 255
    /// @notice Next outbound source nonce per burning sender.
    mapping(address => uint64) public transferNonces;
    /// @notice ERC-20 total supply in Taira units (9 decimals).
    uint256 public totalSupply;
    /// @notice ERC-20 balances.
    mapping(address => uint256) public balanceOf;
    /// @notice ERC-20 allowances.
    mapping(address => mapping(address => uint256)) public allowance;

    bytes32 private immutable INITIAL_ROSTER_DIGEST;
    uint64 private immutable INITIAL_GENERATION;
    bytes32 private immutable TAIRA_NETWORK_ID;
    uint8 private immutable NETWORK_TAG;
    uint32 private immutable ROUTE_REVISION;
    uint256 private immutable MAX_WRAPPED_SUPPLY;
    bytes32 private immutable DOMAIN_SEPARATOR;
    bool private immutable REQUIRE_DIRECT_CALLER;

    // ---------------------------------------------------------------------
    // Constructor (§5.2.1)
    // ---------------------------------------------------------------------

    /// @param tairaNetworkId_ Taira `NetworkId` (nonzero), the EIP-712 salt.
    /// @param networkTag_ Own profile tag: 0x41 Ethereum, 0x42 BSC, 0x43 TRON.
    /// @param routeRevision_ Registered route revision bound into every payload (nonzero).
    /// @param maxWrappedSupply_ Immutable supply cap in Taira units (nonzero, below 2^128).
    /// @param initialRoster Genuine Taira roster generation pinned by this deployment.
    constructor(
        bytes32 tairaNetworkId_,
        uint8 networkTag_,
        uint32 routeRevision_,
        uint256 maxWrappedSupply_,
        RosterV1 memory initialRoster
    ) {
        if (tairaNetworkId_ == bytes32(0)) revert WrongChain();
        if (networkTag_ != TAG_ETHEREUM && networkTag_ != TAG_BSC && networkTag_ != TAG_TRON) revert WrongChain();
        if (block.chainid != _identityWordOf(networkTag_)) revert WrongChain();
        if (routeRevision_ == 0) revert BadPayload();
        if (maxWrappedSupply_ == 0 || maxWrappedSupply_ >> 128 != 0) revert BadAmount();

        bytes32 digest = _initialRosterDigest(tairaNetworkId_, initialRoster);
        _checkRosterValidity(initialRoster.validFromMs, initialRoster.validUntilMs, block.timestamp * 1000);

        INITIAL_ROSTER_DIGEST = digest;
        INITIAL_GENERATION = initialRoster.generation;
        TAIRA_NETWORK_ID = tairaNetworkId_;
        NETWORK_TAG = networkTag_;
        ROUTE_REVISION = routeRevision_;
        MAX_WRAPPED_SUPPLY = maxWrappedSupply_;
        DOMAIN_SEPARATOR = keccak256(abi.encode(DOMAIN_TYPEHASH, NAME_HASH, VERSION_HASH, tairaNetworkId_));
        REQUIRE_DIRECT_CALLER = networkTag_ == TAG_TRON;

        rosterDigest = digest;
        rosterGeneration = initialRoster.generation;
        rosterValidUntilMs = initialRoster.validUntilMs;
    }

    // ---------------------------------------------------------------------
    // Taira -> destination (§5.1.3)
    // ---------------------------------------------------------------------

    /// @notice Mints a Taira transfer proven against an attestation of its own block.
    function finalizeFromTaira(
        AttestationV1 calldata attestation,
        RosterV1 calldata roster,
        SignaturesV1 calldata signatures,
        MessageProofV1 calldata proof
    ) external returns (bytes32 messageId) {
        _requireChain();
        if (mintingPaused) revert MintingIsPaused();
        _verifyAccepted(attestation, roster, signatures);
        (bytes32 id, uint64 nonce, address recipient, uint256 amount) =
            _verifyMessage(proof, attestation.sccpRoot, attestation.messageCount, true);
        _consume(nonce);
        _mint(recipient, amount);
        _bumpOpCount();
        emit SccpFinalized(id, nonce, recipient, amount);
        return id;
    }

    /// @notice Mints a Taira transfer of an older block proven through the attested history root.
    function finalizeFromTairaHistorical(
        AttestationV1 calldata attestation,
        RosterV1 calldata roster,
        SignaturesV1 calldata signatures,
        HistoryProofV1 calldata history,
        MessageProofV1 calldata proof
    ) external returns (bytes32) {
        _requireChain();
        if (mintingPaused) revert MintingIsPaused();
        _verifyAccepted(attestation, roster, signatures);
        (bytes32 id, uint64 nonce, address recipient, uint256 amount) =
            _verifyMessage(proof, history.sccpRoot, history.messageCount, true);
        _verifyHistory(attestation, history);
        _consume(nonce);
        _mint(recipient, amount);
        _bumpOpCount();
        emit SccpFinalized(id, nonce, recipient, amount);
        return id;
    }

    // ---------------------------------------------------------------------
    // Rotation (§5.1.5)
    // ---------------------------------------------------------------------

    /// @notice Applies up to 16 sequential roster rotations, each signed by the then-current roster.
    function rotateRosters(RotationV1[] calldata rotations) external {
        _requireChain();
        uint256 count = rotations.length;
        if (count == 0 || count > MAX_ROTATIONS_PER_CALL) revert BadRotation();
        uint256 nowMs = block.timestamp * 1000;
        for (uint256 i = 0; i < count; ++i) {
            _rotate(rotations[i], nowMs);
        }
    }

    // ---------------------------------------------------------------------
    // Parliament controls (§5.1.6)
    // ---------------------------------------------------------------------

    /// @notice Applies a Parliament control leaf of an attested block (not blocked by the pause).
    function applyControl(
        AttestationV1 calldata attestation,
        RosterV1 calldata roster,
        SignaturesV1 calldata signatures,
        ControlProofV1 calldata control
    ) external {
        _requireChain();
        _verifyAccepted(attestation, roster, signatures);
        _verifyControlLeaf(control, attestation.sccpRoot, attestation.messageCount);
        _applyControl(control);
    }

    /// @notice Applies a Parliament control leaf of an older block through the attested history root.
    function applyControlHistorical(
        AttestationV1 calldata attestation,
        RosterV1 calldata roster,
        SignaturesV1 calldata signatures,
        HistoryProofV1 calldata history,
        ControlProofV1 calldata control
    ) external {
        _requireChain();
        _verifyAccepted(attestation, roster, signatures);
        _verifyControlLeaf(control, history.sccpRoot, history.messageCount);
        _verifyHistory(attestation, history);
        _applyControl(control);
    }

    // ---------------------------------------------------------------------
    // Destination -> Taira (§5.1.7)
    // ---------------------------------------------------------------------

    /// @notice Burns `tokenAmount` from the caller and emits the inbound source event for Taira.
    /// @dev Only the canonical ABI encoding succeeds, because Taira proves TRON burns from calldata.
    function transferToTaira(bytes calldata tairaRecipient, uint256 tokenAmount, uint64 expectedNonce)
        external
        returns (bytes32 messageId)
    {
        _requireChain();
        _requireCanonicalTransferCalldata();
        if (tokenAmount == 0 || tokenAmount >> 128 != 0) revert BadAmount();
        if (REQUIRE_DIRECT_CALLER) _requireDirectCaller();
        address sender = msg.sender;
        uint64 nonce = transferNonces[sender];
        if (expectedNonce != nonce) revert BadNonce(nonce);
        transferNonces[sender] = nonce + 1;
        _burn(sender, tokenAmount);
        bytes memory payload = _inboundPayload(nonce, tokenAmount, tairaRecipient);
        messageId = _inboundMessageId(payload);
        _bumpOpCount();
        emit SccpTransferToTaira(messageId, sender, nonce, payload);
    }

    // ---------------------------------------------------------------------
    // Voids (§5.1.8)
    // ---------------------------------------------------------------------

    /// @notice Voids an attested Taira transfer after its deadline so that Taira refunds it.
    function voidExpired(
        uint64 nonce,
        AttestationV1 calldata attestation,
        RosterV1 calldata roster,
        SignaturesV1 calldata signatures,
        MessageProofV1 calldata proof
    ) external {
        _requireChain();
        if (REQUIRE_DIRECT_CALLER) _requireDirectCaller();
        _verifyAccepted(attestation, roster, signatures);
        (bytes32 id, uint64 payloadNonce,,) =
            _verifyMessage(proof, attestation.sccpRoot, attestation.messageCount, false);
        _void(id, nonce, payloadNonce);
    }

    /// @notice Voids an older attested Taira transfer after its deadline through the history root.
    function voidExpiredHistorical(
        uint64 nonce,
        AttestationV1 calldata attestation,
        RosterV1 calldata roster,
        SignaturesV1 calldata signatures,
        HistoryProofV1 calldata history,
        MessageProofV1 calldata proof
    ) external {
        _requireChain();
        if (REQUIRE_DIRECT_CALLER) _requireDirectCaller();
        _verifyAccepted(attestation, roster, signatures);
        (bytes32 id, uint64 payloadNonce,,) = _verifyMessage(proof, history.sccpRoot, history.messageCount, false);
        _verifyHistory(attestation, history);
        _void(id, nonce, payloadNonce);
    }

    /// @notice Voids up to 256 unconsumed nonces once both the current and the previous roster expired.
    function voidFrozen(uint64 firstNonce, uint64 count) external {
        _requireChain();
        if (REQUIRE_DIRECT_CALLER) _requireDirectCaller();
        uint256 first = firstNonce;
        if (count == 0 || count > MAX_VOID_FROZEN_RANGE || first + count > 1 << 64) revert BadAmount();
        uint256 nowMs = block.timestamp * 1000;
        if (nowMs <= rosterValidUntilMs || nowMs <= prevRosterValidUntilMs) revert NotFrozen();
        uint256 last = first + count - 1;
        _consumeRange(first, last);
        _bumpOpCount();
        // One `SccpVoided(0, nonce)` per nonce, emitted directly as LOG3 without data.
        bytes32 topic = SccpVoided.selector;
        assembly ("memory-safe") {
            for { let nonce := first } iszero(gt(nonce, last)) { nonce := add(nonce, 1) } {
                log3(0, 0, topic, 0, nonce)
            }
        }
    }

    // ---------------------------------------------------------------------
    // Views (§5.2.2)
    // ---------------------------------------------------------------------

    /// @notice Current and previous roster acceptance state.
    function rosterState()
        external
        view
        returns (bytes32 digest, uint64 generation, uint64 validUntilMs, bytes32 prevDigest, uint64 prevValidUntilMs)
    {
        return (rosterDigest, rosterGeneration, rosterValidUntilMs, prevRosterDigest, prevRosterValidUntilMs);
    }

    /// @notice Whether an outbound nonce is consumed by a mint or a void.
    function isConsumed(uint64 nonce) external view returns (bool) {
        return consumedBitmap[uint256(nonce) >> 8] & (uint256(1) << (uint256(nonce) & 0xff)) != 0;
    }

    /// @notice Taira `NetworkId` pinned by this deployment.
    function tairaNetworkId() external view returns (bytes32) {
        return TAIRA_NETWORK_ID;
    }

    /// @notice Route revision bound into every payload.
    function routeRevision() external view returns (uint32) {
        return ROUTE_REVISION;
    }

    /// @notice Immutable supply cap in Taira units.
    function maxWrappedSupply() external view returns (uint256) {
        return MAX_WRAPPED_SUPPLY;
    }

    /// @notice EIP-712 domain separator salted with the Taira `NetworkId` (§3.6).
    function domainSeparator() external view returns (bytes32) {
        return DOMAIN_SEPARATOR;
    }

    /// @notice Digest of the roster generation pinned at deployment.
    function initialRosterDigest() external view returns (bytes32) {
        return INITIAL_ROSTER_DIGEST;
    }

    /// @notice Generation of the roster pinned at deployment.
    function initialRosterGeneration() external view returns (uint64) {
        return INITIAL_GENERATION;
    }

    /// @notice Upper bound on any installed roster's validity window.
    function maxRosterValidityMs() external pure returns (uint64) {
        return MAX_ROSTER_VALIDITY_MS;
    }

    // ---------------------------------------------------------------------
    // ERC-20 surface
    // ---------------------------------------------------------------------

    /// @notice Token name.
    function name() external pure returns (string memory) {
        return "Taira XOR";
    }

    /// @notice Token symbol.
    function symbol() external pure returns (string memory) {
        return "tXOR";
    }

    /// @notice Token decimals; one token unit equals one Taira unit.
    function decimals() external pure returns (uint8) {
        return 9;
    }

    /// @notice ERC-20 transfer.
    function transfer(address to, uint256 value) external returns (bool) {
        _transfer(msg.sender, to, value);
        return true;
    }

    /// @notice ERC-20 transferFrom; an unlimited allowance is not decremented.
    function transferFrom(address from, address to, uint256 value) external returns (bool) {
        uint256 current = allowance[from][msg.sender];
        if (current != type(uint256).max) {
            if (current < value) revert ERC20InsufficientAllowance(msg.sender, current, value);
            unchecked {
                allowance[from][msg.sender] = current - value;
            }
        }
        _transfer(from, to, value);
        return true;
    }

    /// @notice ERC-20 approve.
    function approve(address spender, uint256 value) external returns (bool) {
        if (spender == address(0)) revert ERC20InvalidSpender(address(0));
        allowance[msg.sender][spender] = value;
        emit Approval(msg.sender, spender, value);
        return true;
    }

    // ---------------------------------------------------------------------
    // Attestation and roster verification (§3.6 - §3.8, §5.1.2)
    // ---------------------------------------------------------------------

    function _verifyAccepted(AttestationV1 calldata attestation, RosterV1 calldata roster, SignaturesV1 calldata signatures)
        private
        view
    {
        bytes32 claimed = attestation.rosterDigest;
        uint256 nowMs = block.timestamp * 1000;
        bool accepted = claimed != bytes32(0)
            && (
                (claimed == rosterDigest && nowMs <= rosterValidUntilMs)
                    || (claimed == prevRosterDigest && nowMs <= prevRosterValidUntilMs)
            );
        if (!accepted) revert RosterNotAccepted();
        _verifySigned(attestation, roster, signatures, claimed);
    }

    function _verifySigned(
        AttestationV1 calldata attestation,
        RosterV1 calldata roster,
        SignaturesV1 calldata signatures,
        bytes32 claimed
    ) private view {
        (bytes32 digest, uint256 members) = _rosterDigest(roster);
        if (digest != claimed) revert BadRoster();
        _checkAttestationInvariants(attestation);
        _verifySignatures(_attestationDigest(attestation), roster, members, signatures);
    }

    function _checkAttestationInvariants(AttestationV1 calldata attestation) private pure {
        uint256 messageCount = attestation.messageCount;
        if ((messageCount == 0) != (attestation.sccpRoot == bytes32(0))) revert BadProof();
        if ((attestation.historySize == 0) != (attestation.historyRoot == bytes32(0))) revert BadProof();
        if (messageCount > MAX_BLOCK_LEAVES) revert BadProof();
    }

    /// @dev `digest(A) = keccak256(0x1901 ‖ DOMAIN_SEPARATOR ‖ hashStruct(A))`; the ten static
    /// words of `A` are copied verbatim, so a non-canonical word can never match a signature.
    function _attestationDigest(AttestationV1 calldata attestation) private view returns (bytes32 digest) {
        bytes32 typehash = ATTESTATION_TYPEHASH;
        bytes32 separator = DOMAIN_SEPARATOR;
        assembly ("memory-safe") {
            let p := mload(0x40)
            mstore(p, typehash)
            calldatacopy(add(p, 0x20), attestation, 0x140)
            let structHash := keccak256(p, 0x160)
            mstore(p, shl(240, 0x1901))
            mstore(add(p, 2), separator)
            mstore(add(p, 34), structHash)
            digest := keccak256(p, 66)
        }
    }

    /// @dev §3.7 digest of a calldata roster with the `n`, `t`, generation and ordering checks.
    function _rosterDigest(RosterV1 calldata roster) private view returns (bytes32 digest, uint256 n) {
        bytes calldata members = roster.members;
        uint256 length = members.length;
        n = length / 20;
        if (length != n * 20 || n < MIN_ROSTER_MEMBERS || n > MAX_ROSTER_MEMBERS) revert BadRoster();
        uint256 threshold = roster.threshold;
        if (threshold != (2 * n) / 3 + 1) revert BadRoster();
        uint256 generation = roster.generation;
        if (generation == 0) revert BadRoster();
        uint256 validFromMs = roster.validFromMs;
        uint256 validUntilMs = roster.validUntilMs;
        bytes32 networkId = TAIRA_NETWORK_ID;
        bool ordered = true;
        assembly ("memory-safe") {
            let previous := 0
            for { let i := 0 } lt(i, n) { i := add(i, 1) } {
                let member := shr(96, calldataload(add(members.offset, mul(i, 20))))
                switch iszero(member)
                case 1 { if previous { ordered := 0 } }
                default {
                    if iszero(gt(member, previous)) { ordered := 0 }
                    previous := member
                }
            }
            let p := mload(0x40)
            mstore(p, "SCCP/ROSTER/V1")
            mstore(add(p, 14), networkId)
            mstore(add(p, 46), shl(192, generation))
            mstore(add(p, 54), shl(192, validFromMs))
            mstore(add(p, 62), shl(192, validUntilMs))
            mstore8(add(p, 70), n)
            mstore8(add(p, 71), threshold)
            calldatacopy(add(p, 72), members.offset, length)
            digest := keccak256(p, add(72, length))
        }
        if (!ordered) revert BadRoster();
    }

    /// @dev §3.8: positional signers, low-S, `v ∈ {27, 28}`, recovered address masked to 160 bits.
    function _verifySignatures(bytes32 digest, RosterV1 calldata roster, uint256 n, SignaturesV1 calldata signatures)
        private
        view
    {
        uint256 bitmap = signatures.signerBitmap;
        if (bitmap >> n != 0) revert BadSignatures();
        uint256 count;
        for (uint256 bits = bitmap; bits != 0; bits &= bits - 1) {
            ++count;
        }
        if (count < roster.threshold) revert TooFewSignatures();
        bytes calldata packed = signatures.signatures;
        if (packed.length != count * 65) revert BadSignatures();
        bytes calldata members = roster.members;
        uint256 membersOffset;
        uint256 signaturesOffset;
        assembly ("memory-safe") {
            membersOffset := members.offset
            signaturesOffset := packed.offset
        }
        if (!_recoverSigners(digest, bitmap, membersOffset, signaturesOffset)) revert BadSignatures();
    }

    /// @dev Recovers one signer per set bit, in ascending bit order, and compares it with the
    /// member at that position; the member must be nonzero.
    function _recoverSigners(bytes32 digest, uint256 bitmap, uint256 membersOffset, uint256 signature)
        private
        view
        returns (bool valid)
    {
        valid = true;
        assembly ("memory-safe") {
            let p := mload(0x40)
            for { let member := membersOffset } bitmap { member := add(member, 20) } {
                if and(bitmap, 1) {
                    let r := calldataload(signature)
                    let s := calldataload(add(signature, 32))
                    let v := byte(0, calldataload(add(signature, 64)))
                    signature := add(signature, 65)
                    if or(
                        iszero(or(eq(v, 27), eq(v, 28))),
                        or(or(iszero(r), iszero(lt(r, SECP256K1_N))), or(iszero(s), gt(s, SECP256K1_HALF_N)))
                    ) {
                        valid := 0
                        break
                    }
                    mstore(p, digest)
                    mstore(add(p, 32), v)
                    mstore(add(p, 64), r)
                    mstore(add(p, 96), s)
                    // Yul evaluates arguments right to left, so the call result is bound first
                    // and `returndatasize()` is read only after the precompile returned.
                    let recovered := 0
                    let success := staticcall(gas(), 1, p, 128, p, 32)
                    if and(success, eq(returndatasize(), 32)) {
                        recovered := and(mload(p), ADDRESS_MASK)
                    }
                    let expected := shr(96, calldataload(member))
                    if or(iszero(expected), iszero(eq(recovered, expected))) {
                        valid := 0
                        break
                    }
                }
                bitmap := shr(1, bitmap)
            }
        }
    }

    // ---------------------------------------------------------------------
    // Rotation internals
    // ---------------------------------------------------------------------

    function _rotate(RotationV1 calldata rotation, uint256 nowMs) private {
        AttestationV1 calldata attestation = rotation.attestation;
        bytes32 current = rosterDigest;
        uint256 currentValidUntilMs = rosterValidUntilMs;
        if (attestation.rosterDigest != current || nowMs > currentValidUntilMs) revert RosterNotAccepted();
        _verifySigned(attestation, rotation.current, rotation.signatures, current);

        RosterV1 calldata next = rotation.next;
        bytes32 nextDigest = attestation.nextRosterDigest;
        (bytes32 computed,) = _rosterDigest(next);
        if (nextDigest == bytes32(0) || computed != nextDigest) revert BadRotation();
        uint64 generation = next.generation;
        if (generation != uint256(rosterGeneration) + 1 || next.validFromMs != attestation.timestampMs) {
            revert BadRotation();
        }
        uint64 validUntilMs = next.validUntilMs;
        _checkRosterValidity(next.validFromMs, validUntilMs, nowMs);

        uint256 graceUntilMs = nowMs + PREVIOUS_ROSTER_GRACE_MS;
        prevRosterDigest = current;
        prevRosterValidUntilMs =
            uint64(currentValidUntilMs < graceUntilMs ? currentValidUntilMs : graceUntilMs);
        rosterDigest = nextDigest;
        rosterGeneration = generation;
        rosterValidUntilMs = validUntilMs;
        emit SccpRosterRotated(generation, nextDigest, validUntilMs);
    }

    /// @dev §5.1.5 step 3 bounds, shared by the constructor and every rotation.
    function _checkRosterValidity(uint256 validFromMs, uint256 validUntilMs, uint256 nowMs) private pure {
        if (
            validUntilMs <= validFromMs || validFromMs > nowMs + MAX_CLOCK_SKEW_MS
                || validUntilMs - validFromMs > MAX_ROSTER_VALIDITY_MS || validUntilMs <= nowMs
                || validUntilMs > nowMs + MAX_ROSTER_VALIDITY_MS
        ) revert BadRosterValidity();
    }

    // ---------------------------------------------------------------------
    // Message, control and history verification (§3.2 - §3.5)
    // ---------------------------------------------------------------------

    /// @dev §5.1.3 steps 4 and 5 against the given block root; `mint` selects the deadline side.
    function _verifyMessage(MessageProofV1 calldata proof, bytes32 sccpRoot, uint256 messageCount, bool mint)
        private
        view
        returns (bytes32 messageId, uint64 nonce, address recipient, uint256 amount)
    {
        if (messageCount == 0) revert BadProof();
        uint256 deadlineMs;
        (nonce, deadlineMs, amount, recipient) = _parseOutboundPayload(proof.payload);
        _checkDeadline(deadlineMs, mint);
        messageId = _outboundMessageId(proof.payload);
        _checkTransferInclusion(messageId, proof, sccpRoot, messageCount);
    }

    /// @dev Mint while `now_ms ≤ deadline_ms`; void only after it (§3.2).
    function _checkDeadline(uint256 deadlineMs, bool mint) private view {
        uint256 nowMs = block.timestamp * 1000;
        if (mint) {
            if (nowMs > deadlineMs) revert DeadlinePassed();
        } else if (nowMs <= deadlineMs) {
            revert DeadlineNotReached();
        }
    }

    function _checkTransferInclusion(
        bytes32 messageId,
        MessageProofV1 calldata proof,
        bytes32 sccpRoot,
        uint256 messageCount
    ) private view {
        if (_merkleRoot(_transferLeaf(messageId), proof.leafIndex, messageCount, proof.path) != sccpRoot) {
            revert BadProof();
        }
    }

    function _verifyControlLeaf(ControlProofV1 calldata control, bytes32 sccpRoot, uint256 messageCount)
        private
        view
    {
        if (messageCount == 0) revert BadProof();
        bytes32 leaf = _controlLeaf(control.controlNonce, control.paused);
        if (_merkleRoot(leaf, control.leafIndex, messageCount, control.path) != sccpRoot) revert BadProof();
    }

    /// @dev Proves the historical block `{height, sccpRoot, messageCount}` against `A.historyRoot`.
    function _verifyHistory(AttestationV1 calldata attestation, HistoryProofV1 calldata history) private pure {
        uint256 messageCount = history.messageCount;
        bytes32 sccpRoot = history.sccpRoot;
        if (messageCount == 0 || messageCount > MAX_BLOCK_LEAVES || sccpRoot == bytes32(0)) revert BadProof();
        if (history.path.length > MAX_HISTORY_PATH) revert BadProof();
        uint256 height = history.height;
        bytes32 leaf;
        assembly ("memory-safe") {
            let p := mload(0x40)
            mstore(p, "SCCP/HISTORY/V1")
            mstore(add(p, 15), shl(192, height))
            mstore(add(p, 23), sccpRoot)
            mstore(add(p, 55), shl(224, messageCount))
            leaf := keccak256(p, 59)
        }
        bytes32 root = _merkleRoot(leaf, history.leafIndex, attestation.historySize, history.path);
        if (root != attestation.historyRoot) revert BadProof();
    }

    /// @dev Positional promote-odd Merkle root (§3.4) binding the leaf index and count.
    function _merkleRoot(bytes32 leaf, uint256 index, uint256 count, bytes32[] calldata path)
        private
        pure
        returns (bytes32 node)
    {
        if (count == 0 || index >= count) revert BadProof();
        uint256 siblings = path.length;
        uint256 k;
        node = leaf;
        while (count > 1) {
            if (index & 1 == 1) {
                if (k >= siblings) revert BadProof();
                node = _node(path[k], node);
                ++k;
            } else if (index + 1 < count) {
                if (k >= siblings) revert BadProof();
                node = _node(node, path[k]);
                ++k;
            }
            index >>= 1;
            count = (count + 1) >> 1;
        }
        if (k != siblings) revert BadProof();
    }

    function _node(bytes32 left, bytes32 right) private pure returns (bytes32 node) {
        assembly ("memory-safe") {
            let p := mload(0x40)
            mstore(p, "SCCP/NODE/V1")
            mstore(add(p, 12), left)
            mstore(add(p, 44), right)
            node := keccak256(p, 76)
        }
    }

    function _transferLeaf(bytes32 messageId) private view returns (bytes32 leaf) {
        assembly ("memory-safe") {
            let p := mload(0x40)
            mstore(p, "SCCP/LEAF/V1")
            mstore(add(p, 12), messageId)
            mstore(add(p, 44), and(address(), ADDRESS_MASK))
            leaf := keccak256(p, 76)
        }
    }

    /// @dev §3.4 control leaf from the own immutables and the supplied nonce and pause value.
    function _controlLeaf(uint64 nonce, bool paused) private view returns (bytes32 leaf) {
        bytes32 networkId = TAIRA_NETWORK_ID;
        uint256 tag = NETWORK_TAG;
        uint256 identity = _identityWordOf(NETWORK_TAG);
        uint256 revision = ROUTE_REVISION;
        assembly ("memory-safe") {
            let p := mload(0x40)
            mstore(p, "SCCP/CONTROL/V1")
            mstore8(add(p, 15), TAG_TAIRA)
            mstore(add(p, 16), networkId)
            mstore8(add(p, 48), tag)
            mstore(add(p, 49), identity)
            mstore(add(p, 81), and(address(), ADDRESS_MASK))
            mstore(add(p, 113), shl(224, revision))
            mstore(add(p, 117), shl(192, nonce))
            mstore8(add(p, 125), paused)
            leaf := keccak256(p, 126)
        }
    }

    /// @dev `message_id` over lane `(sora-taira, own)` for an outbound calldata payload (§3.3).
    function _outboundMessageId(bytes calldata payload) private view returns (bytes32 messageId) {
        bytes32 networkId = TAIRA_NETWORK_ID;
        uint256 tag = NETWORK_TAG;
        uint256 identity = _identityWordOf(NETWORK_TAG);
        assembly ("memory-safe") {
            let p := mload(0x40)
            mstore(p, "SCCP/PAYLOAD/V1")
            calldatacopy(add(p, 15), payload.offset, payload.length)
            let payloadHash := keccak256(p, add(15, payload.length))
            mstore(p, "SCCP/MESSAGE/V1")
            mstore8(add(p, 15), TAG_TAIRA)
            mstore(add(p, 16), networkId)
            mstore8(add(p, 48), tag)
            mstore(add(p, 49), identity)
            mstore(add(p, 81), payloadHash)
            messageId := keccak256(p, 113)
        }
    }

    /// @dev `message_id` over lane `(own, sora-taira)` for an inbound memory payload (§3.3).
    function _inboundMessageId(bytes memory payload) private view returns (bytes32 messageId) {
        bytes32 networkId = TAIRA_NETWORK_ID;
        uint256 tag = NETWORK_TAG;
        uint256 identity = _identityWordOf(NETWORK_TAG);
        assembly ("memory-safe") {
            // The 15-byte domain tag temporarily occupies the tail of the length word.
            let length := mload(payload)
            mstore(payload, shr(136, "SCCP/PAYLOAD/V1"))
            let payloadHash := keccak256(add(payload, 17), add(15, length))
            mstore(payload, length)
            let p := mload(0x40)
            mstore(p, "SCCP/MESSAGE/V1")
            mstore8(add(p, 15), tag)
            mstore(add(p, 16), identity)
            mstore8(add(p, 48), TAG_TAIRA)
            mstore(add(p, 49), networkId)
            mstore(add(p, 81), payloadHash)
            messageId := keccak256(p, 113)
        }
    }

    /// @dev Strict §3.2 decoding of a Taira -> own payload; every field is checked and no
    /// trailing byte is admitted. Reads calldata slices only.
    function _parseOutboundPayload(bytes calldata payload)
        private
        view
        returns (uint64 nonce, uint256 deadlineMs, uint256 amount, address recipient)
    {
        uint256 length = payload.length;
        if (length < MIN_PAYLOAD_BYTES || length > MAX_PAYLOAD_BYTES) revert BadPayload();
        uint256 o;
        assembly ("memory-safe") {
            o := payload.offset
        }
        uint256 senderLength;
        (nonce, deadlineMs, amount, senderLength) = _parsePayloadHead(o);
        recipient = _parsePayloadTail(o + MIN_PAYLOAD_BYTES + senderLength, o + length);
    }

    /// @dev Fixed-layout head up to the sender length: kind, version, domains, nonce, revision,
    /// deadline, asset and amount.
    function _parsePayloadHead(uint256 o)
        private
        view
        returns (uint64 nonce, uint256 deadlineMs, uint256 amount, uint256 senderLength)
    {
        if (
            _be(o, 1) != PAYLOAD_KIND_TRANSFER || _be(o + 1, 1) != PAYLOAD_VERSION || _be(o + 2, 4) != 0
                || _be(o + 6, 4) != _domainOf(NETWORK_TAG) || _be(o + 18, 4) != ROUTE_REVISION
        ) revert BadPayload();
        nonce = uint64(_be(o + 10, 8));
        deadlineMs = _be(o + 22, 8);
        amount = _be(o + 40, 16);
        if (
            deadlineMs == 0 || _be(o + 30, 4) != 0 || _be(o + 34, 1) != CODEC_CANONICAL_TEXT || _be(o + 35, 2) != 3
                || _be(o + 37, 3) != ASSET_ID_XOR || amount == 0 || _be(o + 56, 1) != CODEC_TAIRA_ACCOUNT
        ) revert BadPayload();
        senderLength = _be(o + 57, 2);
        if (senderLength == 0 || senderLength > MAX_TAIRA_ACCOUNT_BYTES) revert BadPayload();
    }

    /// @dev Recipient and route id from calldata offset `r` to the exact payload end `end`.
    function _parsePayloadTail(uint256 r, uint256 end) private view returns (address recipient) {
        (uint256 codec, uint256 accountLength) = _accountCodecOf(NETWORK_TAG);
        (bytes32 routeId, uint256 routeIdLength) = _routeIdOf(NETWORK_TAG);
        if (end != r + 3 + accountLength + 3 + routeIdLength) revert BadPayload();
        if (_be(r, 1) != codec || _be(r + 1, 2) != accountLength) revert BadPayload();
        uint256 q = r + 3 + accountLength;
        if (
            _be(q, 1) != CODEC_CANONICAL_TEXT || _be(q + 1, 2) != routeIdLength
                || _be(q + 3, routeIdLength) != uint256(routeId) >> (256 - 8 * routeIdLength)
        ) revert BadPayload();
        r += 3;
        if (codec == CODEC_TRON_ADDRESS21) {
            if (_be(r, 1) != TRON_ADDRESS_PREFIX) revert BadRecipient();
            r += 1;
        }
        recipient = address(uint160(_be(r, 20)));
        if (recipient == address(0) || recipient == _self()) revert BadRecipient();
    }

    /// @dev Builds the §3.2 own -> Taira payload for `transferToTaira` in memory.
    function _inboundPayload(uint64 nonce, uint256 amount, bytes calldata tairaRecipient)
        private
        view
        returns (bytes memory payload)
    {
        (uint256 codec, uint256 accountLength) = _accountCodecOf(NETWORK_TAG);
        (bytes32 routeId, uint256 routeIdLength) = _routeIdOf(NETWORK_TAG);
        payload = new bytes(MIN_PAYLOAD_BYTES + accountLength + 3 + tairaRecipient.length + 3 + routeIdLength);
        uint256 head = (0x0201 << 32 | _domainOf(NETWORK_TAG)) << 208;
        uint256 revision = ROUTE_REVISION;
        assembly ("memory-safe") {
            let c := add(payload, 32)
            // kind, version, source domain (own), destination domain 0
            mstore(c, head)
            mstore(add(c, 10), shl(192, nonce))
            mstore(add(c, 18), shl(224, revision))
            // deadline 0, asset home domain 0, asset codec 1, asset id "xor"
            mstore(add(c, 22), 0)
            mstore(add(c, 34), or(shl(248, CODEC_CANONICAL_TEXT), or(shl(232, 3), shl(208, ASSET_ID_XOR))))
            mstore(add(c, 40), shl(128, amount))
            mstore(add(c, 56), or(shl(248, codec), shl(232, accountLength)))
            c := add(c, 59)
            if eq(codec, CODEC_TRON_ADDRESS21) {
                mstore8(c, TRON_ADDRESS_PREFIX)
                c := add(c, 1)
            }
            mstore(c, shl(96, and(caller(), ADDRESS_MASK)))
            c := add(c, 20)
            let recipientLength := tairaRecipient.length
            mstore(c, or(shl(248, CODEC_TAIRA_ACCOUNT), shl(232, recipientLength)))
            calldatacopy(add(c, 3), tairaRecipient.offset, recipientLength)
            c := add(c, add(3, recipientLength))
            mstore(c, or(shl(248, CODEC_CANONICAL_TEXT), shl(232, routeIdLength)))
            // The route id tail spills only zero bytes into unallocated memory.
            mstore(add(c, 3), routeId)
        }
    }

    /// @dev `NonCanonicalCalldata()` unless the §5.1.7 canonical encoding holds exactly.
    function _requireCanonicalTransferCalldata() private pure {
        bool canonical;
        assembly ("memory-safe") {
            let size := calldatasize()
            let length := calldataload(0x64)
            let padded := and(add(length, 31), not(31))
            canonical := and(
                and(eq(calldataload(4), 0x60), and(gt(length, 0), iszero(gt(length, MAX_TAIRA_ACCOUNT_BYTES)))),
                eq(size, add(CANONICAL_TRANSFER_HEAD, padded))
            )
            let tail := and(length, 31)
            if and(canonical, gt(tail, 0)) {
                let last := calldataload(add(CANONICAL_TRANSFER_HEAD, sub(padded, 32)))
                canonical := iszero(shl(mul(tail, 8), last))
            }
        }
        if (!canonical) revert NonCanonicalCalldata();
    }

    // ---------------------------------------------------------------------
    // State transitions
    // ---------------------------------------------------------------------

    function _applyControl(ControlProofV1 calldata control) private {
        uint64 nonce = control.controlNonce;
        if (nonce <= controlNonce) revert StaleControl();
        bool paused = control.paused;
        controlNonce = nonce;
        mintingPaused = paused;
        _bumpOpCount();
        emit SccpControlApplied(nonce, paused);
    }

    function _void(bytes32 messageId, uint64 nonce, uint64 payloadNonce) private {
        if (nonce != payloadNonce) revert BadNonce(payloadNonce);
        _consume(nonce);
        _bumpOpCount();
        emit SccpVoided(messageId, nonce);
    }

    function _consume(uint64 nonce) private {
        uint256 word = uint256(nonce) >> 8;
        uint256 bit = uint256(1) << (uint256(nonce) & 0xff);
        uint256 bits = consumedBitmap[word];
        if (bits & bit != 0) revert AlreadyConsumed(nonce);
        consumedBitmap[word] = bits | bit;
    }

    /// @dev Consumes `[first, last]` (at most 256 nonces, so at most two bitmap words).
    function _consumeRange(uint256 first, uint256 last) private {
        for (uint256 word = first >> 8; word <= last >> 8; ++word) {
            uint256 low = word == first >> 8 ? first & 0xff : 0;
            uint256 high = word == last >> 8 ? last & 0xff : 0xff;
            uint256 mask = (type(uint256).max >> (255 - high)) & (type(uint256).max << low);
            uint256 bits = consumedBitmap[word];
            uint256 taken = bits & mask;
            if (taken != 0) {
                uint256 lowest;
                while (taken & (uint256(1) << lowest) == 0) {
                    ++lowest;
                }
                revert AlreadyConsumed(uint64((word << 8) | lowest));
            }
            consumedBitmap[word] = bits | mask;
        }
    }

    function _mint(address recipient, uint256 amount) private {
        uint256 supply = totalSupply + amount;
        if (supply > MAX_WRAPPED_SUPPLY) revert SupplyCapExceeded();
        totalSupply = supply;
        unchecked {
            balanceOf[recipient] += amount;
        }
        emit Transfer(address(0), recipient, amount);
    }

    function _burn(address sender, uint256 amount) private {
        uint256 balance = balanceOf[sender];
        if (balance < amount) revert ERC20InsufficientBalance(sender, balance, amount);
        unchecked {
            balanceOf[sender] = balance - amount;
            totalSupply -= amount;
        }
        emit Transfer(sender, address(0), amount);
    }

    function _transfer(address from, address to, uint256 value) private {
        if (from == address(0)) revert ERC20InvalidSender(address(0));
        if (to == address(0) || to == _self()) revert ERC20InvalidReceiver(to);
        uint256 balance = balanceOf[from];
        if (balance < value) revert ERC20InsufficientBalance(from, balance, value);
        unchecked {
            balanceOf[from] = balance - value;
            balanceOf[to] += value;
        }
        emit Transfer(from, to, value);
    }

    function _bumpOpCount() private {
        unchecked {
            opCount += 1;
        }
    }

    // ---------------------------------------------------------------------
    // Profile helpers
    // ---------------------------------------------------------------------

    function _requireChain() private view {
        if (block.chainid != _identityWordOf(NETWORK_TAG)) revert WrongChain();
    }

    function _requireDirectCaller() private view {
        bool direct;
        assembly ("memory-safe") {
            direct := eq(and(caller(), ADDRESS_MASK), and(origin(), ADDRESS_MASK))
        }
        if (!direct) revert DirectCallerRequired();
    }

    function _self() private view returns (address self) {
        assembly ("memory-safe") {
            self := and(address(), ADDRESS_MASK)
        }
    }

    /// @dev The §2.1 identity word (EVM/TVM chain id) of an admitted network tag.
    function _identityWordOf(uint8 tag) private pure returns (uint256) {
        if (tag == TAG_ETHEREUM) return 1;
        if (tag == TAG_BSC) return 56;
        return TRON_CHAIN_ID;
    }

    /// @dev SCCP domain of an admitted network tag (§2).
    function _domainOf(uint8 tag) private pure returns (uint256) {
        if (tag == TAG_ETHEREUM) return 1;
        if (tag == TAG_BSC) return 2;
        return 5;
    }

    /// @dev Left-aligned `route_id` text and its length (§2.3).
    function _routeIdOf(uint8 tag) private pure returns (bytes32 routeId, uint256 length) {
        if (tag == TAG_ETHEREUM) return ("taira_eth_xor", 13);
        if (tag == TAG_BSC) return ("taira_bsc_xor", 13);
        return ("taira_tron_xor", 14);
    }

    /// @dev Own account codec and its byte length (§3.1).
    function _accountCodecOf(uint8 tag) private pure returns (uint256 codec, uint256 length) {
        if (tag == TAG_TRON) return (CODEC_TRON_ADDRESS21, 21);
        return (CODEC_EVM_ADDRESS20, 20);
    }

    /// @dev Big-endian unsigned integer of `width` (1..=32) bytes at calldata offset `offset`.
    function _be(uint256 offset, uint256 width) private pure returns (uint256 value) {
        assembly ("memory-safe") {
            value := shr(sub(256, mul(width, 8)), calldataload(offset))
        }
    }

    /// @dev §3.7 digest of the constructor's memory roster with the same checks as `_rosterDigest`.
    function _initialRosterDigest(bytes32 networkId, RosterV1 memory roster) private pure returns (bytes32) {
        bytes memory members = roster.members;
        uint256 length = members.length;
        uint256 n = length / 20;
        if (length != n * 20 || n < MIN_ROSTER_MEMBERS || n > MAX_ROSTER_MEMBERS) revert BadRoster();
        if (roster.threshold != (2 * n) / 3 + 1 || roster.generation == 0) revert BadRoster();
        uint256 previous;
        for (uint256 i = 0; i < n; ++i) {
            uint256 member;
            assembly ("memory-safe") {
                member := shr(96, mload(add(add(members, 32), mul(i, 20))))
            }
            if (member == 0) {
                if (previous != 0) revert BadRoster();
            } else {
                if (member <= previous) revert BadRoster();
                previous = member;
            }
        }
        return keccak256(
            abi.encodePacked(
                bytes14("SCCP/ROSTER/V1"),
                networkId,
                roster.generation,
                roster.validFromMs,
                roster.validUntilMs,
                uint8(n),
                roster.threshold,
                members
            )
        );
    }
}
