// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {Plan, Leg} from "../src/ArbGuard.sol";
import {PlanEncoding} from "../src/PlanEncoding.sol";

/// Proves `PlanEncoding.digest` produces the exact same bytes as the Rust
/// side's `BasePlan::digest()` for one literal, committed fixture plan. Both
/// this test and `crates/arb-evm/tests/plan_parity.rs` read
/// `test/fixtures/plan-digest.json` and check it against its own encoder;
/// this file never hand-computes or hardcodes the expected hash.
contract PlanEncodingTest is Test {
    string internal constant FIXTURE_PATH = "test/fixtures/plan-digest.json";

    function _stripSha256Prefix(string memory value) internal pure returns (string memory) {
        bytes memory raw = bytes(value);
        bytes memory prefix = bytes("sha256:");
        require(raw.length > prefix.length, "PlanEncodingTest: digest too short");
        bytes memory stripped = new bytes(raw.length - prefix.length);
        for (uint256 i = prefix.length; i < raw.length; i++) {
            stripped[i - prefix.length] = raw[i];
        }
        return string(stripped);
    }

    function test_fixtureDigestMatchesExpected() public {
        string memory json = vm.readFile(FIXTURE_PATH);

        uint256 chainId = vm.parseJsonUint(json, ".chain_id");
        address spendingAccount = vm.parseJsonAddress(json, ".spending_account.address");
        uint256 principal = vm.parseJsonUint(json, ".spending_account.principal");
        address startingAsset = vm.parseJsonAddress(json, ".starting_asset");
        uint256 deadline = vm.parseJsonUint(json, ".deadline_unix");
        uint256 minFinalBalance = vm.parseJsonUint(json, ".min_final_balance");

        Leg[] memory legs = new Leg[](2);
        uint64[] memory feeTiers = new uint64[](2);
        for (uint256 i = 0; i < 2; i++) {
            string memory base = string.concat(".legs[", vm.toString(i), "]");
            legs[i] = Leg({
                pool: vm.parseJsonAddress(json, string.concat(base, ".pool")),
                tokenIn: vm.parseJsonAddress(json, string.concat(base, ".token_in")),
                tokenOut: vm.parseJsonAddress(json, string.concat(base, ".token_out")),
                exactIn: vm.parseJsonUint(json, string.concat(base, ".exact_in")),
                minOut: vm.parseJsonUint(json, string.concat(base, ".min_out"))
            });
            feeTiers[i] = uint64(vm.parseJsonUint(json, string.concat(base, ".fee_tier")));
        }

        PlanEncoding.Allowance[] memory allowances = new PlanEncoding.Allowance[](2);
        for (uint256 i = 0; i < 2; i++) {
            string memory base = string.concat(".allowances[", vm.toString(i), "]");
            allowances[i] = PlanEncoding.Allowance({
                token: vm.parseJsonAddress(json, string.concat(base, ".token")),
                spender: vm.parseJsonAddress(json, string.concat(base, ".spender")),
                amount: vm.parseJsonUint(json, string.concat(base, ".amount"))
            });
        }

        address[] memory callbackPools =
            vm.parseJsonAddressArray(json, ".callback_authorization.pools");

        Plan memory plan = Plan({
            spendingAccount: spendingAccount,
            startingAsset: startingAsset,
            legs: legs,
            deadline: deadline,
            minFinalBalance: minFinalBalance
        });

        bytes32 actual =
            PlanEncoding.digest(plan, chainId, principal, feeTiers, allowances, callbackPools);

        string memory expectedDigestField = vm.parseJsonString(json, ".expected_digest");
        bytes32 expected =
            vm.parseBytes32(string.concat("0x", _stripSha256Prefix(expectedDigestField)));

        assertEq(actual, expected, "PlanEncoding.digest must match the committed fixture digest");
    }
}
