// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {PlanEncoding} from "./PlanEncoding.sol";

/// Research-only atomic Base execution guard (ARB-028, issue #42).
///
/// This contract is never deployed to a public network. It holds no private
/// key, funds nothing on its own and never initiates a broadcast: `execute`
/// only pulls the plan's declared `spendingAccount` principal through an
/// existing ERC-20 allowance and swaps through the plan's own allowlisted
/// pools. Every pool referenced by the accompanying test harness is a
/// `MockV3Pool` test double, not a deployed Uniswap V3 pool.

/// One exact-input swap leg of a [Plan]. There is no `feeTier` field: this
/// harness's `MockV3Pool` has no fee tiers. `PlanEncoding`'s canonical digest
/// still includes a fee tier per leg, supplied separately (see
/// `PlanEncoding.sol`) so it can mirror `BasePlan::canonical_bytes` exactly.
struct Leg {
    address pool;
    address tokenIn;
    address tokenOut;
    uint256 exactIn;
    uint256 minOut;
}

/// A cyclic route that starts and ends in `startingAsset`, funded from
/// `spendingAccount`'s existing allowance to the guard that executes it.
struct Plan {
    address spendingAccount;
    address startingAsset;
    Leg[] legs;
    uint256 deadline;
    uint256 minFinalBalance;
}

interface IERC20 {
    function balanceOf(address account) external view returns (uint256);
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
}

interface IUniswapV3Pool {
    function swap(
        address recipient,
        bool zeroForOne,
        int256 amountSpecified,
        uint160 sqrtPriceLimitX96,
        bytes calldata data
    ) external returns (int256 amount0, int256 amount1);
}

interface IUniswapV3SwapCallback {
    function uniswapV3SwapCallback(int256 amount0Delta, int256 amount1Delta, bytes calldata data)
        external;
}

