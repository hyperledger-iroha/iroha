// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "../../evm/sccp/TairaXorExactEvmSccpBridge.sol";

/** Concrete exact XOR transfer route for Ethereum mainnet or Sepolia. */
contract TairaXorEthereumSccpBridge is TairaXorExactEvmSccpBridge {
    constructor(
        address tokenAddress,
        address verifierAddress,
        bytes32 expectedVerifierCodeHash,
        bytes32 expectedVerifierKeyHash,
        bytes32 expectedSemanticProofProfileHash,
        bytes32 expectedSoraFinalityAnchorHash,
        uint8 configuredEthereumProfile,
        uint32 configuredRouteRevision
    ) TairaXorExactEvmSccpBridge(
        tokenAddress,
        verifierAddress,
        expectedVerifierCodeHash,
        expectedVerifierKeyHash,
        expectedSemanticProofProfileHash,
        expectedSoraFinalityAnchorHash,
        1,
        configuredEthereumProfile,
        configuredRouteRevision
    ) {}

    /** Return the exact SCCP Ethereum profile tag (`2` mainnet or `3` Sepolia). */
    function ethereumProfile() external view returns (uint8) {
        return networkProfile;
    }
}
