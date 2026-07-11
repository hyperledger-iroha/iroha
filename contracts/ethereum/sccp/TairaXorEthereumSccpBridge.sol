// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.24;

import "../../evm/sccp/TairaXorExactEvmSccpBridge.sol";
import "./TairaXOR.sol";

/** Concrete exact XOR route that atomically creates its Ethereum token. */
contract TairaXorEthereumSccpBridge is TairaXorExactEvmSccpBridge {
    constructor(
        VerifierPolicyV1 memory configuredVerifierPolicy,
        uint8 configuredEthereumProfile,
        uint32 configuredRouteRevision
    ) TairaXorExactEvmSccpBridge(
        address(new TairaXOR(address(this))),
        configuredVerifierPolicy,
        1,
        configuredEthereumProfile,
        configuredRouteRevision
    ) {}

    /** Return the exact SCCP Ethereum profile tag (`2` mainnet or `3` Sepolia). */
    function ethereumProfile() external view returns (uint8) {
        return networkProfile;
    }
}
