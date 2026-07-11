// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.24;

import "../../evm/sccp/TairaXorExactEvmSccpBridge.sol";
import "./TairaXOR.sol";

/** Concrete exact XOR route that atomically creates its BSC token. */
contract TairaXorBscSccpBridge is TairaXorExactEvmSccpBridge {
    constructor(
        VerifierPolicyV1 memory configuredVerifierPolicy,
        uint8 configuredBscProfile,
        uint32 configuredRouteRevision
    ) TairaXorExactEvmSccpBridge(
        address(new TairaXOR(address(this))),
        configuredVerifierPolicy,
        2,
        configuredBscProfile,
        configuredRouteRevision
    ) {}

    /** Return the exact SCCP BSC profile tag (`4` mainnet or `5` testnet). */
    function bscProfile() external view returns (uint8) {
        return networkProfile;
    }
}
