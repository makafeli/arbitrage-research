// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {ArbGuard, Plan, Leg, Allowance} from "../src/ArbGuard.sol";
import {PlanEncoding} from "../src/PlanEncoding.sol";
import {MockERC20} from "./mocks/MockERC20.sol";

/// Read-only view surface of the real Uniswap V3 pool ABI needed to assert
/// fixture preconditions and to quote a swap's output directly (never
/// through `ArbGuard`). `ArbGuard.sol`'s own `IUniswapV3Pool` only declares
/// `swap`/`fee`, which is all the guard itself needs.
interface IUniswapV3PoolView {
    function swap(
        address recipient,
        bool zeroForOne,
        int256 amountSpecified,
        uint160 sqrtPriceLimitX96,
        bytes calldata data
    ) external returns (int256 amount0, int256 amount1);
    function fee() external view returns (uint24);
    function tickSpacing() external view returns (int24);
    function liquidity() external view returns (uint128);
    function token0() external view returns (address);
    function token1() external view returns (address);
    function slot0()
        external
        view
        returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint16 observationIndex,
            uint16 observationCardinality,
            uint16 observationCardinalityNext,
            uint8 feeProtocol,
            bool unlocked
        );
}

/// Offline harness that etches the REAL runtime code and storage of two
/// live Base Uniswap v3 WETH/USDC pools (fee 500 / tick spacing 10 at
/// `0xd0b53d9277642d899df5c87a3966a349a798f224`, fee 3000 / tick spacing 60
/// at `0x6c561b446416e1a00e8e93e221854d6ea4171372`), pinned at finalized
/// Base block 51569825 (timestamp 1789928997), and drives `ArbGuard.execute`
/// through both pools in one atomic call.
///
/// The pool bytecode and every fetched storage slot (`slot0`,
/// `feeGrowthGlobal{0,1}X128`, `protocolFees`, `liquidity`, the tick-bitmap
/// words and initialized ticks around the pinned tick, and the observation
/// slots a swap this small reads or writes) are real, taken from
/// `test/fixtures/mainnet/pools.json`
/// (`scripts/fetch_base_pool_fixtures.py`). Everything else is synthetic and
/// commented as such: `MockERC20` runtime code is etched at the REAL WETH
/// (`0x4200000000000000000000000000000000000006`) and USDC
/// (`0x833589fcd6edb6e08f4c7c32d4f71b54bda02913`) addresses (their real
/// contracts are never executed), both pools are minted a synthetic balance
/// of both tokens large enough to settle these swaps, and the spending
/// account's principal is a synthetic mint, not a real balance. No fork, no
/// RPC, no `--fork-url`, no deploy script, no key material. This contract is
/// never deployed to a public network and authorizes no live trade.
contract ArbGuardMainnetTest is Test {
    string internal constant FIXTURE_PATH = "test/fixtures/mainnet/pools.json";

    uint256 internal constant BASE_CHAIN_ID = 8453;
    // Fixture's own `provenance.block_timestamp`; +2 so the pool's oracle
    // write observes a later block than the one the fixture was pinned at.
    uint256 internal constant FIXTURE_BLOCK_TIMESTAMP = 1789928997;

    address internal constant WETH = 0x4200000000000000000000000000000000000006;
    address internal constant USDC = 0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913;
    address internal constant POOL_500 = 0xd0b53D9277642d899DF5C87A3966A349A798F224;
    address internal constant POOL_3000 = 0x6c561B446416E1A00E8E93E221854d6eA4171372;

    // Uniswap V3 TickMath bounds (identical values to `ArbGuard.sol`'s
    // private constants, duplicated here only for this file's own
    // direct-to-pool quoting calls, which never go through the guard).
    uint160 internal constant MIN_SQRT_RATIO = 4295128739;
    uint160 internal constant MAX_SQRT_RATIO = 1461446703485210103287273052203988822378723970342;

    // SYNTHETIC: leg0's exact input and the spending account's entire
    // starting balance (0.1 WETH), matching `ArbGuard.t.sol`'s pattern of
    // funding the spender with exactly `PRINCIPAL`.
    uint256 internal constant PRINCIPAL = 0.1 ether;
    // SYNTHETIC: large mock reserves minted to both pools so they can settle
    // either leg's output; far above what a 0.1 WETH swap needs against
    // this fixture's real liquidity.
    uint256 internal constant SYNTHETIC_POOL_WETH = 100_000 ether;
    uint256 internal constant SYNTHETIC_POOL_USDC = 1_000_000_000e6;

    address internal owner;
    address internal spender;
    ArbGuard internal guard;

    function setUp() public {
        vm.chainId(BASE_CHAIN_ID);
        vm.warp(FIXTURE_BLOCK_TIMESTAMP + 2);

        string memory json = vm.readFile(FIXTURE_PATH);
        address pool0 = _etchPool(json, 0);
        address pool1 = _etchPool(json, 1);
        require(pool0 == POOL_500, "fixture pools[0] address moved");
        require(pool1 == POOL_3000, "fixture pools[1] address moved");

        // SYNTHETIC: MockERC20 runtime code + real constructor at the REAL
        // WETH/USDC addresses. The real WETH/USDC contracts are never
        // executed; only their addresses are reused so the etched pool
        // bytecode's baked-in `token0`/`token1` immutables still resolve to
        // live ERC-20s.
        deployCodeTo("MockERC20.sol:MockERC20", abi.encode("Wrapped Ether", "WETH"), WETH);
        deployCodeTo("MockERC20.sol:MockERC20", abi.encode("USD Coin", "USDC"), USDC);

        // SYNTHETIC: both pools start at zero balance of the freshly-etched
        // mock tokens (etching code does not copy the donor's storage), so
        // fund each with both sides.
        MockERC20(WETH).mint(pool0, SYNTHETIC_POOL_WETH);
        MockERC20(USDC).mint(pool0, SYNTHETIC_POOL_USDC);
        MockERC20(WETH).mint(pool1, SYNTHETIC_POOL_WETH);
        MockERC20(USDC).mint(pool1, SYNTHETIC_POOL_USDC);

        owner = makeAddr("owner");
        spender = makeAddr("spender");

        address[] memory allowedPools = new address[](2);
        allowedPools[0] = POOL_500;
        allowedPools[1] = POOL_3000;
        guard = new ArbGuard(owner, allowedPools);

        // SYNTHETIC: spending account's entire starting balance, approved to
        // the guard in full, mirroring `ArbGuard.t.sol`'s `_validPlan` setup.
        MockERC20(WETH).mint(spender, PRINCIPAL);
        vm.prank(spender);
        MockERC20(WETH).approve(address(guard), PRINCIPAL);

        _assertFixturePreconditions(json);
    }

    // --- fixture loading ------------------------------------------------

    /// Etches `pools[poolIndex]`'s real runtime code, then replays every
    /// `storage` slot -> value pair from the fixture with `vm.store`.
    function _etchPool(string memory json, uint256 poolIndex) internal returns (address poolAddr) {
        string memory base = string.concat(".pools[", vm.toString(poolIndex), "]");
        poolAddr = vm.parseJsonAddress(json, string.concat(base, ".address"));
        bytes memory code = vm.parseJsonBytes(json, string.concat(base, ".code"));
        vm.etch(poolAddr, code);

        string memory storagePath = string.concat(base, ".storage");
        string[] memory slots = vm.parseJsonKeys(json, storagePath);
        for (uint256 i = 0; i < slots.length; i++) {
            // Each key is itself the hex slot number (e.g. "0x7f8"); each
            // value is a hex-encoded, non-zero-padded uint256 (e.g. "0x1"),
            // so both go through the numeric parsers rather than
            // `parseJsonBytes32`/`parseBytes32`, which require even-length,
            // fully-padded hex and reject most of these values outright.
            bytes32 slot = bytes32(vm.parseUint(slots[i]));
            bytes32 value =
                bytes32(vm.parseJsonUint(json, string.concat(storagePath, ".", slots[i])));
            vm.store(poolAddr, slot, value);
        }
    }

    /// Asserts both pools' live, on-chain `fee()`/`tickSpacing()`/`slot0()`/
    /// `liquidity()`/`token0()`/`token1()` still equal the fixture's values,
    /// proving the etch + storage replay reproduced real pool state exactly.
    /// Called once in `setUp`, and again after a reverted `execute` to prove
    /// the pool state rolled back.
    function _assertFixturePreconditions(string memory json) internal view {
        _assertPoolPreconditions(json, 0, POOL_500);
        _assertPoolPreconditions(json, 1, POOL_3000);
    }

    function _assertPoolPreconditions(string memory json, uint256 poolIndex, address poolAddr)
        internal
        view
    {
        string memory base = string.concat(".pools[", vm.toString(poolIndex), "]");
        IUniswapV3PoolView pool = IUniswapV3PoolView(poolAddr);

        uint24 expectedFee = uint24(vm.parseJsonUint(json, string.concat(base, ".immutables.fee")));
        int24 expectedTickSpacing =
            int24(vm.parseJsonInt(json, string.concat(base, ".immutables.tickSpacing")));
        address expectedToken0 =
            vm.parseJsonAddress(json, string.concat(base, ".immutables.token0"));
        address expectedToken1 =
            vm.parseJsonAddress(json, string.concat(base, ".immutables.token1"));
        uint256 expectedLiquidity = vm.parseJsonUint(json, string.concat(base, ".liquidity"));
        uint256 expectedSqrtPriceX96 =
            vm.parseJsonUint(json, string.concat(base, ".slot0.sqrt_price_x96"));
        int256 expectedTick = vm.parseJsonInt(json, string.concat(base, ".slot0.tick"));

        assertEq(pool.fee(), expectedFee, "fixture precondition: fee()");
        assertEq(
            int256(pool.tickSpacing()),
            int256(expectedTickSpacing),
            "fixture precondition: tickSpacing()"
        );
        assertEq(pool.token0(), expectedToken0, "fixture precondition: token0()");
        assertEq(pool.token1(), expectedToken1, "fixture precondition: token1()");
        assertEq(pool.token0(), WETH, "fixture precondition: token0() is WETH");
        assertEq(pool.token1(), USDC, "fixture precondition: token1() is USDC");
        assertEq(uint256(pool.liquidity()), expectedLiquidity, "fixture precondition: liquidity()");

        (uint160 sqrtPriceX96, int24 tick,,,,,) = pool.slot0();
        assertEq(
            uint256(sqrtPriceX96),
            expectedSqrtPriceX96,
            "fixture precondition: slot0().sqrtPriceX96"
        );
        assertEq(int256(tick), expectedTick, "fixture precondition: slot0().tick");
    }

    // --- plan building ----------------------------------------------------

    /// Two-leg cyclic route: `PRINCIPAL` WETH -> USDC on the 500 pool, then
    /// that USDC -> WETH on the 3000 pool. `minOut` is 0 on both legs (the
    /// route's floor is enforced once, via `minFinalBalance`); the caller
    /// supplies the observed `leg1Out` as the floor.
    function _twoLegPlan(uint256 leg0Out, uint256 leg1Out) internal view returns (Plan memory) {
        Leg[] memory legs = new Leg[](2);
        legs[0] = Leg({
            pool: POOL_500,
            tokenIn: WETH,
            tokenOut: USDC,
            feeTier: 500,
            exactIn: PRINCIPAL,
            minOut: 0
        });
        legs[1] = Leg({
            pool: POOL_3000,
            tokenIn: USDC,
            tokenOut: WETH,
            feeTier: 3000,
            exactIn: leg0Out,
            minOut: 0
        });
        Allowance[] memory allowances = new Allowance[](1);
        allowances[0] = Allowance({token: WETH, spender: address(guard), amount: PRINCIPAL});
        address[] memory callbackPools = new address[](2);
        callbackPools[0] = POOL_500;
        callbackPools[1] = POOL_3000;
        return Plan({
            chainId: block.chainid,
            executor: address(guard),
            spendingAccount: spender,
            principal: PRINCIPAL,
            startingAsset: WETH,
            legs: legs,
            allowances: allowances,
            deadline: block.timestamp + 1 days,
            minFinalBalance: leg1Out,
            callbackPools: callbackPools
        });
    }

    // --- direct-to-pool quoting (never through ArbGuard) -------------------

    /// Quotes `pool`'s output for an exact-input swap by calling `swap`
    /// directly from this test contract (paying the callback from a
    /// SYNTHETIC mint), then rolls the whole state change back with
    /// `vm.revertToState` so the quote never disturbs the pool the real
    /// `execute()` call below swaps through. Used only to learn the exact
    /// number `ArbGuard`'s `ExactInMismatch`/`InsufficientFinalBalance`
    /// checks require; asserted, not assumed, by `test_realPool*`.
    function _quoteExactIn(address pool, address tokenIn, address tokenOut, uint256 exactIn)
        internal
        returns (uint256 amountOut)
    {
        uint256 snapshotId = vm.snapshotState();

        MockERC20(tokenIn).mint(address(this), exactIn); // SYNTHETIC, reverted below
        bool zeroForOne = tokenIn < tokenOut;
        uint160 sqrtPriceLimit = zeroForOne ? MIN_SQRT_RATIO + 1 : MAX_SQRT_RATIO - 1;
        uint256 balanceBefore = MockERC20(tokenOut).balanceOf(address(this));
        IUniswapV3PoolView(pool)
            .swap(address(this), zeroForOne, int256(exactIn), sqrtPriceLimit, abi.encode(tokenIn));
        amountOut = MockERC20(tokenOut).balanceOf(address(this)) - balanceBefore;

        require(vm.revertToState(snapshotId), "quote: revertToState failed");
    }

    /// Callback for this test contract's own direct `_quoteExactIn` calls
    /// only. `ArbGuard.execute` never triggers this: the pool calls back
    /// whoever called `swap`, and in the real route below that is `guard`,
    /// which implements its own `uniswapV3SwapCallback`.
    function uniswapV3SwapCallback(int256 amount0Delta, int256 amount1Delta, bytes calldata data)
        external
    {
        address tokenIn = abi.decode(data, (address));
        int256 positiveDelta = amount0Delta > amount1Delta ? amount0Delta : amount1Delta;
        require(positiveDelta > 0, "quote callback: no amount owed");
        require(
            MockERC20(tokenIn).transfer(msg.sender, uint256(positiveDelta)),
            "quote callback: pay failed"
        );
    }

    // --- tests --------------------------------------------------------

    /// Proves the two-leg route executes atomically through the real,
    /// etched pools and the guard's `RouteExecuted` digest is the same
    /// `PlanEncoding`/`BasePlan` digest proven elsewhere against mock pools.
    function test_realPoolRouteExecutesThroughTheGuard() public {
        uint256 leg0Out = _quoteExactIn(POOL_500, WETH, USDC, PRINCIPAL);
        uint256 leg1Out = _quoteExactIn(POOL_3000, USDC, WETH, leg0Out);

        Plan memory plan = _twoLegPlan(leg0Out, leg1Out);
        bytes32 expectedDigest = PlanEncoding.digest(plan);

        vm.expectEmit(true, true, true, true, address(guard));
        emit ArbGuard.RouteExecuted(expectedDigest, leg1Out);

        vm.prank(owner);
        guard.execute(plan);

        assertEq(MockERC20(WETH).balanceOf(spender), leg1Out);
        assertGe(MockERC20(WETH).balanceOf(spender), plan.minFinalBalance);
        assertEq(MockERC20(WETH).balanceOf(address(guard)), 0);
        assertEq(MockERC20(USDC).balanceOf(address(guard)), 0);
    }

    /// Same route with `minFinalBalance` one wei above what it actually
    /// returns: `execute` must revert `InsufficientFinalBalance` and leave
    /// every balance, and both pools' own state, exactly as it was.
    function test_realPoolRouteRevertsWhenFinalBalanceIsShort() public {
        string memory json = vm.readFile(FIXTURE_PATH);

        uint256 leg0Out = _quoteExactIn(POOL_500, WETH, USDC, PRINCIPAL);
        uint256 leg1Out = _quoteExactIn(POOL_3000, USDC, WETH, leg0Out);

        Plan memory plan = _twoLegPlan(leg0Out, leg1Out);
        plan.minFinalBalance = leg1Out + 1; // one wei above the achievable final balance

        vm.expectRevert(
            abi.encodeWithSelector(ArbGuard.InsufficientFinalBalance.selector, leg1Out + 1, leg1Out)
        );
        vm.prank(owner);
        guard.execute(plan);

        assertEq(MockERC20(WETH).balanceOf(spender), PRINCIPAL);
        assertEq(MockERC20(WETH).balanceOf(address(guard)), 0);
        assertEq(MockERC20(USDC).balanceOf(address(guard)), 0);
        assertEq(MockERC20(WETH).balanceOf(POOL_500), SYNTHETIC_POOL_WETH);
        assertEq(MockERC20(USDC).balanceOf(POOL_500), SYNTHETIC_POOL_USDC);
        assertEq(MockERC20(WETH).balanceOf(POOL_3000), SYNTHETIC_POOL_WETH);
        assertEq(MockERC20(USDC).balanceOf(POOL_3000), SYNTHETIC_POOL_USDC);

        // Pool state itself rolled back too, not just token balances.
        _assertFixturePreconditions(json);
    }

    /// A leg's declared `feeTier` (3000) does not match the REAL `fee()`
    /// (500) of the pool it targets: rejected before any swap is attempted.
    function test_realPoolFeeTierMismatchIsRejected() public {
        Plan memory plan = _twoLegPlan(0, 0);
        plan.legs[0].feeTier = 3000;

        vm.expectRevert(
            abi.encodeWithSelector(
                ArbGuard.FeeTierMismatch.selector, uint256(0), uint24(500), uint32(3000)
            )
        );
        vm.prank(owner);
        guard.execute(plan);
    }

    /// No committed Rust-side golden output exists for these two real pools
    /// (only the synthetic mock-pool fixtures in `plan-digest*.json` have
    /// one), so this proves price-impact DIRECTION instead. Both legs are far
    /// too small, against this fixture's real liquidity, to cross a full
    /// tick boundary (`slot0().tick` is unchanged by either leg — verified
    /// below), so the direction is asserted on the continuous
    /// `slot0().sqrtPriceX96` price itself, not the discretized tick: leg 1
    /// sells WETH (token0) into the 500 pool, so its sqrt price moves lower
    /// than the pinned fixture price; leg 2 sells USDC (token1) into the
    /// 3000 pool for WETH, so its sqrt price moves higher than the pinned
    /// fixture price.
    function test_realPoolPriceImpactDirectionMatchesSoldTokenPerLeg() public {
        string memory json = vm.readFile(FIXTURE_PATH);
        uint256 fixtureSqrtPrice0 = vm.parseJsonUint(json, ".pools[0].slot0.sqrt_price_x96");
        uint256 fixtureSqrtPrice1 = vm.parseJsonUint(json, ".pools[1].slot0.sqrt_price_x96");
        int256 fixtureTick0 = vm.parseJsonInt(json, ".pools[0].slot0.tick");
        int256 fixtureTick1 = vm.parseJsonInt(json, ".pools[1].slot0.tick");

        uint256 leg0Out = _quoteExactIn(POOL_500, WETH, USDC, PRINCIPAL);
        uint256 leg1Out = _quoteExactIn(POOL_3000, USDC, WETH, leg0Out);
        Plan memory plan = _twoLegPlan(leg0Out, leg1Out);

        vm.prank(owner);
        guard.execute(plan);

        (uint160 sqrtPrice0After, int24 tick0After,,,,,) = IUniswapV3PoolView(POOL_500).slot0();
        (uint160 sqrtPrice1After, int24 tick1After,,,,,) = IUniswapV3PoolView(POOL_3000).slot0();

        // Both legs are small enough, against this fixture's real liquidity,
        // that the discretized tick does not move at all.
        assertEq(int256(tick0After), fixtureTick0);
        assertEq(int256(tick1After), fixtureTick1);

        assertLt(uint256(sqrtPrice0After), fixtureSqrtPrice0);
        assertGt(uint256(sqrtPrice1After), fixtureSqrtPrice1);
    }
}
