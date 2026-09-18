// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {Plan, Leg} from "../src/ArbGuard.sol";
import {PlanEncoding} from "../src/PlanEncoding.sol";

/// Proves `PlanEncoding.digest` produces the exact same bytes as the Rust
/// side's `BasePlan::digest()`, for two literal, committed fixture plans:
/// the original "regular" fixture (`plan-digest.json`) and a second
/// "irregular" one (`plan-digest-irregular.json`) whose principal,
/// allowances and callback-pool set are deliberately NOT equal to the legs'
/// own fields (see that fixture's own comment). A fixture built only from
/// coincidental equalities (principal == legs[0].exact_in, one allowance
/// per leg, callback pools == leg pools in leg order) cannot catch an
/// encoder that silently swaps or misorders those independent fields; the
/// irregular fixture can. Both this file and
/// `crates/arb-evm/tests/plan_parity.rs` read the same JSON files and check
/// them against their own encoder; this file never hand-computes or
/// hardcodes an expected hash.
contract PlanEncodingTest is Test {
    string internal constant FIXTURE_PATH = "test/fixtures/plan-digest.json";
    string internal constant FIXTURE_IRREGULAR_PATH = "test/fixtures/plan-digest-irregular.json";

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

    /// Counts entries in a top-level JSON array of objects (`.legs`,
    /// `.allowances`) by probing index existence, so the two fixtures can
    /// have different lengths (2 vs 3 legs, 2 vs 1 allowances) without this
    /// test hardcoding either.
    function _arrayLength(string memory json, string memory field)
        internal
        view
        returns (uint256 length)
    {
        while (vm.keyExistsJson(json, string.concat(".", field, "[", vm.toString(length), "]"))) {
            length++;
        }
    }

    function test_fixtureDigestMatchesExpected() public {
        _assertFixtureDigestMatches(FIXTURE_PATH);
    }

    function test_irregularFixtureDigestMatchesExpected() public {
        _assertFixtureDigestMatches(FIXTURE_IRREGULAR_PATH);
    }

    /// Reads `.legs` into `Leg[]`/`feeTiers[]` together, and `.allowances`
    /// into `Allowance[]`, in their own functions (rather than inline in
    /// `_assertFixtureDigestMatches`) purely to keep that function's local
    /// variable count low enough for solc's legacy codegen — with every
    /// field read inline in one function, compilation fails with "stack too
    /// deep" even though the logic itself is simple.
    function _parseLegs(string memory json)
        internal
        view
        returns (Leg[] memory legs, uint64[] memory feeTiers)
    {
        uint256 length = _arrayLength(json, "legs");
        legs = new Leg[](length);
        feeTiers = new uint64[](length);
        for (uint256 i = 0; i < length; i++) {
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
    }

    function _parseAllowances(string memory json)
        internal
        view
        returns (PlanEncoding.Allowance[] memory allowances)
    {
        uint256 length = _arrayLength(json, "allowances");
        allowances = new PlanEncoding.Allowance[](length);
        for (uint256 i = 0; i < length; i++) {
            string memory base = string.concat(".allowances[", vm.toString(i), "]");
            allowances[i] = PlanEncoding.Allowance({
                token: vm.parseJsonAddress(json, string.concat(base, ".token")),
                spender: vm.parseJsonAddress(json, string.concat(base, ".spender")),
                amount: vm.parseJsonUint(json, string.concat(base, ".amount"))
            });
        }
    }

    function _assertFixtureDigestMatches(string memory fixturePath) internal {
        string memory json = vm.readFile(fixturePath);

        (Leg[] memory legs, uint64[] memory feeTiers) = _parseLegs(json);
        PlanEncoding.Allowance[] memory allowances = _parseAllowances(json);
        address[] memory callbackPools =
            vm.parseJsonAddressArray(json, ".callback_authorization.pools");

        Plan memory plan = Plan({
            spendingAccount: vm.parseJsonAddress(json, ".spending_account.address"),
            startingAsset: vm.parseJsonAddress(json, ".starting_asset"),
            legs: legs,
            deadline: vm.parseJsonUint(json, ".deadline_unix"),
            minFinalBalance: vm.parseJsonUint(json, ".min_final_balance")
        });

        bytes32 actual = PlanEncoding.digest(
            plan,
            vm.parseJsonUint(json, ".chain_id"),
            vm.parseJsonUint(json, ".spending_account.principal"),
            feeTiers,
            allowances,
            callbackPools
        );

        string memory expectedDigestField = vm.parseJsonString(json, ".expected_digest");
        bytes32 expected =
            vm.parseBytes32(string.concat("0x", _stripSha256Prefix(expectedDigestField)));

        assertEq(actual, expected, "PlanEncoding.digest must match the committed fixture digest");
    }
}
