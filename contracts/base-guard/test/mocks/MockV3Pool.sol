// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {IERC20, IUniswapV3SwapCallback} from "../../src/ArbGuard.sol";

/// Minimal Uniswap V3 pool test double for the offline ArbGuard harness: it
/// settles an exact-input swap at a fixed configurable rate and drives the
/// real `uniswapV3SwapCallback` path a production pool would use. It has no
/// tick math, no protocol fee and is never a deployed pool.
contract MockV3Pool {
    address public immutable token0;
    address public immutable token1;
    uint256 public rateNumerator;
    uint256 public rateDenominator;

    constructor(
        address token0_,
        address token1_,
        uint256 rateNumerator_,
        uint256 rateDenominator_
    ) {
        require(token0_ < token1_, "MockV3Pool: token order");
        token0 = token0_;
        token1 = token1_;
        rateNumerator = rateNumerator_;
        rateDenominator = rateDenominator_;
    }

    /// Lets a test make the next swap through this pool unprofitable.
    function setRate(uint256 rateNumerator_, uint256 rateDenominator_) external {
        rateNumerator = rateNumerator_;
        rateDenominator = rateDenominator_;
    }

    function swap(
        address recipient,
        bool zeroForOne,
        int256 amountSpecified,
        uint160,
        bytes calldata data
    ) external returns (int256 amount0, int256 amount1) {
        require(amountSpecified > 0, "MockV3Pool: exact input only");
        uint256 amountIn = uint256(amountSpecified);
        address tokenIn = zeroForOne ? token0 : token1;
        address tokenOut = zeroForOne ? token1 : token0;
        uint256 amountOut = (amountIn * rateNumerator) / rateDenominator;

        uint256 balanceInBefore = IERC20(tokenIn).balanceOf(address(this));
        require(IERC20(tokenOut).transfer(recipient, amountOut), "MockV3Pool: payout failed");

        if (zeroForOne) {
            amount0 = int256(amountIn);
            amount1 = -int256(amountOut);
        } else {
            amount1 = int256(amountIn);
            amount0 = -int256(amountOut);
        }
        IUniswapV3SwapCallback(msg.sender).uniswapV3SwapCallback(amount0, amount1, data);

        uint256 balanceInAfter = IERC20(tokenIn).balanceOf(address(this));
        require(balanceInAfter >= balanceInBefore + amountIn, "MockV3Pool: input not received");
    }
}
