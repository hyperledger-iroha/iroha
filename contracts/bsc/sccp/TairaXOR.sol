// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "../../evm/sccp/Ownable.sol";

/**
 * @title TairaXOR
 * @dev BEP20/ERC20-compatible bridged XOR token for the TAIRA <-> BSC SCCP route.
 *
 * Minting and bridge burns are intentionally delegated to one bridge contract.
 * The deployer can set that bridge during deployment and then lock it, so
 * production deployments do not depend on end-user key custody or generated
 * wallets.
 */
contract TairaXOR is Ownable {
    string public constant name = "TAIRA XOR";
    string public constant symbol = "TairaXOR";
    uint8 public constant decimals = 18;

    uint256 public totalSupply;
    address public bridge;
    bool public bridgeLocked;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);
    event BridgeUpdated(address indexed previousBridge, address indexed newBridge);
    event BridgeLocked(address indexed bridge);

    modifier onlyBridge() {
        require(msg.sender == bridge, "Caller is not the bridge");
        _;
    }

    function setBridge(address newBridge) external onlyOwner {
        require(!bridgeLocked, "Bridge is locked");
        require(newBridge != address(0), "Bridge address is required");
        address previousBridge = bridge;
        bridge = newBridge;
        emit BridgeUpdated(previousBridge, newBridge);
    }

    function lockBridge() external onlyOwner {
        require(!bridgeLocked, "Bridge is already locked");
        require(bridge != address(0), "Bridge address is required");
        bridgeLocked = true;
        emit BridgeLocked(bridge);
    }

    function transfer(address to, uint256 value) external returns (bool) {
        _transfer(msg.sender, to, value);
        return true;
    }

    function approve(address spender, uint256 value) external returns (bool) {
        require(spender != address(0), "Spender address is required");
        allowance[msg.sender][spender] = value;
        emit Approval(msg.sender, spender, value);
        return true;
    }

    function transferFrom(
        address from,
        address to,
        uint256 value
    ) external returns (bool) {
        uint256 currentAllowance = allowance[from][msg.sender];
        require(currentAllowance >= value, "Allowance exceeded");
        allowance[from][msg.sender] = _sub(currentAllowance, value);
        emit Approval(from, msg.sender, allowance[from][msg.sender]);
        _transfer(from, to, value);
        return true;
    }

    function mint(address to, uint256 value) external onlyBridge returns (bool) {
        require(to != address(0), "Recipient address is required");
        require(value != 0, "Amount is required");
        totalSupply = _add(totalSupply, value);
        balanceOf[to] = _add(balanceOf[to], value);
        emit Transfer(address(0), to, value);
        return true;
    }

    function burnFrom(address from, uint256 value) external onlyBridge returns (bool) {
        require(from != address(0), "Account address is required");
        require(value != 0, "Amount is required");
        balanceOf[from] = _sub(balanceOf[from], value);
        totalSupply = _sub(totalSupply, value);
        emit Transfer(from, address(0), value);
        return true;
    }

    function _transfer(address from, address to, uint256 value) private {
        require(from != address(0), "Sender address is required");
        require(to != address(0), "Recipient address is required");
        balanceOf[from] = _sub(balanceOf[from], value);
        balanceOf[to] = _add(balanceOf[to], value);
        emit Transfer(from, to, value);
    }

    function _add(uint256 left, uint256 right) private pure returns (uint256) {
        uint256 result = left + right;
        require(result >= left, "Uint256 overflow");
        return result;
    }

    function _sub(uint256 left, uint256 right) private pure returns (uint256) {
        require(left >= right, "Uint256 underflow");
        return left - right;
    }
}
