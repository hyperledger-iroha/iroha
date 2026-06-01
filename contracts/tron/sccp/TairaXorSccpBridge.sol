// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

interface ITairaXorToken {
    function mint(address to, uint256 value) external returns (bool);
    function burnFrom(address from, uint256 value) external returns (bool);
}

interface ISccpTronSourceBridge {
    function submitSccpSourceEvent(
        uint32 eventSourceDomain,
        uint32 eventTargetDomain,
        bytes32 sourceEventDigest
    ) external returns (bytes32);
}

interface ISccpTronVerifierView {
    function destinationBindingHash() external view returns (bytes32);
    function networkId() external view returns (bytes32);
    function expectedSourceDomain() external view returns (uint32);
    function expectedTargetDomain() external view returns (uint32);
    function verifySccpMessageProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 submittedDestinationBindingHash
    )
        external
        view
        returns (
            bytes32 messageId,
            uint32 sourceDomain,
            bytes32 commitmentRoot
        );
}

/**
 * @title TairaXorSccpBridge
 * @dev Route-bound SCCP bridge for XOR between TAIRA and TRON.
 *
 * TAIRA-origin proofs are checked against the production TRON verifier binding,
 * then the bridged token is minted. TRON-origin exits burn the bridged token and
 * emit the SCCP source digest through `SccpTronSourceBridge`.
 */
