// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {IERC20, IUniswapV3SwapCallback} from "../../src/ArbGuard.sol";

/// A malicious Uniswap V3 pool test double, used only to prove `ArbGuard`'s
/// callback defenses: it never stands in for a well-behaved pool in a
/// positive-path test. Two independent misbehaviors, each opt-in via a
/// setter so a test exercises exactly one at a time:
///
/// - `repeatCallback`: invokes `uniswapV3SwapCallback` a second time after
///   the first, legitimate call. `ArbGuard` clears its active-pool state
///   before paying the first callback, so the second call must be rejected
///   as `UnauthorizedCallback`, not paid again.
/// - `liedAmountIn`: reports a different "amount in" to the callback than
///   the pool actually asked for, simulating a pool that lies about the
///   swap it is settling. `ArbGuard` must reject this as
///   `CallbackAmountMismatch` because it checks the callback's reported
///   amount against what it itself committed to before the call, never
///   against attacker-suppliable data.
///
/// It is never a deployed pool.
contract GreedyV3Pool {
    address public immutable token0;
    address public immutable token1;
    /// See `MockV3Pool.fee`: the fee tier `ArbGuard.execute` compares against
    /// each leg's declared `feeTier`.
    uint24 public immutable fee;
    uint256 public rateNumerator;
    uint256 public rateDenominator;
    bool public repeatCallback;
    uint256 public liedAmountIn;

    constructor(
        address token0_,
        address token1_,
        uint256 rateNumerator_,
        uint256 rateDenominator_,
        uint24 fee_
    ) {
        require(token0_ < token1_, "GreedyV3Pool: token order");
        token0 = token0_;
        token1 = token1_;
        rateNumerator = rateNumerator_;
        rateDenominator = rateDenominator_;
        fee = fee_;
    }

    /// When set, `swap` invokes the callback a second time with the same
    /// (correct) amounts, after the first call has already been paid.
    function setRepeatCallback(bool value) external {
        repeatCallback = value;
    }

    /// When nonzero, `swap` reports this amount as the "amount in" to the
    /// callback instead of the real `amountIn` it computed from
    /// `amountSpecified`.
    function setLiedAmountIn(uint256 value) external {
        liedAmountIn = value;
    }

    function swap(
        address recipient,
        bool zeroForOne,
        int256 amountSpecified,
        uint160,
        bytes calldata data
    ) external returns (int256 amount0, int256 amount1) {
        require(amountSpecified > 0, "GreedyV3Pool: exact input only");
        uint256 amountIn = uint256(amountSpecified);
        address tokenOut = zeroForOne ? token1 : token0;
        uint256 amountOut = (amountIn * rateNumerator) / rateDenominator;
        uint256 reportedIn = liedAmountIn == 0 ? amountIn : liedAmountIn;

        require(IERC20(tokenOut).transfer(recipient, amountOut), "GreedyV3Pool: payout failed");

        if (zeroForOne) {
            amount0 = int256(reportedIn);
            amount1 = -int256(amountOut);
        } else {
            amount1 = int256(reportedIn);
            amount0 = -int256(amountOut);
        }
        IUniswapV3SwapCallback(msg.sender).uniswapV3SwapCallback(amount0, amount1, data);
        if (repeatCallback) {
            IUniswapV3SwapCallback(msg.sender).uniswapV3SwapCallback(amount0, amount1, data);
        }
    }
}
