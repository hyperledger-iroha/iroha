// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "../../evm/sccp/Ownable.sol";

/**
 * @title SccpBscSourceBridge
 * @dev Owner-governed BSC source emitter for SCCP source-chain proofs.
 *
 * Successful calls to `submitSccpSourceEvent(uint32,uint32,bytes32)` emit the
 * digest consumed by the BSC -> SORA source proof lane. The bridge owner should
 * be transferred to the route bridge during deployment so end users cannot emit
 * arbitrary SCCP source events.
 */
contract SccpBscSourceBridge is Ownable {
    uint32 private constant SCCP_DOMAIN_SORA = 0;
    uint32 private constant SCCP_DOMAIN_BSC = 2;
    bytes32 private constant SOURCE_BRIDGE_CONFIG_PREFIX =
        keccak256("iroha:sccp:bsc-source-bridge-config:v1");

    bytes32 public immutable networkId;
    uint32 public immutable sourceDomain;
    uint32 public immutable targetDomain;
    mapping(bytes32 => bool) public submittedSourceEvents;

    event SourceBridgeConfigured(
        address indexed bridge,
        bytes32 networkId,
        uint32 indexed sourceDomain,
        uint32 indexed targetDomain,
        address ownerAddress,
        bytes32 configHash
    );

    event SourceBridgeConfigHash(bytes32 indexed configHash, address ownerAddress);

    event SccpSourceEvent(bytes32 indexed sourceEventDigest);

    constructor(
        bytes32 configuredNetworkId,
        uint32 configuredSourceDomain,
        uint32 configuredTargetDomain
    ) {
        require(configuredNetworkId != bytes32(0), "Network id is required");
        require(
            configuredSourceDomain == SCCP_DOMAIN_BSC,
            "Source domain must be BSC"
        );
        require(
            configuredTargetDomain == SCCP_DOMAIN_SORA,
            "Target domain must be SORA"
        );
        require(
            configuredSourceDomain != configuredTargetDomain,
            "Source and target domains must differ"
        );

        networkId = configuredNetworkId;
        sourceDomain = configuredSourceDomain;
        targetDomain = configuredTargetDomain;

        emit SourceBridgeConfigured(
            address(this),
            configuredNetworkId,
            configuredSourceDomain,
            configuredTargetDomain,
            owner,
            _sourceBridgeConfigHashFor(
                configuredNetworkId,
                configuredSourceDomain,
                configuredTargetDomain,
                owner
            )
        );
    }

    function _sourceBridgeConfigHashFor(
        bytes32 configuredNetworkId,
        uint32 configuredSourceDomain,
        uint32 configuredTargetDomain,
        address ownerAddress
    )
        internal
        view
        returns (bytes32)
    {
        return keccak256(
            abi.encode(
                SOURCE_BRIDGE_CONFIG_PREFIX,
                address(this),
                configuredNetworkId,
                configuredSourceDomain,
                configuredTargetDomain,
                ownerAddress
            )
        );
    }

    function _sourceBridgeConfigHash() internal view returns (bytes32) {
        return _sourceBridgeConfigHashFor(
            networkId,
            sourceDomain,
            targetDomain,
            owner
        );
    }

    function sourceBridgeConfigHash() external view returns (bytes32) {
        return _sourceBridgeConfigHash();
    }

    function emitSourceBridgeConfigHash()
        external
        onlyOwner
        returns (bytes32)
    {
        bytes32 configHash = _sourceBridgeConfigHash();
        emit SourceBridgeConfigHash(configHash, owner);
        return configHash;
    }

    function _afterOwnershipTransferred(
        address,
        address newOwner
    )
        internal
        override
    {
        bytes32 configHash = _sourceBridgeConfigHashFor(
            networkId,
            sourceDomain,
            targetDomain,
            newOwner
        );
        emit SourceBridgeConfigHash(configHash, newOwner);
    }

    function submitSccpSourceEvent(
        uint32 eventSourceDomain,
        uint32 eventTargetDomain,
        bytes32 sourceEventDigest
    )
        external
        onlyOwner
        returns (bytes32)
    {
        require(
            eventSourceDomain == sourceDomain,
            "Unexpected source domain"
        );
        require(
            eventTargetDomain == targetDomain,
            "Unexpected target domain"
        );
        require(
            sourceEventDigest != bytes32(0),
            "Source event digest is required"
        );
        require(
            !submittedSourceEvents[sourceEventDigest],
            "Source event already submitted"
        );

        submittedSourceEvents[sourceEventDigest] = true;
        emit SccpSourceEvent(sourceEventDigest);
        return sourceEventDigest;
    }
}