contract TairaXorSccpBridge {
    uint32 private constant SCCP_DOMAIN_SORA = 0;
    uint32 private constant SCCP_DOMAIN_TRON = 5;
    bytes32 private constant TAIRA_XOR_TRANSFER_PAYLOAD_PREFIX =
        keccak256("iroha:sccp:taira-xor:transfer-payload:v1");
    bytes32 private constant TAIRA_XOR_BURN_SOURCE_EVENT_PREFIX =
        keccak256("iroha:sccp:taira-xor:burn-source-event:v1");

    ITairaXorToken public immutable token;
    ISccpTronVerifierView public immutable verifier;
    ISccpTronSourceBridge public immutable sourceBridge;
    bytes32 public immutable routeIdHash;
    bytes32 public immutable assetKeyHash;
    bytes32 public immutable networkId;
    bytes32 public immutable destinationBindingHash;

    mapping(bytes32 => bool) public usedMessageProofs;
    uint256 public burnNonce;

    event TairaXorMintFinalized(
        bytes32 indexed messageId,
        address indexed recipient,
        uint256 amount,
        bytes32 routeIdHash,
        bytes32 assetKeyHash,
        bytes32 payloadHash
    );

    event TairaXorBurnStarted(
        bytes32 indexed sourceEventDigest,
        address indexed burner,
        bytes32 indexed tairaRecipientHash,
        uint256 amount,
        uint256 nonce,
        bytes32 routeIdHash,
        bytes32 assetKeyHash,
        bytes tairaRecipient
    );

    constructor(
        address tokenAddress,
        address verifierAddress,
        address sourceBridgeAddress,
        bytes32 configuredRouteIdHash,
        bytes32 configuredAssetKeyHash
    ) {
        require(tokenAddress != address(0), "Token address is required");
        require(verifierAddress != address(0), "Verifier address is required");
        require(sourceBridgeAddress != address(0), "Source bridge address is required");
        require(configuredRouteIdHash != bytes32(0), "Route id hash is required");
        require(configuredAssetKeyHash != bytes32(0), "Asset key hash is required");

        ISccpTronVerifierView configuredVerifier =
            ISccpTronVerifierView(verifierAddress);
        require(
            configuredVerifier.expectedSourceDomain() == SCCP_DOMAIN_SORA,
            "Verifier source domain must be SORA"
        );
        require(
            configuredVerifier.expectedTargetDomain() == SCCP_DOMAIN_TRON,
            "Verifier target domain must be TRON"
        );
        bytes32 configuredNetworkId = configuredVerifier.networkId();
        bytes32 configuredDestinationBindingHash =
            configuredVerifier.destinationBindingHash();
        require(configuredNetworkId != bytes32(0), "Network id is required");
        require(
            configuredDestinationBindingHash != bytes32(0),
            "Destination binding hash is required"
        );

        token = ITairaXorToken(tokenAddress);
        verifier = configuredVerifier;
        sourceBridge = ISccpTronSourceBridge(sourceBridgeAddress);
        routeIdHash = configuredRouteIdHash;
        assetKeyHash = configuredAssetKeyHash;
        networkId = configuredNetworkId;
        destinationBindingHash = configuredDestinationBindingHash;
    }

    function finalizeFromTaira(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 submittedRouteIdHash,
        bytes32 submittedAssetKeyHash,
        address recipient,
        uint256 amount
    ) external returns (bytes32 messageId) {
        require(statementHash != bytes32(0), "Statement hash is required");
        require(submittedRouteIdHash == routeIdHash, "Unexpected route");
        require(submittedAssetKeyHash == assetKeyHash, "Unexpected asset");
        require(recipient != address(0), "Recipient address is required");
        require(amount != 0, "Amount is required");
        require(
            publicInputs[2] == _abiWordU32(SCCP_DOMAIN_TRON),
            "Unexpected target domain"
        );

        bytes32 payloadHash = tairaXorTransferPayloadHash(
            submittedRouteIdHash,
            submittedAssetKeyHash,
            recipient,
            amount
        );
        require(publicInputs[1] == payloadHash, "Payload hash mismatch");

        uint32 sourceDomain;
        bytes32 commitmentRoot;
        (messageId, sourceDomain, commitmentRoot) = verifier
            .verifySccpMessageProof(
                proofBytes,
                publicInputs,
                statementHash,
                destinationBindingHash
            );
        require(messageId != bytes32(0), "Verifier returned empty message id");
        require(sourceDomain == SCCP_DOMAIN_SORA, "Unexpected source domain");
        require(publicInputs[0] == messageId, "Public inputs message id mismatch");
        require(
            publicInputs[3] == commitmentRoot,
            "Public inputs commitment root mismatch"
        );
        require(!usedMessageProofs[messageId], "Message proof already used");

        usedMessageProofs[messageId] = true;
        require(token.mint(recipient, amount), "Token mint failed");

        emit TairaXorMintFinalized(
            messageId,
            recipient,
            amount,
            submittedRouteIdHash,
            submittedAssetKeyHash,
            payloadHash
        );
    }

    function burnToTaira(
        bytes32 submittedRouteIdHash,
        bytes32 submittedAssetKeyHash,
        bytes calldata tairaRecipient,
        uint256 amount
    ) external returns (bytes32 sourceEventDigest) {
        require(submittedRouteIdHash == routeIdHash, "Unexpected route");
        require(submittedAssetKeyHash == assetKeyHash, "Unexpected asset");
        require(tairaRecipient.length != 0, "TAIRA recipient is required");
        require(amount != 0, "Amount is required");
        require(burnNonce != uint256(-1), "Burn nonce exhausted");

        uint256 nonce = burnNonce;
        burnNonce = nonce + 1;
        bytes32 recipientHash = keccak256(tairaRecipient);
        sourceEventDigest = tairaXorBurnSourceEventDigest(
            submittedRouteIdHash,
            submittedAssetKeyHash,
            msg.sender,
            recipientHash,
            amount,
            nonce
        );

        require(token.burnFrom(msg.sender, amount), "Token burn failed");
        require(
            sourceBridge.submitSccpSourceEvent(
                SCCP_DOMAIN_TRON,
                SCCP_DOMAIN_SORA,
                sourceEventDigest
            ) == sourceEventDigest,
            "Source bridge submit failed"
        );

        emit TairaXorBurnStarted(
            sourceEventDigest,
            msg.sender,
            recipientHash,
            amount,
            nonce,
            submittedRouteIdHash,
            submittedAssetKeyHash,
            tairaRecipient
        );
    }

    function tairaXorTransferPayloadHash(
        bytes32 submittedRouteIdHash,
        bytes32 submittedAssetKeyHash,
        address recipient,
        uint256 amount
    ) public view returns (bytes32) {
        return keccak256(
            abi.encode(
                TAIRA_XOR_TRANSFER_PAYLOAD_PREFIX,
                submittedRouteIdHash,
                submittedAssetKeyHash,
                address(this),
                recipient,
                amount
            )
        );
    }

    function tairaXorBurnSourceEventDigest(
        bytes32 submittedRouteIdHash,
        bytes32 submittedAssetKeyHash,
        address burner,
        bytes32 tairaRecipientHash,
        uint256 amount,
        uint256 nonce
    ) public view returns (bytes32) {
        return keccak256(
            abi.encode(
                TAIRA_XOR_BURN_SOURCE_EVENT_PREFIX,
                submittedRouteIdHash,
                submittedAssetKeyHash,
                address(this),
                burner,
                tairaRecipientHash,
                amount,
                nonce
            )
        );
    }

    function _abiWordU32(uint32 value) private pure returns (bytes32 out) {
        out = bytes32(uint256(value));
    }
}
