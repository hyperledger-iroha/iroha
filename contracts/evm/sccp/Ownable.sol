// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

contract Ownable {
    address public owner;

    event OwnershipTransferred(
        address indexed previousOwner,
        address indexed newOwner
    );

    constructor() {
        owner = msg.sender;
        emit OwnershipTransferred(address(0), msg.sender);
    }

    modifier onlyOwner() {
        require(msg.sender == owner, "Caller is not the owner");
        _;
    }

    function transferOwnership(address newOwner) public onlyOwner {
        require(newOwner != address(0), "Owner address is required");
        address previousOwner = owner;
        emit OwnershipTransferred(previousOwner, newOwner);
        owner = newOwner;
        _afterOwnershipTransferred(previousOwner, newOwner);
    }

    function _afterOwnershipTransferred(address, address) internal virtual {}
}
