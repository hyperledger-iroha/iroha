// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "./SccpMessageBridge.sol";

contract SccpMessageBridgeDeployer {
    SccpMessageBridge public bridge;

    event NewSccpMessageBridgeDeployed(address bridgeAddress);

    function deploySccpMessageBridgeContract(
        address verifierAddress,
        string memory verifierBackendKey,
        string memory proofFamily,
        bytes32 networkId,
        uint32 sourceDomain,
        uint32 targetDomain
    ) public returns (address) {
        bridge = new SccpMessageBridge(
            verifierAddress,
            verifierBackendKey,
            proofFamily,
            networkId,
            sourceDomain,
            targetDomain
        );
        emit NewSccpMessageBridgeDeployed(address(bridge));
        return address(bridge);
    }
}