contract ArbGuard is IUniswapV3SwapCallback {
    // Uniswap V3 TickMath bounds, hardcoded rather than imported (this
    // harness carries no Uniswap dependency): swap's sqrtPriceLimitX96 must
    // sit strictly inside (MIN_SQRT_RATIO, MAX_SQRT_RATIO).
    uint160 private constant MIN_SQRT_RATIO = 4295128739;
    uint160 private constant MAX_SQRT_RATIO = 1461446703485210103287273052203988822378723970342;

    address public immutable owner;
    mapping(address => bool) public isAllowedPool;

    // Set for the duration of exactly one leg's swap call so the callback can
    // verify its caller is that leg's pool and the amount it reports. A plain
    // storage slot rather than EIP-1153 transient storage: readability over
    // the gas saving, and this artifact is never executed under gas
    // pressure. All three are cleared together, before the callback pays the
    // pool, so a second callback in the same swap (a malicious or buggy
    // pool) sees `activePool == address(0)` and reverts `UnauthorizedCallback`
    // instead of being paid again.
    address private activePool;
    address private activeTokenIn;
    uint256 private activeExactIn;

    error ZeroOwner();
    error NotOwner();
    error DeadlinePassed();
    error RouteTooShort();
    error RouteNotCyclic();
    error LegDiscontinuity(uint256 index);
    error ExactInMismatch(uint256 legIndex, uint256 received, uint256 exactIn);
    error ExactInTooLarge();
    error UnsupportedTarget(address target);
    error InsufficientPrincipal();
    error InsufficientOutput(uint256 legIndex, uint256 received, uint256 required);
    error InsufficientFinalBalance(uint256 required, uint256 actual);
    error SweepFailed();
    error UnauthorizedCallback(address caller);
    error CallbackAmountMismatch();

    event RouteExecuted(bytes32 digest, uint256 finalBalance);

    constructor(address owner_, address[] memory allowedPools) {
        if (owner_ == address(0)) revert ZeroOwner();
        owner = owner_;
        for (uint256 i = 0; i < allowedPools.length; i++) {
            isAllowedPool[allowedPools[i]] = true;
        }
    }

    /// Validates, then executes, the complete cyclic route in one call: any
    /// failing check or leg reverts the entire transaction, so principal is
    /// never pulled and no leg is ever left half-settled.
    function execute(Plan calldata plan) external {
        if (msg.sender != owner) revert NotOwner();
        if (block.timestamp > plan.deadline) revert DeadlinePassed();
        if (plan.legs.length < 2) revert RouteTooShort();
        if (
            plan.legs[0].tokenIn != plan.startingAsset
                || plan.legs[plan.legs.length - 1].tokenOut != plan.startingAsset
        ) {
            revert RouteNotCyclic();
        }
        for (uint256 i = 0; i + 1 < plan.legs.length; i++) {
            // Reports the LATER leg's index, mirroring
            // `crates/arb-evm/src/plan.rs`'s `LegDiscontinuity { index: index + 1 }`.
            if (plan.legs[i].tokenOut != plan.legs[i + 1].tokenIn) revert LegDiscontinuity(i + 1);
        }
        for (uint256 i = 0; i < plan.legs.length; i++) {
            if (!isAllowedPool[plan.legs[i].pool]) revert UnsupportedTarget(plan.legs[i].pool);
        }

        _pullPrincipal(plan.startingAsset, plan.spendingAccount, plan.legs[0].exactIn);

        uint256 received;
        for (uint256 i = 0; i < plan.legs.length; i++) {
            Leg calldata leg = plan.legs[i];
            // A later leg must spend exactly what the previous leg produced:
            // not just "enough" (`>=`), so nothing is ever left stranded in
            // the guard between legs.
            if (i > 0 && leg.exactIn != received) {
                revert ExactInMismatch(i, received, leg.exactIn);
            }
            received = _swapLeg(leg, i);
        }

        uint256 residual = IERC20(plan.startingAsset).balanceOf(address(this));
        if (residual > 0) {
            if (!IERC20(plan.startingAsset).transfer(plan.spendingAccount, residual)) {
                revert SweepFailed();
            }
        }
        uint256 finalBalance = IERC20(plan.startingAsset).balanceOf(plan.spendingAccount);
        if (finalBalance < plan.minFinalBalance) {
            revert InsufficientFinalBalance(plan.minFinalBalance, finalBalance);
        }

        emit RouteExecuted(_planDigest(plan), finalBalance);
    }

    /// The only place this guard ever pays a pool: it trusts `msg.sender`
    /// only while `activePool` names it, checks the amount the pool reports
    /// against what `_swapLeg` itself committed to (`activeTokenIn`/
    /// `activeExactIn`), never against attacker-suppliable `data`, and
    /// refuses to pay anything else. `activePool` (and the other two) are
    /// cleared BEFORE the payout, making this one-shot per swap: a pool that
    /// calls back a second time in the same `swap()` finds `activePool`
    /// already zeroed and gets `UnauthorizedCallback`, not a second payout.
    /// `data` is accepted only because the interface requires it; it is
    /// never read or trusted.
    function uniswapV3SwapCallback(int256 amount0Delta, int256 amount1Delta, bytes calldata)
        external
    {
        if (msg.sender != activePool) revert UnauthorizedCallback(msg.sender);
        address tokenIn = activeTokenIn;
        uint256 exactIn = activeExactIn;
        activePool = address(0);
        activeTokenIn = address(0);
        activeExactIn = 0;

        int256 positiveDelta = amount0Delta > amount1Delta ? amount0Delta : amount1Delta;
        if (positiveDelta <= 0 || uint256(positiveDelta) != exactIn) {
            revert CallbackAmountMismatch();
        }
        if (!IERC20(tokenIn).transfer(msg.sender, exactIn)) revert CallbackAmountMismatch();
    }

    function _swapLeg(Leg calldata leg, uint256 index) private returns (uint256 receivedAmount) {
        if (leg.exactIn > uint256(type(int256).max)) revert ExactInTooLarge();

        activePool = leg.pool;
        activeTokenIn = leg.tokenIn;
        activeExactIn = leg.exactIn;
        bool zeroForOne = leg.tokenIn < leg.tokenOut;
        uint160 sqrtPriceLimit = zeroForOne ? MIN_SQRT_RATIO + 1 : MAX_SQRT_RATIO - 1;
        uint256 balanceBefore = IERC20(leg.tokenOut).balanceOf(address(this));
        IUniswapV3Pool(leg.pool)
            .swap(address(this), zeroForOne, int256(leg.exactIn), sqrtPriceLimit, "");
        // Defensive: the callback already clears these before paying out, so
        // this is a no-op on the honest path. It only matters if a pool's
        // `swap()` returns without ever calling back, which would otherwise
        // leave a stale `activePool` armed for an unrelated later call.
        activePool = address(0);
        activeTokenIn = address(0);
        activeExactIn = 0;
        receivedAmount = IERC20(leg.tokenOut).balanceOf(address(this)) - balanceBefore;
        if (receivedAmount < leg.minOut) {
            revert InsufficientOutput(index, receivedAmount, leg.minOut);
        }
    }

    /// Wraps `transferFrom` so both a `false` return and a revert (an
    /// unfunded or non-standard token) become one `InsufficientPrincipal`
    /// classification: a funded fee account with no starting-asset principal
    /// must fail exactly the same way as an empty allowance.
    function _pullPrincipal(address token, address from, uint256 amount) private {
        (bool success, bytes memory data) = token.call(
            abi.encodeWithSelector(IERC20.transferFrom.selector, from, address(this), amount)
        );
        bool ok = success && (data.length == 0 || abi.decode(data, (bool)));
        if (!ok) revert InsufficientPrincipal();
    }

    /// `RouteExecuted.digest` IS NOT the off-chain `BasePlan` digest. Plainly:
    /// this is a guard-local EXECUTION digest, for observability only, built
    /// from only the data this on-chain `Plan` actually carries: principal is
    /// the amount actually pulled (`legs[0].exactIn`), fee tiers are zero
    /// (this harness's `Plan`/`Leg` carry none), allowances are empty and the
    /// callback pool set is the route's own legs. It is never compared
    /// against, and must never be assumed equal to, an off-chain `BasePlan`
    /// digest for the "same" economic route. The byte-for-byte encoder
    /// parity claim with
    /// `crates/arb-evm/src/plan.rs::BasePlan::canonical_bytes` is proven only
    /// by the fixtures in `test/PlanEncoding.t.sol` and
    /// `crates/arb-evm/tests/plan_parity.rs`, which call `PlanEncoding.digest`
    /// directly with the full field set (real principal, per-leg fee tiers,
    /// allowances and an arbitrary callback pool set) — not through this
    /// function.
    function _planDigest(Plan calldata plan) private view returns (bytes32) {
        uint64[] memory feeTiers = new uint64[](plan.legs.length);
        PlanEncoding.Allowance[] memory allowances = new PlanEncoding.Allowance[](0);
        address[] memory callbackPools = new address[](plan.legs.length);
        for (uint256 i = 0; i < plan.legs.length; i++) {
            callbackPools[i] = plan.legs[i].pool;
        }
        return PlanEncoding.digest(
            plan, block.chainid, plan.legs[0].exactIn, feeTiers, allowances, callbackPools
        );
    }
}
