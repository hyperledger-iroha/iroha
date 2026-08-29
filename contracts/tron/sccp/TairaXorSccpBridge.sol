// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.7.4;
pragma experimental ABIEncoderV2;

import "../../evm/sccp/SccpExactTransferCodec.sol";
import "../../evm/sccp/SccpSha256ReplayForest.sol";

interface ITairaXorTronToken {
    function bridge() external view returns (address);
    function decimals() external view returns (uint8);
    function totalSupply() external view returns (uint256);
    function balanceOf(address account) external view returns (uint256);
    function mint(address to, uint256 value) external returns (bool);
    function burnFrom(address from, uint256 value) external returns (bool);
}

interface ISccpTronExactVerifier {
    function networkId() external view returns (bytes32);
    function expectedSourceDomain() external view returns (uint32);
    function expectedTargetDomain() external view returns (uint32);
    function verifyingKeyHash() external view returns (bytes32);
    function semanticProofProfileHash() external view returns (bytes32);
    function soraFinalityAnchorHash() external view returns (bytes32);
    function verifySccpMessageProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 submittedDestinationBindingHash,
        bytes32 submittedRouteConfigurationHash
    ) external view returns (bytes32 messageId, uint32 sourceDomain, bytes32 commitmentRoot);
}

/** Route-bound, one-way 3-of-5 controller for stopping new wrapped mints. */
contract SccpTronMintBreaker {
    address public immutable route;
    address public immutable guardian0;
    address public immutable guardian1;
    address public immutable guardian2;
    address public immutable guardian3;
    address public immutable guardian4;
    mapping(address => bool) public hasVoted;
    uint8 public voteCount;
    bool public mintingDisabled;

    event MintDisableVote(address indexed guardian, uint8 voteCount);
    event MintingDisabled(address indexed route);

    constructor(address[5] memory guardians) {
        for (uint256 i = 0; i < guardians.length; i++) {
            require(guardians[i] != address(0), "SC_BREAKER");
            for (uint256 j = 0; j < i; j++) {
                require(guardians[i] != guardians[j], "SC_BREAKER");
            }
        }
        route = msg.sender;
        guardian0 = guardians[0];
        guardian1 = guardians[1];
        guardian2 = guardians[2];
        guardian3 = guardians[3];
        guardian4 = guardians[4];
    }

    function disableMinting() external {
        require(!mintingDisabled, "SC_BREAKER");
        require(
            msg.sender == guardian0 || msg.sender == guardian1 || msg.sender == guardian2
                || msg.sender == guardian3 || msg.sender == guardian4,
            "SC_BREAKER"
        );
        require(!hasVoted[msg.sender], "SC_BREAKER");
        hasVoted[msg.sender] = true;
        uint8 votes = voteCount + 1;
        voteCount = votes;
        emit MintDisableVote(msg.sender, votes);
        if (votes == 3) {
            mintingDisabled = true;
            emit MintingDisabled(route);
        }
    }
}

