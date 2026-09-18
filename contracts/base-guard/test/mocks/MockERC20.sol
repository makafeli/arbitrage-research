// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {IERC20} from "../../src/ArbGuard.sol";

/// Minimal ERC-20 test double for the offline ArbGuard harness. `mint` is
/// test-only funding, never a production capability. `transferFrom` and
/// `transfer` return `false` on an insufficient balance or allowance instead
/// of reverting, matching common non-reverting ERC-20 behavior, so the
/// guard's translation of that failure into `InsufficientPrincipal` is
/// actually exercised.
contract MockERC20 is IERC20 {
    string public name;
    string public symbol;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    // Lets a test make one specific `transfer` destination fail (return
    // `false`, never revert) without breaking other transfers of the same
    // token to other recipients — e.g. `ArbGuard`'s residual sweep to the
    // spending account, while a pool's earlier payout to the guard itself
    // still succeeds.
    bool public failTransfers;
    address public failTransfersTo;

    constructor(string memory name_, string memory symbol_) {
        name = name_;
        symbol = symbol_;
    }

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    /// `setFailTransfers(true, recipient)` makes `transfer(recipient, ...)`
    /// return `false` from now on; `setFailTransfers(false, address(0))`
    /// undoes it.
    function setFailTransfers(bool shouldFail, address to) external {
        failTransfers = shouldFail;
        failTransfersTo = to;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        if (failTransfers && to == failTransfersTo) return false;
        return _transfer(msg.sender, to, amount);
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        uint256 allowed = allowance[from][msg.sender];
        if (allowed < amount) return false;
        if (!_transfer(from, to, amount)) return false;
        allowance[from][msg.sender] = allowed - amount;
        return true;
    }

    function _transfer(address from, address to, uint256 amount) private returns (bool) {
        if (balanceOf[from] < amount) return false;
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}
