// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.7.4;
pragma experimental ABIEncoderV2;

import "./ISccpMessageVerifier.sol";
import "./SccpExactTransferCodec.sol";
import "./SccpSha256ReplayForest.sol";

interface ITairaXorExactEvmToken {
    function bridge() external view returns (address);
    function decimals() external view returns (uint8);
    function totalSupply() external view returns (uint256);
    function balanceOf(address account) external view returns (uint256);
    function mint(address to, uint256 value) external returns (bool);
    function burnFrom(address from, uint256 value) external returns (bool);
}

/** Route-bound, one-way 3-of-5 controller for stopping new wrapped mints. */
contract SccpEvmMintBreaker {
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

/**
 * @title TairaXorExactEvmSccpBridge
 * @dev Shared implementation for the concrete Ethereum and BSC XOR routes.
 * The shared base validates an immutable token bound to the precomputed route
 * address plus a closed SCCP network profile, and verifies the executing chain
 * id. There is no owner, arbitrary digest submission, mutable
 * route, configurable asset, signer committee, or proof bypass.
 */
abstract contract TairaXorExactEvmSccpBridge {
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
        uint32 externalDomain;
        uint8 networkProfile;
        uint32 routeRevision;
        uint256 externalChainId;
        bytes32 sourceLaneHash;
        bytes32 destinationLaneHash;
        address replayVerifierAddress;
        bytes32 replayVerifierCodeHash;
        address mintBreakerAddress;
        bytes32 mintBreakerCodeHash;
        uint256 maxWrappedSupply;
    }

    uint32 internal constant DOMAIN_SORA = 0;
    uint32 internal constant DOMAIN_ETHEREUM = 1;
    uint32 internal constant DOMAIN_BSC = 2;
    uint8 internal constant CODEC_TEXT = 0;
    uint8 internal constant CODEC_EVM20 = 1;
    uint32 private constant REPLAY_NETWORK_SORA = 0x40;
    uint32 private constant REPLAY_NETWORK_ETHEREUM = 0x41;
    uint32 private constant REPLAY_NETWORK_BSC = 0x42;
    uint8 private constant REPLAY_ACTOR_EVM = 1;
    uint8 private constant REPLAY_PRINCIPAL_EVM = 1;
    uint8 private constant REPLAY_EVM_SOURCE_BURN = 0x10;
    uint8 private constant REPLAY_EVM_DESTINATION_MINT = 0x11;
    uint256 internal constant TAIRA_TO_TOKEN_SCALE = 1000000000;
    uint256 private constant MAX_U128 = (uint256(1) << 128) - 1;
    bytes private constant ASSET_ID = "xor";
    bytes private constant ETHEREUM_ROUTE_ID = "taira_eth_xor";
    bytes private constant BSC_ROUTE_ID = "taira_bsc_xor";
    bytes32 private constant DESTINATION_BINDING_SEPARATOR =
        keccak256("iroha:sccp:evm-destination-binding:v1");
    bytes32 private constant ROUTE_CONFIG_SEPARATOR =
        keccak256("sccp:concrete-route-config:v1");
    bytes32 private constant VERIFIER_BACKEND_HASH = keccak256("evm-groth16-bn254-v1");
    bytes32 private constant EMPTY_CODE_HASH = keccak256("");

    ITairaXorExactEvmToken public immutable token;
    ISccpMessageVerifier public immutable verifier;
    uint32 public immutable externalDomain;
    uint8 public immutable networkProfile;
    uint32 public immutable routeRevision;
    uint256 public immutable externalChainId;
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
    SccpEvmMintBreaker public immutable mintBreaker;
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
        uint256 tokenAmount,
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
        require(_chainId() == externalChainId, "SC_CHAIN");
        _;
    }

    constructor(
        address tokenAddress,
        VerifierPolicyV1 memory configuredVerifierPolicy,
        uint32 configuredExternalDomain,
        uint8 configuredNetworkProfile,
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
        require(_profileDomain(configuredNetworkProfile) == configuredExternalDomain,
            "SC_DEPLOY");
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
        uint256 expectedChainId = _profileChainId(configuredNetworkProfile);
        require(_chainId() == expectedChainId, "SC_CHAIN");

        RouteDeploymentV1 memory deployment;
        deployment.tokenAddress = tokenAddress;
        deployment.verifierPolicy = configuredVerifierPolicy;
        deployment.externalDomain = configuredExternalDomain;
        deployment.networkProfile = configuredNetworkProfile;
        deployment.routeRevision = configuredRouteRevision;
        deployment.externalChainId = expectedChainId;
        deployment.maxWrappedSupply = configuredMaxWrappedSupply;
        ITairaXorExactEvmToken configuredToken = ITairaXorExactEvmToken(tokenAddress);
        require(configuredToken.bridge() == address(this), "SC_DEPLOY");
        require(configuredToken.decimals() == 18, "SC_DEPLOY");
        require(configuredToken.totalSupply() == 0, "SC_DEPLOY");
        deployment.tokenCodeHash = _codeHash(tokenAddress);
        require(
            deployment.tokenCodeHash != bytes32(0)
                && deployment.tokenCodeHash != EMPTY_CODE_HASH,
            "SC_DEPLOY");
        _validateVerifierPolicy(configuredVerifierPolicy);
        deployment.replayVerifierAddress = address(new SccpSha256ReplayForest());
        deployment.replayVerifierCodeHash = _codeHash(deployment.replayVerifierAddress);
        require(
            deployment.replayVerifierCodeHash != bytes32(0)
                && deployment.replayVerifierCodeHash != EMPTY_CODE_HASH,
            "SC_DEPLOY"
        );
        deployment.mintBreakerAddress = address(new SccpEvmMintBreaker(
            configuredMintGuardians
        ));
        deployment.mintBreakerCodeHash = _codeHash(deployment.mintBreakerAddress);
        _requireDistinctDeploymentHashRoles(
            deployment.tokenCodeHash,
            deployment.replayVerifierCodeHash,
            deployment.mintBreakerCodeHash,
            configuredVerifierPolicy
        );
        {
            bytes memory externalNetwork = _network(configuredNetworkProfile);
            bytes memory taira = SccpExactTransferCodec.tairaNetwork();
            deployment.sourceLaneHash = SccpExactTransferCodec.laneHashEvm(
                SccpExactTransferCodec.lane(externalNetwork, taira)
            );
            deployment.destinationLaneHash = SccpExactTransferCodec.laneHashEvm(
                SccpExactTransferCodec.lane(taira, externalNetwork)
            );
        }
        bytes32 exactDestinationBindingHash = _destinationBinding(deployment);
        bytes32 exactRouteConfigHash = _routeConfigurationHash(deployment);
        (bytes32 exactSourceReplayDomainHash, bytes32 exactDestinationReplayDomainHash) =
            _replayDomainHashes(
                deployment.replayVerifierAddress,
                configuredNetworkProfile,
                configuredRouteRevision,
                exactRouteConfigHash
            );

        token = configuredToken;
        verifier = ISccpMessageVerifier(configuredVerifierPolicy.verifierAddress);
        externalDomain = configuredExternalDomain;
        networkProfile = configuredNetworkProfile;
        routeRevision = configuredRouteRevision;
        externalChainId = expectedChainId;
        tokenCodeHash = deployment.tokenCodeHash;
        verifierCodeHash = configuredVerifierPolicy.verifierCodeHash;
        verifierKeyHash = configuredVerifierPolicy.verifierKeyHash;
        semanticProofProfileHash = configuredVerifierPolicy.semanticProofProfileHash;
        soraFinalityAnchorHash = configuredVerifierPolicy.soraFinalityAnchorHash;
        sourceLaneHash = deployment.sourceLaneHash;
        destinationLaneHash = deployment.destinationLaneHash;
        replayVerifier = SccpSha256ReplayForest(deployment.replayVerifierAddress);
        replayVerifierCodeHash = deployment.replayVerifierCodeHash;
        emptyReplayShardRoot = SccpSha256ReplayForest(
            deployment.replayVerifierAddress
        ).emptyShardRoot();
        sourceReplayDomainHash = exactSourceReplayDomainHash;
        destinationReplayDomainHash = exactDestinationReplayDomainHash;
        mintBreaker = SccpEvmMintBreaker(deployment.mintBreakerAddress);
        mintBreakerCodeHash = deployment.mintBreakerCodeHash;
        maxWrappedSupply = configuredMaxWrappedSupply;
        destinationBindingHash = exactDestinationBindingHash;
        routeConfigHash = exactRouteConfigHash;
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
            deployment.replayVerifierAddress,
            deployment.replayVerifierCodeHash,
            deployment.mintBreakerAddress,
            deployment.mintBreakerCodeHash
        ));
        bytes32 assetRouteConfigHash = keccak256(abi.encode(
            keccak256(ASSET_ID),
            keccak256(_routeIdForDomain(deployment.externalDomain)),
            deployment.routeRevision,
            TAIRA_TO_TOKEN_SCALE,
            deployment.maxWrappedSupply
        ));
        return keccak256(abi.encode(
            ROUTE_CONFIG_SEPARATOR,
            deployment.externalDomain,
            deployment.networkProfile,
            deployment.externalChainId,
            deployment.sourceLaneHash,
            deployment.destinationLaneHash,
            deploymentConfigHash,
            assetRouteConfigHash
        ));
    }

    function _replayDomainHashes(
        address configuredReplayVerifier,
        uint8 configuredNetworkProfile,
        uint32 configuredRouteRevision,
        bytes32 configuredRouteHash
    ) private view returns (bytes32 source, bytes32 destination) {
        bytes memory actor = abi.encodePacked(address(this));
        source = SccpSha256ReplayForest(configuredReplayVerifier).domainHash(
            uint32(configuredNetworkProfile),
            REPLAY_NETWORK_SORA,
            REPLAY_EVM_SOURCE_BURN,
            configuredRouteRevision,
            configuredRouteHash,
            REPLAY_ACTOR_EVM,
            actor
        );
        destination = SccpSha256ReplayForest(configuredReplayVerifier).domainHash(
            REPLAY_NETWORK_SORA,
            uint32(configuredNetworkProfile),
            REPLAY_EVM_DESTINATION_MINT,
            configuredRouteRevision,
            configuredRouteHash,
            REPLAY_ACTOR_EVM,
            actor
        );
    }

    /** Burn wrapped XOR and emit one exact external-EVM-to-Taira statement. */
    function transferToTaira(
        bytes calldata tairaRecipient,
        uint256 tokenAmount,
        bytes calldata replayWitness
    )
        external
        onExpectedChain
        nonReentrant
        returns (bytes32 messageId)
    {
        bytes memory recipient = tairaRecipient;
        require(
            SccpExactTransferCodec.isCanonicalTairaRecipient(recipient),
            "SC_TRANSFER"
        );
        require(tokenAmount != 0 && tokenAmount % TAIRA_TO_TOKEN_SCALE == 0,
            "SC_TRANSFER");
        uint256 tairaAmount = tokenAmount / TAIRA_TO_TOKEN_SCALE;
        require(tairaAmount != 0 && tairaAmount <= MAX_U128, "SC_TRANSFER");
        uint64 currentNonce = transferNonces[msg.sender];
        require(currentNonce != type(uint64).max, "SC_TRANSFER");
        require(_codeHash(address(token)) == tokenCodeHash, "SC_TOKEN");

        uint64 nonce = currentNonce;
        SccpExactTransferCodec.TransferFields memory fields;
        fields.sourceDomain = externalDomain;
        fields.destinationDomain = DOMAIN_SORA;
        fields.nonce = nonce;
        fields.routeRevision = routeRevision;
        fields.assetHomeDomain = DOMAIN_SORA;
        fields.assetId = ASSET_ID;
        fields.amount = tairaAmount;
        fields.senderCodec = CODEC_EVM20;
        fields.sender = abi.encodePacked(msg.sender);
        fields.recipientCodec = CODEC_TEXT;
        fields.recipient = recipient;
        fields.routeId = _routeId();
        bytes memory payload = SccpExactTransferCodec.transferPayload(fields);
        bytes memory exactLane = SccpExactTransferCodec.lane(
            _network(networkProfile), SccpExactTransferCodec.tairaNetwork()
        );
        messageId = SccpExactTransferCodec.messageId(exactLane, payload);
        bytes32 canonicalPayloadHash = SccpExactTransferCodec.payloadHashEvm(payload);
        bytes32 eventDigest = SccpExactTransferCodec.sourceEventDigest(
            sourceLaneHash, messageId, canonicalPayloadHash
        );

        transferNonces[msg.sender] = nonce + 1;
        SccpSha256ReplayForest.SccpAddressReplayRecord memory replayRecord =
            SccpSha256ReplayForest.SccpAddressReplayRecord({
                operation: REPLAY_EVM_SOURCE_BURN,
                replayId: messageId,
                payloadSha256: sha256(payload),
                amountScale9: uint128(tairaAmount),
                principalKind: REPLAY_PRINCIPAL_EVM,
                principal: msg.sender,
                auxiliaryIdentitySha256: sha256(abi.encodePacked(eventDigest))
            });
        _occupyReplay(true, replayRecord, replayWitness);
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

    /** Verify one exact Taira-to-external-EVM proof and mint wrapped XOR. */
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
        require(publicInputs[2] == bytes32(uint256(externalDomain)), "SC_PROOF");
        require(_codeHash(address(token)) == tokenCodeHash, "SC_TOKEN");
        bytes memory payload = canonicalPayloadBytes;
        (address recipient, uint256 tairaAmount) = _parseTairaToEvmTransfer(payload);
        bytes32 canonicalPayloadHash = SccpExactTransferCodec.payloadHashEvm(payload);
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

        SccpSha256ReplayForest.SccpAddressReplayRecord memory replayRecord =
            SccpSha256ReplayForest.SccpAddressReplayRecord({
                operation: REPLAY_EVM_DESTINATION_MINT,
                replayId: messageId,
                payloadSha256: sha256(payload),
                amountScale9: uint128(tairaAmount),
                principalKind: REPLAY_PRINCIPAL_EVM,
                principal: recipient,
                auxiliaryIdentitySha256: sha256(abi.encodePacked(destinationBindingHash))
            });
        _occupyReplay(false, replayRecord, replayWitness);
        _mutateTokenExact(recipient, tokenAmount, true);
        emit TairaXorMintFinalized(messageId, recipient, tokenAmount, canonicalPayloadHash);
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
                    && expectedSupply <= maxWrappedSupply - amount,
                "SC_TOKEN"
            );
            require(expectedBalance <= type(uint256).max - amount);
            expectedSupply += amount;
            expectedBalance += amount;
            require(token.mint(account, amount), "SC_TOKEN");
        } else {
            require(expectedSupply >= amount && expectedBalance >= amount);
            expectedSupply -= amount;
            expectedBalance -= amount;
            require(token.burnFrom(account, amount), "SC_TOKEN");
        }
        require(
            token.totalSupply() == expectedSupply && token.balanceOf(account) == expectedBalance,
            "SC_TOKEN"
        );
    }

    function sourceEventDigest(bytes32 messageId, bytes32 canonicalPayloadHash)
        external view returns (bytes32)
    {
        return SccpExactTransferCodec.sourceEventDigest(
            sourceLaneHash, messageId, canonicalPayloadHash
        );
    }

    function sccpPayloadHash(bytes calldata canonicalPayload) external view returns (bytes32) {
        bytes memory payload = canonicalPayload;
        return SccpExactTransferCodec.payloadHashEvm(payload);
    }

    function sccpDestinationMessageId(bytes calldata canonicalPayload)
        external view returns (bytes32)
    {
        bytes memory payload = canonicalPayload;
        return _destinationMessageId(payload);
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

    function _destinationBinding(RouteDeploymentV1 memory deployment)
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
            bytes32(deployment.externalChainId),
            uint256(DOMAIN_SORA),
            uint256(deployment.externalDomain),
            deployment.verifierPolicy.verifierAddress,
            address(this)
        );
        bytes memory suffix = abi.encode(
            deployment.verifierPolicy.verifierCodeHash,
            deployment.verifierPolicy.verifierKeyHash,
            deployment.verifierPolicy.semanticProofProfileHash,
            deployment.verifierPolicy.soraFinalityAnchorHash,
            deployment.replayVerifierAddress,
            deployment.replayVerifierCodeHash,
            deployment.mintBreakerAddress,
            deployment.mintBreakerCodeHash
        );
        return keccak256(abi.encodePacked(prefix, suffix));
    }

    function _destinationMessageId(bytes memory payload) private view returns (bytes32) {
        return SccpExactTransferCodec.messageId(
            SccpExactTransferCodec.lane(
                SccpExactTransferCodec.tairaNetwork(), _network(networkProfile)
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
        require(_verifyingKeyHash(address(verifier)) == verifierKeyHash, "SC_PROOF");
        require(
            _semanticProofProfileHash(address(verifier)) == semanticProofProfileHash,
            "SC_PROOF"
        );
        require(
            _soraFinalityAnchorHash(address(verifier)) == soraFinalityAnchorHash,
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

    function _parseTairaToEvmTransfer(bytes memory payload)
        private view returns (address recipient, uint256 amount)
    {
        uint256 offset = 0;
        require(_readU8(payload, offset++) == 0, "SC_PAYLOAD");
        require(_readU8(payload, offset++) == 1, "SC_PAYLOAD");
        require(_readU32Le(payload, offset) == DOMAIN_SORA, "SC_PAYLOAD"); offset += 4;
        require(_readU32Le(payload, offset) == externalDomain, "SC_PAYLOAD"); offset += 4;
        offset += 8; // nonce is committed by the exact lane message id
        require(_readU32Le(payload, offset) == routeRevision, "SC_PAYLOAD"); offset += 4;
        require(_readU32Le(payload, offset) == DOMAIN_SORA, "SC_PAYLOAD"); offset += 4;
        require(_readU8(payload, offset++) == CODEC_TEXT, "SC_PAYLOAD");
        uint256 start; uint256 length;
        (start, length, offset) = _readVec(payload, offset);
        require(_equals(payload, start, length, ASSET_ID), "SC_PAYLOAD");
        amount = _readU128Le(payload, offset); offset += 16;
        require(amount != 0, "SC_PAYLOAD");
        require(_readU8(payload, offset++) == CODEC_TEXT, "SC_PAYLOAD");
        (start, length, offset) = _readVec(payload, offset);
        require(_canonicalTairaSenderRange(payload, start, length), "SC_PAYLOAD");
        require(_readU8(payload, offset++) == CODEC_EVM20, "SC_PAYLOAD");
        (start, length, offset) = _readVec(payload, offset);
        require(length == 20, "SC_PAYLOAD");
        uint160 raw;
        for (uint256 i = 0; i < 20; i++) {
            raw = (raw << 8) | uint160(uint8(payload[start + i]));
        }
        require(raw != 0, "SC_PAYLOAD");
        recipient = address(raw);
        require(_readU8(payload, offset++) == CODEC_TEXT, "SC_PAYLOAD");
        (start, length, offset) = _readVec(payload, offset);
        require(_equals(payload, start, length, _routeId()), "SC_PAYLOAD");
        require(offset == payload.length, "SC_PAYLOAD");
    }

    function _routeId() private view returns (bytes memory) {
        return _routeIdForDomain(externalDomain);
    }

    function _routeIdForDomain(uint32 domain) private pure returns (bytes memory) {
        if (domain == DOMAIN_ETHEREUM) return ETHEREUM_ROUTE_ID;
        require(domain == DOMAIN_BSC, "SC_DEPLOY");
        return BSC_ROUTE_ID;
    }

    function _network(uint8 profile) private pure returns (bytes memory) {
        if (profile == REPLAY_NETWORK_ETHEREUM) {
            return SccpExactTransferCodec.ethereumNetwork(profile);
        }
        if (profile == REPLAY_NETWORK_BSC) return SccpExactTransferCodec.bscNetwork(profile);
        revert("SC_DEPLOY");
    }

    function _profileDomain(uint8 profile) private pure returns (uint32) {
        if (profile == REPLAY_NETWORK_ETHEREUM) return DOMAIN_ETHEREUM;
        if (profile == REPLAY_NETWORK_BSC) return DOMAIN_BSC;
        revert("SC_DEPLOY");
    }

    function _profileChainId(uint8 profile) private pure returns (uint256) {
        if (profile == REPLAY_NETWORK_ETHEREUM) return 1;
        if (profile == REPLAY_NETWORK_BSC) return 56;
        revert("SC_DEPLOY");
    }

    function _validateVerifierPolicy(VerifierPolicyV1 memory policy) private view {
        bytes32 actualCodeHash = _codeHash(policy.verifierAddress);
        require(policy.verifierCodeHash != bytes32(0)
            && actualCodeHash == policy.verifierCodeHash
            && actualCodeHash != EMPTY_CODE_HASH, "SC_DEPLOY");
        require(policy.verifierKeyHash != bytes32(0)
            && _verifyingKeyHash(policy.verifierAddress) == policy.verifierKeyHash,
            "SC_DEPLOY");
        require(
            _semanticProofProfileHash(policy.verifierAddress) ==
                policy.semanticProofProfileHash,
            "SC_DEPLOY"
        );
        require(
            _soraFinalityAnchorHash(policy.verifierAddress) == policy.soraFinalityAnchorHash,
            "SC_DEPLOY"
        );
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

    function _readU8(bytes memory value, uint256 offset) private pure returns (uint8) {
        require(offset < value.length, "SC_PAYLOAD");
        return uint8(value[offset]);
    }

    function _readU32Le(bytes memory value, uint256 offset) private pure returns (uint32 out) {
        require(offset <= value.length && value.length - offset >= 4, "SC_PAYLOAD");
        for (uint256 i = 0; i < 4; i++) {
            out |= uint32(uint8(value[offset + i])) << uint32(i * 8);
        }
    }

    function _readU128Le(bytes memory value, uint256 offset) private pure returns (uint256 out) {
        require(offset <= value.length && value.length - offset >= 16, "SC_PAYLOAD");
        for (uint256 i = 0; i < 16; i++) {
            out |= uint256(uint8(value[offset + i])) << (i * 8);
        }
    }

    function _readVec(bytes memory value, uint256 offset)
        private pure returns (uint256 start, uint256 length, uint256 next)
    {
        length = _readU32Le(value, offset);
        start = offset + 4;
        require(start <= value.length && value.length - start >= length, "SC_PAYLOAD");
        next = start + length;
    }

    function _equals(bytes memory value, uint256 start, uint256 length, bytes memory expected)
        private pure returns (bool)
    {
        if (length != expected.length) return false;
        for (uint256 i = 0; i < length; i++) {
            if (value[start + i] != expected[i]) return false;
        }
        return true;
    }

    function _canonicalTairaSenderRange(bytes memory value, uint256 start, uint256 length)
        private pure returns (bool)
    {
        return SccpExactTransferCodec.isCanonicalTairaAccountRange(value, start, length);
    }

    function _chainId() private pure returns (uint256 value) {
        assembly { value := chainid() }
    }

    function _codeHash(address account) private view returns (bytes32 codeHash) {
        assembly { codeHash := extcodehash(account) }
    }

    function _verifyingKeyHash(address account) private view returns (bytes32 keyHash) {
        (bool success, bytes memory data) = account.staticcall(
            abi.encodeWithSignature("verifyingKeyHash()")
        );
        require(success && data.length == 32, "SC_PROOF");
        keyHash = abi.decode(data, (bytes32));
    }

    function _semanticProofProfileHash(address account) private view returns (bytes32 profileHash) {
        (bool success, bytes memory data) = account.staticcall(
            abi.encodeWithSignature("semanticProofProfileHash()")
        );
        require(success && data.length == 32, "SC_PROOF");
        profileHash = abi.decode(data, (bytes32));
    }

    function _soraFinalityAnchorHash(address account) private view returns (bytes32 anchorHash) {
        (bool success, bytes memory data) = account.staticcall(
            abi.encodeWithSignature("soraFinalityAnchorHash()")
        );
        require(success && data.length == 32, "SC_PROOF");
        anchorHash = abi.decode(data, (bytes32));
    }
}
