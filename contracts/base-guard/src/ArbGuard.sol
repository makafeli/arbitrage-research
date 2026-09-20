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

/// One exact-input swap leg of a [Plan]. `feeTier` is compared against the
/// leg's pool's own `fee()` (`FeeTierMismatch`) and is part of the canonical
/// digest (see `PlanEncoding.sol`), mirroring
/// `crates/arb-evm/src/plan.rs::SwapLeg::fee_tier` exactly.
struct Leg {
    address pool;
    address tokenIn;
    address tokenOut;
    uint32 feeTier;
    uint256 exactIn;
    uint256 minOut;
}

/// A declared ERC-20 approval, part of the digest. The guard pays every pool
/// from its own balance in the swap callback; pools never pull. The only
/// allowance a plan should declare is `startingAsset: spendingAccount ->
/// executor` (see `execute`'s `MissingAllowance`/`UnexpectedAllowance`).
struct Allowance {
    address token;
    address spender;
    uint256 amount;
}

/// A cyclic route that starts and ends in `startingAsset`, funded from
/// `spendingAccount`'s existing allowance to the guard that executes it.
/// Full mirror of `crates/arb-evm/src/plan.rs::BasePlan`, field for field, in
/// the same order (see `PlanEncoding.canonicalBytes`).
struct Plan {
    uint256 chainId;
    address executor;
    address spendingAccount;
    uint256 principal;
    address startingAsset;
    Leg[] legs;
    Allowance[] allowances;
    uint256 deadline;
    uint256 minFinalBalance;
    address[] callbackPools;
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
    function fee() external view returns (uint24);
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
    error WrongChain(uint256 expected, uint256 actual);
    error WrongExecutor(address executor);
    error DeadlinePassed();
    error RouteTooShort();
    error RouteNotCyclic();
    error LegDiscontinuity(uint256 index);
    error ExactInMismatch(uint256 legIndex, uint256 received, uint256 exactIn);
    error ExactInTooLarge();
    error UnsupportedTarget(address target);
    error FeeTierMismatch(uint256 legIndex, uint24 poolFee, uint32 planFee);
    error FeeAccountOnly();
    error InsufficientPrincipal();
    error MissingAllowance();
    error UnexpectedAllowance(address token, address spender);
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
        if (plan.chainId != block.chainid) revert WrongChain(block.chainid, plan.chainId);
        if (plan.executor != address(this)) revert WrongExecutor(plan.executor);
        if (block.timestamp > plan.deadline) revert DeadlinePassed();
        // Rust's `validate()` allows a 1-leg cycle only in theory (a pool
        // that swaps a token for itself does not exist); the guard is
        // stricter and always requires at least two legs.
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
        for (uint256 i = 0; i < plan.legs.length; i++) {
            uint24 poolFee = IUniswapV3Pool(plan.legs[i].pool).fee();
            if (poolFee != plan.legs[i].feeTier) {
                revert FeeTierMismatch(i, poolFee, plan.legs[i].feeTier);
            }
        }

        if (plan.principal == 0) revert FeeAccountOnly();
        // Pre-accounting: the declared principal must really be there before
        // anything is pulled, mirroring `crates/arb-evm/src/plan.rs`'s
        // `InsufficientPrincipal` (`legs[0].exact_in > principal`), plus the
        // guard's own check that the declared principal is not a fiction.
        if (
            plan.legs[0].exactIn > plan.principal
                || IERC20(plan.startingAsset).balanceOf(plan.spendingAccount) < plan.principal
        ) {
            revert InsufficientPrincipal();
        }

        // Declared allowance: exactly one entry funding the guard, linking
        // the on-chain principal pull to the digest. This validates the
        // *declared* allowance only; the real allowance is still enforced by
        // `transferFrom` in `_pullPrincipal` below.
        bool foundAllowance;
        bool sufficientAllowance;
        for (uint256 i = 0; i < plan.allowances.length; i++) {
            Allowance calldata allowance = plan.allowances[i];
            bool isGuardAllowance =
                allowance.token == plan.startingAsset && allowance.spender == address(this);
            if (!isGuardAllowance || foundAllowance) {
                revert UnexpectedAllowance(allowance.token, allowance.spender);
            }
            foundAllowance = true;
            sufficientAllowance = allowance.amount >= plan.legs[0].exactIn;
        }
        if (!foundAllowance || !sufficientAllowance) revert MissingAllowance();

        // callbackPools must equal the set of leg pools: every callback pool
        // is a leg pool and every leg pool is listed.
        // ponytail: O(n^2) over a handful of legs
        for (uint256 i = 0; i < plan.callbackPools.length; i++) {
            address callbackPool = plan.callbackPools[i];
            bool isLegPool;
            for (uint256 j = 0; j < plan.legs.length; j++) {
                if (plan.legs[j].pool == callbackPool) {
                    isLegPool = true;
                    break;
                }
            }
            if (!isLegPool) revert UnauthorizedCallback(callbackPool);
        }
        for (uint256 i = 0; i < plan.legs.length; i++) {
            address legPool = plan.legs[i].pool;
            bool isListed;
            for (uint256 j = 0; j < plan.callbackPools.length; j++) {
                if (plan.callbackPools[j] == legPool) {
                    isListed = true;
                    break;
                }
            }
            if (!isListed) revert UnauthorizedCallback(legPool);
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

        // The digest IS the off-chain `BasePlan` digest now, proven equal by
        // `test_fixturePlanExecutesWithBasePlanDigest`.
        emit RouteExecuted(PlanEncoding.digest(plan), finalBalance);
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

    // `RouteExecuted.digest` is now the off-chain `BasePlan` digest: `Plan`
    // carries every field `BasePlan` does, so `PlanEncoding.digest(plan)` in
    // `execute` needs no guard-supplied substitutes. Proven equal by
    // `test_fixturePlanExecutesWithBasePlanDigest`, which executes the
    // committed fixture plan through mock pools and checks the fixture's own
    // `expected_digest` against the event this contract actually emits.
}
