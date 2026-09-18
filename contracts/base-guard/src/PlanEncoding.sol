// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Plan} from "./ArbGuard.sol";

/// Canonical byte encoding for a [Plan], mirroring
/// `crates/arb-evm/src/plan.rs::BasePlan::canonical_bytes` word for word.
///
/// The on-chain `Plan`/`Leg` structs carry no `principal`, `feeTier`,
/// `allowances` or `callbackPools` field (see `ArbGuard.sol`): this library
/// takes them as extra parameters rather than adding fields to the guard's
/// execution-critical structs, so `ArbGuard.execute`'s calldata shape is
/// unaffected by this parity requirement.
library PlanEncoding {
    struct Allowance {
        address token;
        address spender;
        uint256 amount;
    }

    /// Deterministic ABI-style encoding: every field as a 32-byte
    /// big-endian word (addresses left-padded), dynamic arrays as a length
    /// word followed by their elements, in the same field order as
    /// `BasePlan::canonical_bytes`. Built with `abi.encodePacked` over values
    /// pre-cast to `uint256`, never plain `abi.encode` (which inserts
    /// offsets) and never `abi.encode`'s struct/array framing.
    function canonicalBytes(
        Plan memory plan,
        uint256 chainId,
        uint256 principal,
        uint64[] memory feeTiers,
        Allowance[] memory allowances,
        address[] memory callbackPools
    ) internal pure returns (bytes memory) {
        require(feeTiers.length == plan.legs.length, "PlanEncoding: feeTiers length");

        bytes memory out = abi.encodePacked(
            chainId,
            uint256(uint160(plan.spendingAccount)),
            principal,
            uint256(uint160(plan.startingAsset)),
            plan.legs.length
        );
        for (uint256 i = 0; i < plan.legs.length; i++) {
            out = bytes.concat(
                out,
                abi.encodePacked(
                    uint256(uint160(plan.legs[i].pool)),
                    uint256(uint160(plan.legs[i].tokenIn)),
                    uint256(uint160(plan.legs[i].tokenOut)),
                    uint256(feeTiers[i]),
                    plan.legs[i].exactIn,
                    plan.legs[i].minOut
                )
            );
        }
        out = bytes.concat(out, abi.encodePacked(allowances.length));
        for (uint256 i = 0; i < allowances.length; i++) {
            out = bytes.concat(
                out,
                abi.encodePacked(
                    uint256(uint160(allowances[i].token)),
                    uint256(uint160(allowances[i].spender)),
                    allowances[i].amount
                )
            );
        }
        out = bytes.concat(
            out, abi.encodePacked(plan.deadline, plan.minFinalBalance, callbackPools.length)
        );
        for (uint256 i = 0; i < callbackPools.length; i++) {
            out = bytes.concat(out, abi.encodePacked(uint256(uint160(callbackPools[i]))));
        }
        return out;
    }

    /// `sha256(canonicalBytes(...))`, matching `BasePlan::digest`'s hash
    /// input exactly (the `"sha256:"` display prefix is a Rust-side/off-chain
    /// formatting concern, not part of this hash).
    function digest(
        Plan memory plan,
        uint256 chainId,
        uint256 principal,
        uint64[] memory feeTiers,
        Allowance[] memory allowances,
        address[] memory callbackPools
    ) internal pure returns (bytes32) {
        return sha256(canonicalBytes(plan, chainId, principal, feeTiers, allowances, callbackPools));
    }
}