/** Exact TRON/Taira route bound to one predeployed immutable token. */
contract TairaXorSccpBridge {
    struct VerifierPolicyV1 {
        address verifierAddress;
        bytes32 verifierCodeHash;
        bytes32 verifierKeyHash;
        bytes32 semanticProofProfileHash;
        bytes32 soraFinalityAnchorHash;
    }

    struct RouteDeploymentV1 {
        address tokenAddress;
        bytes32 tokenCodeHash;
        VerifierPolicyV1 verifierPolicy;
        uint8 tronProfile;
        uint32 routeRevision;
        bytes32 networkId;
        bytes32 sourceLaneHash;
        bytes32 destinationLaneHash;
        bytes32 destinationBindingHash;
        address replayVerifierAddress;
        bytes32 replayVerifierCodeHash;
        address mintBreakerAddress;
        bytes32 mintBreakerCodeHash;
        uint256 maxWrappedSupply;
    }

    uint32 private constant DOMAIN_SORA = 0;
    uint32 private constant DOMAIN_TRON = 5;
    uint8 private constant CODEC_TEXT = 1;
    uint8 private constant CODEC_TRON21 = 5;
    uint32 private constant REPLAY_NETWORK_SORA = 0x40;
    uint8 private constant REPLAY_NETWORK_TRON = 0x43;
    uint8 private constant REPLAY_ACTOR_TRON = 2;
    uint8 private constant REPLAY_PRINCIPAL_TRON = 3;
    uint8 private constant REPLAY_TRON_SOURCE_BURN = 0x20;
    uint8 private constant REPLAY_TRON_DESTINATION_MINT = 0x21;
    uint256 private constant MAX_U128 = (uint256(1) << 128) - 1;
    uint256 private constant TAIRA_TO_TOKEN_SCALE = 1000000000;
    bytes private constant ASSET_ID = "xor";
    bytes private constant ROUTE_ID = "taira_tron_xor";
    bytes32 private constant DESTINATION_BINDING_SEPARATOR =
        keccak256("iroha:sccp:tron-destination-binding:v1");
    bytes32 private constant VERIFIER_BACKEND_HASH =
        keccak256("tron-groth16-bn254-v1");
    bytes32 private constant ROUTE_CONFIG_SEPARATOR =
        keccak256("sccp:concrete-route-config:v1");
    bytes32 private constant EMPTY_CODE_HASH = keccak256("");

    ITairaXorTronToken public immutable token;
    ISccpTronExactVerifier public immutable verifier;
    bytes32 public immutable networkId;
    uint32 public immutable routeRevision;
    bytes32 public immutable tokenCodeHash;
    bytes32 public immutable verifierCodeHash;
    bytes32 public immutable verifierKeyHash;
    bytes32 public immutable semanticProofProfileHash;
    bytes32 public immutable soraFinalityAnchorHash;
    bytes32 public immutable sourceLaneHash;
    bytes32 public immutable destinationLaneHash;
    bytes32 public immutable routeConfigHash;
    bytes32 public immutable destinationBindingHash;
    SccpSha256ReplayForest public immutable replayVerifier;
    bytes32 public immutable replayVerifierCodeHash;
    bytes32 private immutable emptyReplayShardRoot;
    bytes32 private immutable sourceReplayDomainHash;
    bytes32 private immutable destinationReplayDomainHash;
    SccpTronMintBreaker public immutable mintBreaker;
    bytes32 public immutable mintBreakerCodeHash;
    uint256 public immutable maxWrappedSupply;

    mapping(address => uint64) public transferNonces;
    bytes32[256] private sourceReplayRoots;
    bytes32[256] private destinationReplayRoots;
    uint64 private sourceReplayCount;
    uint64 private destinationReplayCount;
    uint256 private reentrancyState = 1;

    event SccpTransfer(
        bytes32 indexed laneHash,
        bytes32 indexed messageId,
        bytes32 indexed sourceEventDigest,
        bytes32 payloadHash,
        bytes32 routeConfigHash,
        bytes canonicalPayload
    );
    event TairaXorMintFinalized(
        bytes32 indexed messageId,
        address indexed recipient,
        uint256 amount,
        bytes32 payloadHash
    );
    event SccpReplayDeltaV1(
        bytes32 indexed domainHash,
        uint8 indexed shard,
        bytes32 indexed key,
        bytes32 recordDigest,
        bytes32 oldRoot,
        bytes32 newRoot,
        uint64 leafCount,
        uint64 updateSequence
    );

    modifier nonReentrant() {
        require(reentrancyState == 1, "SC_REENTRY");
        reentrancyState = 2;
        _;
        reentrancyState = 1;
    }

    modifier onExpectedChain() {
        require(bytes32(_chainId()) == networkId, "SC_TRANSFER");
        _;
    }

    constructor(
        address tokenAddress,
        VerifierPolicyV1 memory configuredVerifierPolicy,
        uint8 configuredTronProfile,
        uint32 configuredRouteRevision,
        address[5] memory configuredMintGuardians,
        uint256 configuredMaxWrappedSupply
    ) {
        require(
            tokenAddress != address(0) && configuredVerifierPolicy.verifierAddress != address(0),
            "SC_DEPLOY"
        );
        require(tokenAddress != configuredVerifierPolicy.verifierAddress,
            "SC_DEPLOY");
        require(configuredTronProfile == REPLAY_NETWORK_TRON, "SC_DEPLOY");
        RouteDeploymentV1 memory deployment;
        deployment.tokenAddress = tokenAddress;
        deployment.verifierPolicy = configuredVerifierPolicy;
        deployment.tronProfile = configuredTronProfile;
        deployment.routeRevision = configuredRouteRevision;
        deployment.networkId = _networkIdWord(configuredTronProfile);
        deployment.maxWrappedSupply = configuredMaxWrappedSupply;
        require(bytes32(_chainId()) == deployment.networkId, "SC_DEPLOY");
        require(configuredRouteRevision != 0, "SC_DEPLOY");
        require(
            configuredMaxWrappedSupply != 0 && configuredMaxWrappedSupply <= MAX_U128,
            "SC_DEPLOY"
        );
        require(
            configuredVerifierPolicy.semanticProofProfileHash != bytes32(0),
            "SC_DEPLOY"
        );
        require(
            configuredVerifierPolicy.soraFinalityAnchorHash != bytes32(0),
            "SC_DEPLOY"
        );
        require(
            configuredVerifierPolicy.semanticProofProfileHash !=
                configuredVerifierPolicy.soraFinalityAnchorHash,
            "SC_DEPLOY"
        );
        ISccpTronExactVerifier configuredVerifier = ISccpTronExactVerifier(
            configuredVerifierPolicy.verifierAddress
        );
        require(configuredVerifier.expectedSourceDomain() == DOMAIN_SORA
            && configuredVerifier.expectedTargetDomain() == DOMAIN_TRON,
            "SC_DEPLOY");
        require(configuredVerifier.networkId() == deployment.networkId,
            "SC_DEPLOY");
        require(configuredVerifierPolicy.verifierCodeHash != bytes32(0)
            && _codeHash(configuredVerifierPolicy.verifierAddress) ==
                configuredVerifierPolicy.verifierCodeHash
            && configuredVerifierPolicy.verifierCodeHash != EMPTY_CODE_HASH,
            "SC_DEPLOY");
        require(configuredVerifierPolicy.verifierKeyHash != bytes32(0)
            && configuredVerifier.verifyingKeyHash() == configuredVerifierPolicy.verifierKeyHash,
            "SC_DEPLOY");
        require(
            configuredVerifier.semanticProofProfileHash() ==
                configuredVerifierPolicy.semanticProofProfileHash,
            "SC_DEPLOY"
        );
        require(
            configuredVerifier.soraFinalityAnchorHash() ==
                configuredVerifierPolicy.soraFinalityAnchorHash,
            "SC_DEPLOY"
        );

        require(ITairaXorTronToken(tokenAddress).bridge() == address(this),
            "SC_DEPLOY");
        require(ITairaXorTronToken(tokenAddress).decimals() == 18,
            "SC_DEPLOY");
        require(ITairaXorTronToken(tokenAddress).totalSupply() == 0,
            "SC_DEPLOY");
        deployment.tokenCodeHash = _codeHash(tokenAddress);
        require(
            deployment.tokenCodeHash != bytes32(0)
                && deployment.tokenCodeHash != EMPTY_CODE_HASH,
            "SC_DEPLOY");
        deployment.replayVerifierAddress = address(new SccpSha256ReplayForest());
        deployment.replayVerifierCodeHash = _codeHash(deployment.replayVerifierAddress);
        require(
            deployment.replayVerifierCodeHash != bytes32(0)
                && deployment.replayVerifierCodeHash != EMPTY_CODE_HASH,
            "SC_DEPLOY"
        );
        deployment.mintBreakerAddress = address(new SccpTronMintBreaker(
            configuredMintGuardians
        ));
        deployment.mintBreakerCodeHash = _codeHash(deployment.mintBreakerAddress);
        _requireDistinctDeploymentHashRoles(
            deployment.tokenCodeHash,
            deployment.replayVerifierCodeHash,
            deployment.mintBreakerCodeHash,
            configuredVerifierPolicy
        );

        (deployment.sourceLaneHash, deployment.destinationLaneHash) =
            _laneHashes(configuredTronProfile);
        bytes32 binding = _routeDestinationBinding(deployment);
        deployment.destinationBindingHash = binding;
        bytes32 exactRouteConfigHash = _routeConfigurationHash(deployment);
        (bytes32 exactSourceReplayDomainHash, bytes32 exactDestinationReplayDomainHash) =
            _replayDomainHashes(
                deployment.replayVerifierAddress,
                configuredRouteRevision,
                exactRouteConfigHash
            );

        token = ITairaXorTronToken(tokenAddress);
        verifier = configuredVerifier;
        networkId = deployment.networkId;
        routeRevision = configuredRouteRevision;
        tokenCodeHash = deployment.tokenCodeHash;
        verifierCodeHash = configuredVerifierPolicy.verifierCodeHash;
        verifierKeyHash = configuredVerifierPolicy.verifierKeyHash;
        semanticProofProfileHash = configuredVerifierPolicy.semanticProofProfileHash;
        soraFinalityAnchorHash = configuredVerifierPolicy.soraFinalityAnchorHash;
        sourceLaneHash = deployment.sourceLaneHash;
        destinationLaneHash = deployment.destinationLaneHash;
        routeConfigHash = exactRouteConfigHash;
        destinationBindingHash = binding;
        replayVerifier = SccpSha256ReplayForest(deployment.replayVerifierAddress);
        replayVerifierCodeHash = deployment.replayVerifierCodeHash;
        emptyReplayShardRoot = SccpSha256ReplayForest(
            deployment.replayVerifierAddress
        ).emptyShardRoot();
        sourceReplayDomainHash = exactSourceReplayDomainHash;
        destinationReplayDomainHash = exactDestinationReplayDomainHash;
        mintBreaker = SccpTronMintBreaker(deployment.mintBreakerAddress);
        mintBreakerCodeHash = deployment.mintBreakerCodeHash;
        maxWrappedSupply = configuredMaxWrappedSupply;
    }

    function _routeConfigurationHash(RouteDeploymentV1 memory deployment)
        private
        pure
        returns (bytes32)
    {
        bytes32 deploymentConfigHash = keccak256(abi.encode(
            deployment.tokenAddress,
            deployment.tokenCodeHash,
            deployment.verifierPolicy.verifierAddress,
            deployment.verifierPolicy.verifierCodeHash,
            deployment.verifierPolicy.verifierKeyHash,
            deployment.verifierPolicy.semanticProofProfileHash,
            deployment.verifierPolicy.soraFinalityAnchorHash,
            deployment.destinationBindingHash,
            deployment.replayVerifierAddress,
            deployment.replayVerifierCodeHash,
            deployment.mintBreakerAddress,
            deployment.mintBreakerCodeHash
        ));
        bytes32 assetRouteConfigHash = keccak256(abi.encode(
            keccak256(ASSET_ID),
            keccak256(ROUTE_ID),
            deployment.routeRevision,
            TAIRA_TO_TOKEN_SCALE,
            deployment.maxWrappedSupply
        ));
        return keccak256(abi.encode(
            ROUTE_CONFIG_SEPARATOR,
            DOMAIN_TRON,
            deployment.tronProfile,
            deployment.networkId,
            deployment.sourceLaneHash,
            deployment.destinationLaneHash,
            deploymentConfigHash,
            assetRouteConfigHash
        ));
    }

    function _replayDomainHashes(
        address configuredReplayVerifier,
        uint32 configuredRouteRevision,
        bytes32 configuredRouteHash
    ) private view returns (bytes32 source, bytes32 destination) {
        bytes memory actor = abi.encodePacked(address(this));
        source = SccpSha256ReplayForest(configuredReplayVerifier).domainHash(
            REPLAY_NETWORK_TRON,
            REPLAY_NETWORK_SORA,
            REPLAY_TRON_SOURCE_BURN,
            configuredRouteRevision,
            configuredRouteHash,
            REPLAY_ACTOR_TRON,
            actor
        );
        destination = SccpSha256ReplayForest(configuredReplayVerifier).domainHash(
            REPLAY_NETWORK_SORA,
            REPLAY_NETWORK_TRON,
            REPLAY_TRON_DESTINATION_MINT,
            configuredRouteRevision,
            configuredRouteHash,
            REPLAY_ACTOR_TRON,
            actor
        );
    }

    function _laneHashes(uint8 profile) private pure returns (bytes32 inbound, bytes32 outbound) {
        bytes memory tron = SccpExactTransferCodec.tronNetwork(profile);
        bytes memory taira = SccpExactTransferCodec.tairaNetwork();
        inbound = SccpExactTransferCodec.laneHash(SccpExactTransferCodec.lane(tron, taira));
        outbound = SccpExactTransferCodec.laneHash(SccpExactTransferCodec.lane(taira, tron));
    }

    function _routeDestinationBinding(RouteDeploymentV1 memory deployment)
        private
        view
        returns (bytes32)
    {
        // Every field is one static ABI word. Splitting the encoder input keeps
        // the exact preimage while avoiding Solidity 0.7.4's 16-slot codegen
        // limit for one large `abi.encode` expression.
        bytes memory prefix = abi.encode(
            DESTINATION_BINDING_SEPARATOR,
            VERIFIER_BACKEND_HASH,
            deployment.networkId,
            uint256(DOMAIN_SORA),
            uint256(DOMAIN_TRON),
            _tronAddressWord(deployment.verifierPolicy.verifierAddress),
            _tronAddressWord(address(this))
        );
        bytes memory suffix = abi.encode(
            deployment.verifierPolicy.verifierCodeHash,
            deployment.verifierPolicy.verifierKeyHash,
            deployment.verifierPolicy.semanticProofProfileHash,
            deployment.verifierPolicy.soraFinalityAnchorHash,
            _tronAddressWord(deployment.replayVerifierAddress),
            deployment.replayVerifierCodeHash,
            _tronAddressWord(deployment.mintBreakerAddress),
            deployment.mintBreakerCodeHash
        );
        return keccak256(abi.encodePacked(prefix, suffix));
    }

    /** Burn wrapped XOR and emit one exact TRON-to-Taira Transfer statement. */
    function transferToTaira(
        bytes calldata tairaRecipient,
        uint256 tokenAmount,
        uint64 expectedNonce,
        bytes calldata replayWitness
    )
        external onExpectedChain nonReentrant returns (bytes32 messageId)
    {
        bytes memory payload;
        require(tokenAmount != 0 && tokenAmount % TAIRA_TO_TOKEN_SCALE == 0,
            "SC_TRANSFER");
        uint256 tairaAmount = tokenAmount / TAIRA_TO_TOKEN_SCALE;
        require(tairaAmount != 0 && tairaAmount <= MAX_U128, "SC_TRANSFER");
        require(transferNonces[msg.sender] != type(uint64).max, "SC_TRANSFER");
        require(expectedNonce == transferNonces[msg.sender], "SC_TRANSFER");
        require(_codeHash(address(token)) == tokenCodeHash, "SC_TOKEN");
        {
            bytes memory recipient = tairaRecipient;
            require(
                SccpExactTransferCodec.isCanonicalTairaRecipient(recipient),
                "SC_TRANSFER"
            );
            SccpExactTransferCodec.TransferFields memory fields;
            fields.sourceDomain = DOMAIN_TRON;
            fields.destinationDomain = DOMAIN_SORA;
            fields.nonce = expectedNonce;
            fields.routeRevision = routeRevision;
            fields.assetHomeDomain = DOMAIN_SORA;
            fields.assetId = ASSET_ID;
            fields.amount = tairaAmount;
            fields.senderCodec = CODEC_TRON21;
            fields.sender = abi.encodePacked(bytes1(0x41), msg.sender);
            fields.recipientCodec = CODEC_TEXT;
            fields.recipient = recipient;
            fields.routeId = ROUTE_ID;
            payload = SccpExactTransferCodec.transferPayload(fields);
        }
        messageId = SccpExactTransferCodec.messageId(
            SccpExactTransferCodec.lane(
                SccpExactTransferCodec.tronNetwork(REPLAY_NETWORK_TRON),
                SccpExactTransferCodec.tairaNetwork()
            ),
            payload
        );
        bytes32 canonicalPayloadHash = SccpExactTransferCodec.payloadHash(payload);
        bytes32 eventDigest = SccpExactTransferCodec.sourceEventDigest(
            sourceLaneHash, messageId, canonicalPayloadHash
        );

        transferNonces[msg.sender] = expectedNonce + 1;
        _occupySourceReplay(
            messageId, payload, uint128(tairaAmount), eventDigest, replayWitness
        );
        _mutateTokenExact(msg.sender, tokenAmount, false);
        emit SccpTransfer(
            sourceLaneHash,
            messageId,
            eventDigest,
            canonicalPayloadHash,
            routeConfigHash,
            payload
        );
    }

    /** Verify one exact Taira-to-TRON transfer proof and mint wrapped XOR. */
    function finalizeFromTaira(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes calldata canonicalPayloadBytes,
        bytes calldata replayWitness
    ) external onExpectedChain nonReentrant returns (bytes32 messageId) {
        require(
            _codeHash(address(mintBreaker)) == mintBreakerCodeHash,
            "SC_BREAKER"
        );
        require(!mintBreaker.mintingDisabled(), "SC_BREAKER");
        require(statementHash != bytes32(0), "SC_PROOF");
        require(publicInputs[2] == bytes32(uint256(DOMAIN_TRON)), "SC_PROOF");
        require(_codeHash(address(token)) == tokenCodeHash, "SC_TOKEN");
        bytes memory payload = canonicalPayloadBytes;
        (address recipient, uint256 tairaAmount) = _parseTairaToTronTransfer(payload);
        bytes32 canonicalPayloadHash = SccpExactTransferCodec.payloadHash(payload);
        require(publicInputs[1] == canonicalPayloadHash, "SC_PROOF");
        bytes32 expectedMessageId = _destinationMessageId(payload);
        require(publicInputs[0] == expectedMessageId, "SC_PROOF");
        messageId = _verifyDestinationProof(
            proofBytes, publicInputs, statementHash, expectedMessageId
        );
        require(
            tairaAmount <= type(uint256).max / TAIRA_TO_TOKEN_SCALE,
            "SC_TRANSFER"
        );
        uint256 tokenAmount = tairaAmount * TAIRA_TO_TOKEN_SCALE;

        _occupyDestinationReplay(
            messageId, payload, uint128(tairaAmount), recipient, replayWitness
        );
        _mutateTokenExact(recipient, tokenAmount, true);
        emit TairaXorMintFinalized(messageId, recipient, tokenAmount, canonicalPayloadHash);
    }

    function _occupySourceReplay(
        bytes32 replayId,
        bytes memory payload,
        uint128 amount,
        bytes32 eventDigest,
        bytes calldata witness
    ) private {
        SccpSha256ReplayForest.SccpAddressReplayRecord memory record =
            SccpSha256ReplayForest.SccpAddressReplayRecord({
                operation: REPLAY_TRON_SOURCE_BURN,
                replayId: replayId,
                payloadSha256: sha256(payload),
                amountScale9: amount,
                principalKind: REPLAY_PRINCIPAL_TRON,
                principal: msg.sender,
                auxiliaryIdentitySha256: sha256(abi.encodePacked(eventDigest))
            });
        _occupyReplay(true, record, witness);
    }

    function _occupyDestinationReplay(
        bytes32 replayId,
        bytes memory payload,
        uint128 amount,
        address principal,
        bytes calldata witness
    ) private {
        SccpSha256ReplayForest.SccpAddressReplayRecord memory record =
            SccpSha256ReplayForest.SccpAddressReplayRecord({
                operation: REPLAY_TRON_DESTINATION_MINT,
                replayId: replayId,
                payloadSha256: sha256(payload),
                amountScale9: amount,
                principalKind: REPLAY_PRINCIPAL_TRON,
                principal: principal,
                auxiliaryIdentitySha256: sha256(abi.encodePacked(destinationBindingHash))
            });
        _occupyReplay(false, record, witness);
    }

    function _occupyReplay(
        bool source,
        SccpSha256ReplayForest.SccpAddressReplayRecord memory record,
        bytes calldata witness
    ) private {
        require(
            _codeHash(address(replayVerifier)) == replayVerifierCodeHash,
            "SC_REPLAY"
        );
        bytes32 domainHash = source ? sourceReplayDomainHash : destinationReplayDomainHash;
        uint8 shard;
        bytes32 key;
        bytes32 recordDigest;
        bytes32 oldRoot;
        bytes32 newRoot;
        (shard, key, recordDigest, oldRoot, newRoot) =
            replayVerifier.prepareAddressOccupation(
                domainHash, record, witness
        );
        require(oldRoot == _replayShardRoot(source, shard), "SC_REPLAY");

        uint64 currentCount = source ? sourceReplayCount : destinationReplayCount;
        require(currentCount != type(uint64).max, "SC_REPLAY");
        uint64 nextCount = currentCount + 1;
        if (source) {
            sourceReplayRoots[shard] = newRoot;
            sourceReplayCount = nextCount;
        } else {
            destinationReplayRoots[shard] = newRoot;
            destinationReplayCount = nextCount;
        }
        emit SccpReplayDeltaV1(
            domainHash,
            shard,
            key,
            recordDigest,
            oldRoot,
            newRoot,
            nextCount,
            nextCount
        );
    }

    function _mutateTokenExact(address account, uint256 amount, bool minting) private {
        uint256 expectedSupply = token.totalSupply();
        uint256 expectedBalance = token.balanceOf(account);
        if (minting) {
            require(
                amount <= maxWrappedSupply
                    && expectedSupply <= maxWrappedSupply - amount
            );
            require(expectedBalance <= type(uint256).max - amount);
            expectedSupply += amount;
            expectedBalance += amount;
            require(token.mint(account, amount));
        } else {
            require(expectedSupply >= amount && expectedBalance >= amount);
            expectedSupply -= amount;
            expectedBalance -= amount;
            require(token.burnFrom(account, amount));
        }
        // Solidity 0.7 has no custom errors; keep this hot adapter path terse
        // enough for the production TVM runtime-size corridor.
        require(token.totalSupply() == expectedSupply && token.balanceOf(account) == expectedBalance);
    }

    /** Return the complete constant-size state needed to build a strict witness. */
    function replayForestState(bool source, uint8 shard)
        external
        view
        returns (bytes32, bytes32, uint64, uint64)
    {
        uint64 count = source ? sourceReplayCount : destinationReplayCount;
        return (
            source ? sourceReplayDomainHash : destinationReplayDomainHash,
            _replayShardRoot(source, shard),
            count,
            count
        );
    }

    function _replayShardRoot(bool source, uint8 shard) private view returns (bytes32) {
        bytes32 root = source ? sourceReplayRoots[shard] : destinationReplayRoots[shard];
        return root == bytes32(0) ? emptyReplayShardRoot : root;
    }

    function _destinationMessageId(bytes memory payload) private pure returns (bytes32) {
        return SccpExactTransferCodec.messageId(
            SccpExactTransferCodec.lane(
                SccpExactTransferCodec.tairaNetwork(),
                SccpExactTransferCodec.tronNetwork(REPLAY_NETWORK_TRON)
            ),
            payload
        );
    }

    function _verifyDestinationProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 expectedMessageId
    ) private view returns (bytes32 messageId) {
        require(_codeHash(address(verifier)) == verifierCodeHash, "SC_PROOF");
        require(verifier.verifyingKeyHash() == verifierKeyHash, "SC_PROOF");
        require(
            verifier.semanticProofProfileHash() == semanticProofProfileHash,
            "SC_PROOF"
        );
        require(
            verifier.soraFinalityAnchorHash() == soraFinalityAnchorHash,
            "SC_PROOF"
        );
        uint32 sourceDomain;
        bytes32 commitmentRoot;
        (messageId, sourceDomain, commitmentRoot) = verifier.verifySccpMessageProof(
            proofBytes, publicInputs, statementHash, destinationBindingHash, routeConfigHash
        );
        require(messageId == expectedMessageId, "SC_PROOF");
        require(sourceDomain == DOMAIN_SORA, "SC_PROOF");
        require(commitmentRoot == publicInputs[3] && commitmentRoot != bytes32(0),
            "SC_PROOF");
    }

    function _parseTairaToTronTransfer(bytes memory payload)
        private view returns (address recipient, uint256 amount)
    {
        uint256 offset = 0;
        require(_readU8(payload, offset++) == 2, "SC_TRANSFER");
        require(_readU8(payload, offset++) == 1, "SC_TRANSFER");
        require(_readU32Le(payload, offset) == DOMAIN_SORA, "SC_TRANSFER"); offset += 4;
        require(_readU32Le(payload, offset) == DOMAIN_TRON, "SC_TRANSFER"); offset += 4;
        offset += 8;
        require(_readU32Le(payload, offset) == routeRevision, "SC_TRANSFER"); offset += 4;
        require(_readU32Le(payload, offset) == DOMAIN_SORA, "SC_TRANSFER"); offset += 4;
        require(_readU8(payload, offset++) == CODEC_TEXT, "SC_TRANSFER");
        uint256 start; uint256 length;
        (start, length, offset) = _readVec(payload, offset);
        require(_equals(payload, start, length, ASSET_ID), "SC_TRANSFER");
        amount = _readU128Le(payload, offset); offset += 16;
        require(amount != 0, "SC_TRANSFER");
        require(_readU8(payload, offset++) == CODEC_TEXT, "SC_TRANSFER");
        (start, length, offset) = _readVec(payload, offset);
        require(_canonicalTairaSenderRange(payload, start, length), "SC_TRANSFER");
        require(_readU8(payload, offset++) == CODEC_TRON21, "SC_TRANSFER");
        (start, length, offset) = _readVec(payload, offset);
        require(length == 21 && payload[start] == bytes1(0x41), "SC_TRANSFER");
        uint160 raw;
        for (uint256 i = 1; i < 21; i++) raw = (raw << 8) | uint160(uint8(payload[start + i]));
        require(raw != 0, "SC_TRANSFER"); recipient = address(raw);
        require(_readU8(payload, offset++) == CODEC_TEXT, "SC_TRANSFER");
        (start, length, offset) = _readVec(payload, offset);
        require(_equals(payload, start, length, ROUTE_ID), "SC_TRANSFER");
        require(offset == payload.length, "SC_TRANSFER");
    }

    function _networkIdWord(uint8 profile) private pure returns (bytes32) {
        if (profile == REPLAY_NETWORK_TRON) return bytes32(uint256(0x2b6653dc));
        revert("SC_DEPLOY");
    }
    function _requireDistinctDeploymentHashRoles(
        bytes32 configuredTokenCodeHash,
        bytes32 configuredReplayVerifierCodeHash,
        bytes32 configuredMintBreakerCodeHash,
        VerifierPolicyV1 memory policy
    ) private pure {
        require(
            configuredTokenCodeHash != configuredReplayVerifierCodeHash
                && configuredTokenCodeHash != configuredMintBreakerCodeHash
                && configuredReplayVerifierCodeHash != configuredMintBreakerCodeHash
                && configuredTokenCodeHash != policy.verifierCodeHash
                && configuredTokenCodeHash != policy.verifierKeyHash
                && configuredTokenCodeHash != policy.semanticProofProfileHash
                && configuredTokenCodeHash != policy.soraFinalityAnchorHash
                && configuredReplayVerifierCodeHash != policy.verifierCodeHash
                && configuredReplayVerifierCodeHash != policy.verifierKeyHash
                && configuredReplayVerifierCodeHash != policy.semanticProofProfileHash
                && configuredReplayVerifierCodeHash != policy.soraFinalityAnchorHash
                && configuredMintBreakerCodeHash != policy.verifierCodeHash
                && configuredMintBreakerCodeHash != policy.verifierKeyHash
                && configuredMintBreakerCodeHash != policy.semanticProofProfileHash
                && configuredMintBreakerCodeHash != policy.soraFinalityAnchorHash
                && policy.verifierCodeHash != policy.verifierKeyHash
                && policy.verifierCodeHash != policy.semanticProofProfileHash
                && policy.verifierCodeHash != policy.soraFinalityAnchorHash
                && policy.verifierKeyHash != policy.semanticProofProfileHash
                && policy.verifierKeyHash != policy.soraFinalityAnchorHash
                && policy.semanticProofProfileHash != policy.soraFinalityAnchorHash,
            "SC_DEPLOY"
        );
    }
    function _tronAddressWord(address account) private pure returns (bytes32) {
        return bytes32((uint256(0x41) << 160) | uint256(uint160(account)));
    }
    function _readU8(bytes memory value, uint256 offset) private pure returns (uint8) {
        require(offset < value.length, "SC_TRANSFER"); return uint8(value[offset]);
    }
    function _readU32Le(bytes memory value, uint256 offset) private pure returns (uint32 out) {
        require(offset <= value.length && value.length - offset >= 4, "SC_TRANSFER");
        for (uint256 i = 0; i < 4; i++) {
            out |= uint32(uint8(value[offset + i])) << uint32(i * 8);
        }
    }
    function _readU128Le(bytes memory value, uint256 offset) private pure returns (uint256 out) {
        require(offset <= value.length && value.length - offset >= 16, "SC_TRANSFER");
        for (uint256 i = 0; i < 16; i++) out |= uint256(uint8(value[offset + i])) << (i * 8);
    }
    function _readVec(bytes memory value, uint256 offset)
        private pure returns (uint256 start, uint256 length, uint256 next)
    {
        length = _readU32Le(value, offset); start = offset + 4;
        require(start <= value.length && value.length - start >= length, "SC_TRANSFER");
        next = start + length;
    }
    function _equals(bytes memory value, uint256 start, uint256 length, bytes memory expected)
        private pure returns (bool)
    {
        if (length != expected.length) return false;
        for (uint256 i = 0; i < length; i++) if (value[start + i] != expected[i]) return false;
        return true;
    }
    function _canonicalTairaSenderRange(bytes memory value, uint256 start, uint256 length)
        private pure returns (bool)
    {
        return SccpExactTransferCodec.isCanonicalTairaAccountRange(value, start, length);
    }
    function _codeHash(address account) private view returns (bytes32 codeHash) {
        assembly { codeHash := extcodehash(account) }
    }
    function _chainId() private pure returns (uint256 value) {
        assembly { value := chainid() }
    }
}
