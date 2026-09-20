// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Plan} from "./ArbGuard.sol";

/// Canonical byte encoding for a [Plan], mirroring
/// `crates/arb-evm/src/plan.rs::BasePlan::canonical_bytes` word for word.
/// `Plan` is now a full field-for-field mirror of `BasePlan` (see
/// `ArbGuard.sol`), so this library takes only the plan itself: no
/// guard-supplied substitutes for principal, fee tiers, allowances or
/// callback pools.
library PlanEncoding {
    /// Deterministic ABI-style encoding: every field as a 32-byte
    /// big-endian word (addresses left-padded), dynamic arrays as a length
    /// word followed by their elements, in the same field order as
    /// `BasePlan::canonical_bytes` (see the field-order note on `BasePlan`).
    /// Built with `abi.encodePacked` over values pre-cast to `uint256`,
    /// never plain `abi.encode` (which inserts offsets) and never
    /// `abi.encode`'s struct/array framing.
    function canonicalBytes(Plan memory plan) internal pure returns (bytes memory) {
        bytes memory out = abi.encodePacked(
            plan.chainId,
            uint256(uint160(plan.executor)),
            uint256(uint160(plan.spendingAccount)),
            plan.principal,
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
                    uint256(plan.legs[i].feeTier),
                    plan.legs[i].exactIn,
                    plan.legs[i].minOut
                )
            );
        }
        out = bytes.concat(out, abi.encodePacked(plan.allowances.length));
        for (uint256 i = 0; i < plan.allowances.length; i++) {
            out = bytes.concat(
                out,
                abi.encodePacked(
                    uint256(uint160(plan.allowances[i].token)),
                    uint256(uint160(plan.allowances[i].spender)),
                    plan.allowances[i].amount
                )
            );
        }
        out = bytes.concat(
            out, abi.encodePacked(plan.deadline, plan.minFinalBalance, plan.callbackPools.length)
        );
        for (uint256 i = 0; i < plan.callbackPools.length; i++) {
            out = bytes.concat(out, abi.encodePacked(uint256(uint160(plan.callbackPools[i]))));
        }
        return out;
    }

    /// `sha256(canonicalBytes(plan))`, matching `BasePlan::digest`'s hash
    /// input exactly (the `"sha256:"` display prefix is a Rust-side/off-chain
    /// formatting concern, not part of this hash).
    function digest(Plan memory plan) internal pure returns (bytes32) {
        return sha256(canonicalBytes(plan));
    }
}
