// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";

/// Proves the pinned foundry + forge-std toolchain runs in CI. Replaced by the
/// ArbGuard tests in the ARB-028 harness increment.
contract ToolchainTest is Test {
    function test_toolchainRuns() public pure {
        assertEq(uint256(1) + 1, 2);
    }
}
