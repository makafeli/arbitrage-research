// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {ArbGuard, Plan, Leg, Allowance} from "../src/ArbGuard.sol";
import {PlanEncoding} from "../src/PlanEncoding.sol";
import {MockERC20} from "./mocks/MockERC20.sol";
import {MockV3Pool} from "./mocks/MockV3Pool.sol";
import {GreedyV3Pool} from "./mocks/GreedyV3Pool.sol";

/// Offline harness for `ArbGuard`: every pool is a `MockV3Pool`, every token
/// a `MockERC20`, no RPC, no fork, no `--fork-url`, no deploy script. Proves
/// the plan executes atomically and reverts on each required guard.
contract ArbGuardTest is Test {
    address internal owner;
    address internal spender;

    MockERC20 internal tokenA;
    MockERC20 internal tokenB;
    MockV3Pool internal poolAB;
    MockV3Pool internal poolBA;
    ArbGuard internal guard;

    uint256 internal constant PRINCIPAL = 1000;
    uint256 internal constant LEG0_OUT = 2000; // 1000 A * 2/1 through poolAB
    uint256 internal constant LEG1_OUT = 1050; // 2000 B * 21/40 through poolBA: +5% profit
    uint24 internal constant POOL_AB_FEE = 500;
    uint24 internal constant POOL_BA_FEE = 3000;

    function setUp() public {
        owner = makeAddr("owner");
        spender = makeAddr("spender");

        MockERC20 first = new MockERC20("Token First", "FIRST");
        MockERC20 second = new MockERC20("Token Second", "SECOND");
        (tokenA, tokenB) = address(first) < address(second) ? (first, second) : (second, first);

        (address token0, address token1) = _sorted(address(tokenA), address(tokenB));
        poolAB = new MockV3Pool(token0, token1, 2, 1, POOL_AB_FEE);
        poolBA = new MockV3Pool(token0, token1, 21, 40, POOL_BA_FEE);

        address[] memory allowedPools = new address[](2);
        allowedPools[0] = address(poolAB);
        allowedPools[1] = address(poolBA);
        guard = new ArbGuard(owner, allowedPools);

        tokenA.mint(spender, PRINCIPAL);
        vm.prank(spender);
        tokenA.approve(address(guard), PRINCIPAL);

        // Fund each pool with enough of its output token to settle a swap.
        tokenB.mint(address(poolAB), LEG0_OUT);
        tokenA.mint(address(poolBA), LEG1_OUT);
    }

    function _sorted(address x, address y) internal pure returns (address, address) {
        return x < y ? (x, y) : (y, x);
    }

    function _validPlan() internal view returns (Plan memory) {
        Leg[] memory legs = new Leg[](2);
        legs[0] = Leg({
            pool: address(poolAB),
            tokenIn: address(tokenA),
            tokenOut: address(tokenB),
            feeTier: POOL_AB_FEE,
            exactIn: PRINCIPAL,
            minOut: 1900
        });
        legs[1] = Leg({
            pool: address(poolBA),
            tokenIn: address(tokenB),
            tokenOut: address(tokenA),
            feeTier: POOL_BA_FEE,
            exactIn: LEG0_OUT,
            minOut: 1000
        });
        Allowance[] memory allowances = new Allowance[](1);
        allowances[0] =
            Allowance({token: address(tokenA), spender: address(guard), amount: PRINCIPAL});
        address[] memory callbackPools = new address[](2);
        callbackPools[0] = address(poolAB);
        callbackPools[1] = address(poolBA);
        return Plan({
            chainId: block.chainid,
            executor: address(guard),
            spendingAccount: spender,
            principal: PRINCIPAL,
            startingAsset: address(tokenA),
            legs: legs,
            allowances: allowances,
            deadline: block.timestamp + 1 days,
            minFinalBalance: PRINCIPAL + 50 - 1, // 1049: principal + profit - 1
            callbackPools: callbackPools
        });
    }

    function test_routeExecutesAtomicallyAndReturnsProfit() public {
        Plan memory plan = _validPlan();
        bytes32 expectedDigest = PlanEncoding.digest(plan);

        vm.expectEmit(true, true, true, true, address(guard));
        emit ArbGuard.RouteExecuted(expectedDigest, PRINCIPAL + 50);

        vm.prank(owner);
        guard.execute(plan);

        assertGe(tokenA.balanceOf(spender), plan.minFinalBalance);
        assertEq(tokenA.balanceOf(spender), PRINCIPAL + 50);
        assertEq(tokenA.balanceOf(address(guard)), 0);
        assertEq(tokenB.balanceOf(address(guard)), 0);
    }

    function test_revertsInsufficientFinalBalance() public {
        poolBA.setRate(9, 20); // 2000 B -> 900 A: unprofitable
        tokenA.mint(address(poolBA), 900); // pool must actually hold what it pays out
        Plan memory plan = _validPlan();
        plan.legs[1].minOut = 500; // leg-level check must not fire first
        plan.minFinalBalance = PRINCIPAL; // 1000, above the 900 this route now returns

        vm.expectRevert(
            abi.encodeWithSelector(
                ArbGuard.InsufficientFinalBalance.selector, plan.minFinalBalance, 900
            )
        );
        vm.prank(owner);
        guard.execute(plan);

        // Atomic rollback: spender keeps its original principal and poolAB
        // never actually received any A.
        assertEq(tokenA.balanceOf(spender), PRINCIPAL);
        assertEq(tokenA.balanceOf(address(poolAB)), 0);
    }

    function test_revertsUnauthorizedCallbackFromNonPool() public {
        vm.expectRevert(
            abi.encodeWithSelector(ArbGuard.UnauthorizedCallback.selector, address(this))
        );
        guard.uniswapV3SwapCallback(int256(1), int256(-1), abi.encode(address(tokenA), uint256(1)));

        vm.expectRevert(
            abi.encodeWithSelector(ArbGuard.UnauthorizedCallback.selector, address(poolAB))
        );
        vm.prank(address(poolAB));
        guard.uniswapV3SwapCallback(int256(1), int256(-1), abi.encode(address(tokenA), uint256(1)));
    }

    function test_revertsUnsupportedTarget() public {
        address[] memory onlyPoolAB = new address[](1);
        onlyPoolAB[0] = address(poolAB);
        ArbGuard restricted = new ArbGuard(owner, onlyPoolAB);

        Plan memory plan = _validPlan();
        plan.executor = address(restricted);
        vm.expectRevert(
            abi.encodeWithSelector(ArbGuard.UnsupportedTarget.selector, address(poolBA))
        );
        vm.prank(owner);
        restricted.execute(plan);
    }

    function test_revertsRouteNotCyclic() public {
        Plan memory plan = _validPlan();
        plan.legs[1].tokenOut = address(0xBEEF);

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.RouteNotCyclic.selector));
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsLegDiscontinuity() public {
        Plan memory plan = _validPlan();
        // legs[1].tokenIn no longer matches legs[0].tokenOut, but legs[1]
        // still ends in the starting asset so the cyclic check passes first.
        plan.legs[1].tokenIn = address(0xBEEF);

        // Reports the LATER leg's index (1), mirroring
        // `crates/arb-evm/src/plan.rs`'s `LegDiscontinuity { index: index + 1 }`.
        vm.expectRevert(abi.encodeWithSelector(ArbGuard.LegDiscontinuity.selector, uint256(1)));
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsRouteTooShort() public {
        Leg[] memory legs = new Leg[](1);
        legs[0] = Leg({
            pool: address(poolAB),
            tokenIn: address(tokenA),
            tokenOut: address(tokenB),
            feeTier: POOL_AB_FEE,
            exactIn: PRINCIPAL,
            minOut: 1900
        });
        Plan memory plan = Plan({
            chainId: block.chainid,
            executor: address(guard),
            spendingAccount: spender,
            principal: PRINCIPAL,
            startingAsset: address(tokenA),
            legs: legs,
            allowances: new Allowance[](0),
            deadline: block.timestamp + 1 days,
            minFinalBalance: 0,
            callbackPools: new address[](0)
        });

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.RouteTooShort.selector));
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsExactInMismatchWhenLegDoesNotSpendFullReceivedAmount() public {
        Plan memory plan = _validPlan();
        // A later leg must spend exactly what the previous leg produced
        // (LEG0_OUT), not merely "enough": leaving anything less stranded in
        // the guard is rejected even though it is technically <= received.
        plan.legs[1].exactIn = LEG0_OUT - 1;

        vm.expectRevert(
            abi.encodeWithSelector(
                ArbGuard.ExactInMismatch.selector, uint256(1), LEG0_OUT, LEG0_OUT - 1
            )
        );
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_deadlineEqualToBlockTimestampSucceeds() public {
        vm.warp(500);
        Plan memory plan = _validPlan();
        plan.deadline = 500; // exactly equal, not passed

        vm.prank(owner);
        guard.execute(plan);

        assertEq(tokenA.balanceOf(spender), PRINCIPAL + 50);
    }

    function test_revertsWhenPoolInvokesCallbackTwiceInOneSwap() public {
        (address token0, address token1) = _sorted(address(tokenA), address(tokenB));
        GreedyV3Pool greedyPool = new GreedyV3Pool(token0, token1, 2, 1, POOL_AB_FEE);
        greedyPool.setRepeatCallback(true);
        tokenB.mint(address(greedyPool), LEG0_OUT);

        address[] memory allowedPools = new address[](2);
        allowedPools[0] = address(greedyPool);
        allowedPools[1] = address(poolBA);
        ArbGuard greedyGuard = new ArbGuard(owner, allowedPools);

        vm.prank(spender);
        tokenA.approve(address(greedyGuard), PRINCIPAL);

        Plan memory plan = _validPlan();
        plan.executor = address(greedyGuard);
        plan.allowances[0].spender = address(greedyGuard);
        plan.legs[0].pool = address(greedyPool);
        plan.callbackPools[0] = address(greedyPool);

        // The pool's first callback is legitimate and gets paid; its second
        // callback in the same swap finds `activePool` already cleared and
        // is rejected, reverting the whole route atomically.
        vm.expectRevert(
            abi.encodeWithSelector(ArbGuard.UnauthorizedCallback.selector, address(greedyPool))
        );
        vm.prank(owner);
        greedyGuard.execute(plan);

        assertEq(tokenA.balanceOf(spender), PRINCIPAL);
    }

    function test_revertsCallbackAmountMismatchWhenPoolLiesAboutAmountIn() public {
        (address token0, address token1) = _sorted(address(tokenA), address(tokenB));
        GreedyV3Pool lyingPool = new GreedyV3Pool(token0, token1, 2, 1, POOL_AB_FEE);
        lyingPool.setLiedAmountIn(PRINCIPAL + 1); // real committed exactIn is PRINCIPAL
        tokenB.mint(address(lyingPool), LEG0_OUT);

        address[] memory allowedPools = new address[](2);
        allowedPools[0] = address(lyingPool);
        allowedPools[1] = address(poolBA);
        ArbGuard lyingGuard = new ArbGuard(owner, allowedPools);

        vm.prank(spender);
        tokenA.approve(address(lyingGuard), PRINCIPAL);

        Plan memory plan = _validPlan();
        plan.executor = address(lyingGuard);
        plan.allowances[0].spender = address(lyingGuard);
        plan.legs[0].pool = address(lyingPool);
        plan.callbackPools[0] = address(lyingPool);

        // The check compares the pool's reported delta against the amount
        // `_swapLeg` itself committed to (`activeExactIn`), never against
        // attacker-suppliable callback data, so a pool that lies is caught.
        vm.expectRevert(abi.encodeWithSelector(ArbGuard.CallbackAmountMismatch.selector));
        vm.prank(owner);
        lyingGuard.execute(plan);

        assertEq(tokenA.balanceOf(spender), PRINCIPAL);
    }

    function test_revertsSweepFailedWhenResidualTransferFails() public {
        Plan memory plan = _validPlan();
        tokenA.setFailTransfers(true, spender);

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.SweepFailed.selector));
        vm.prank(owner);
        guard.execute(plan);

        // Atomic rollback: nothing was pulled from spender either.
        assertEq(tokenA.balanceOf(spender), PRINCIPAL);
    }

    function test_revertsExactInTooLargeToCastToInt256() public {
        uint256 hugeAmount = uint256(type(int256).max) + 1;
        tokenA.mint(spender, hugeAmount);
        vm.prank(spender);
        tokenA.approve(address(guard), hugeAmount);

        Plan memory plan = _validPlan();
        plan.legs[0].exactIn = hugeAmount;
        plan.principal = hugeAmount;
        plan.allowances[0].amount = hugeAmount;

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.ExactInTooLarge.selector));
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_constructorRevertsZeroOwner() public {
        address[] memory pools = new address[](0);

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.ZeroOwner.selector));
        new ArbGuard(address(0), pools);
    }

    function test_revertsInsufficientOutput() public {
        Plan memory plan = _validPlan();
        plan.legs[1].minOut = 5000; // above the 1050 the pool actually pays

        vm.expectRevert(
            abi.encodeWithSelector(
                ArbGuard.InsufficientOutput.selector, uint256(1), LEG1_OUT, uint256(5000)
            )
        );
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsInsufficientPrincipalWhenOnlyFeeAccountIsFunded() public {
        // Allowance is present but the spending account holds zero starting
        // asset; only the owner (irrelevant: never checked) holds ETH.
        vm.deal(owner, 100 ether);
        address emptySpender = makeAddr("emptySpender");
        vm.prank(emptySpender);
        tokenA.approve(address(guard), PRINCIPAL);
        Plan memory plan = _validPlan();
        plan.spendingAccount = emptySpender;

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.InsufficientPrincipal.selector));
        vm.prank(owner);
        guard.execute(plan);

        // Balance present, allowance zero.
        address unapprovedSpender = makeAddr("unapprovedSpender");
        tokenA.mint(unapprovedSpender, PRINCIPAL);
        plan.spendingAccount = unapprovedSpender;

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.InsufficientPrincipal.selector));
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsDeadlinePassed() public {
        vm.warp(1_000);
        Plan memory plan = _validPlan();
        plan.deadline = 999;

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.DeadlinePassed.selector));
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsNotOwner() public {
        Plan memory plan = _validPlan();

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.NotOwner.selector));
        vm.prank(spender);
        guard.execute(plan);
    }

    function test_revertsWrongChain() public {
        Plan memory plan = _validPlan();
        plan.chainId = 999;

        vm.expectRevert(
            abi.encodeWithSelector(ArbGuard.WrongChain.selector, block.chainid, uint256(999))
        );
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsWrongExecutor() public {
        Plan memory plan = _validPlan();
        plan.executor = address(0xBEEF);

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.WrongExecutor.selector, address(0xBEEF)));
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsFeeTierMismatch() public {
        Plan memory plan = _validPlan();
        plan.legs[0].feeTier = 999;

        vm.expectRevert(
            abi.encodeWithSelector(
                ArbGuard.FeeTierMismatch.selector, uint256(0), POOL_AB_FEE, uint32(999)
            )
        );
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsFeeAccountOnly() public {
        Plan memory plan = _validPlan();
        plan.principal = 0;

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.FeeAccountOnly.selector));
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsDeclaredPrincipalAboveBalance() public {
        // legs[0].exactIn(PRINCIPAL) <= principal, but the spending account
        // never held more than PRINCIPAL: the declared principal is a
        // fiction the guard must reject before pulling anything.
        Plan memory plan = _validPlan();
        plan.principal = PRINCIPAL + 1000;

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.InsufficientPrincipal.selector));
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsMissingDeclaredAllowance() public {
        Plan memory plan = _validPlan();
        plan.allowances = new Allowance[](0);

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.MissingAllowance.selector));
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsUnexpectedAllowance() public {
        Plan memory plan = _validPlan();
        plan.allowances[0].spender = address(poolAB); // the old per-leg model

        vm.expectRevert(
            abi.encodeWithSelector(
                ArbGuard.UnexpectedAllowance.selector, address(tokenA), address(poolAB)
            )
        );
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsUnexpectedAllowanceWhenTokenIsWrong() public {
        // Kills the mutant that drops the `token == startingAsset` clause
        // from `isGuardAllowance`: spender is still correct, only the token
        // is the route's other leg.
        Plan memory plan = _validPlan();
        plan.allowances[0].token = address(tokenB);

        vm.expectRevert(
            abi.encodeWithSelector(
                ArbGuard.UnexpectedAllowance.selector, address(tokenB), address(guard)
            )
        );
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsUnexpectedAllowanceOnASecondIdenticalGuardEntry() public {
        // Kills the mutant that drops the `|| foundAllowance` clause: a
        // second entry, even a byte-for-byte copy of the one valid guard
        // allowance, must still be rejected.
        Plan memory plan = _validPlan();
        Allowance[] memory allowances = new Allowance[](2);
        allowances[0] = plan.allowances[0];
        allowances[1] = plan.allowances[0];
        plan.allowances = allowances;

        vm.expectRevert(
            abi.encodeWithSelector(
                ArbGuard.UnexpectedAllowance.selector, address(tokenA), address(guard)
            )
        );
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsMissingAllowanceWhenDeclaredAmountBelowExactIn() public {
        // Kills the mutant that hardcodes `sufficientAllowance = true`: token
        // and spender are correct, only the declared amount is one below
        // legs[0].exactIn.
        Plan memory plan = _validPlan();
        plan.allowances[0].amount = PRINCIPAL - 1;

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.MissingAllowance.selector));
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsInsufficientPrincipalWhenExactInExceedsDeclaredPrincipal() public {
        // Kills the mutant that drops the `exactIn > principal` clause: the
        // real balance and real allowance both still cover legs[0].exactIn,
        // only the declared `principal` field is one below it.
        Plan memory plan = _validPlan();
        plan.principal = PRINCIPAL - 1;

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.InsufficientPrincipal.selector));
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsCallbackPoolNotALeg() public {
        Plan memory plan = _validPlan();
        address[] memory callbackPools = new address[](3);
        callbackPools[0] = address(poolAB);
        callbackPools[1] = address(poolBA);
        callbackPools[2] = address(0xBEEF);
        plan.callbackPools = callbackPools;

        vm.expectRevert(
            abi.encodeWithSelector(ArbGuard.UnauthorizedCallback.selector, address(0xBEEF))
        );
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_revertsLegPoolMissingFromCallbacks() public {
        Plan memory plan = _validPlan();
        address[] memory callbackPools = new address[](1);
        callbackPools[0] = address(poolAB);
        plan.callbackPools = callbackPools;

        vm.expectRevert(
            abi.encodeWithSelector(ArbGuard.UnauthorizedCallback.selector, address(poolBA))
        );
        vm.prank(owner);
        guard.execute(plan);
    }

    function test_minFinalBalanceEncodesProfit() public {
        // The floor is an absolute check, not a profit target: it passes
        // exactly at the route's actual profit and reverts one wei above it.
        Plan memory planAtFloor = _validPlan();
        planAtFloor.minFinalBalance = PRINCIPAL + 50;

        vm.prank(owner);
        guard.execute(planAtFloor);
        assertEq(tokenA.balanceOf(spender), PRINCIPAL + 50);

        // Fresh spender/pools so the second execution starts from the same
        // principal instead of the first execution's profit.
        address secondSpender = makeAddr("secondSpender");
        tokenA.mint(secondSpender, PRINCIPAL);
        vm.prank(secondSpender);
        tokenA.approve(address(guard), PRINCIPAL);
        tokenB.mint(address(poolAB), LEG0_OUT);
        tokenA.mint(address(poolBA), LEG1_OUT);

        Plan memory planOverFloor = _validPlan();
        planOverFloor.spendingAccount = secondSpender;
        planOverFloor.minFinalBalance = PRINCIPAL + 51;

        vm.expectRevert(
            abi.encodeWithSelector(
                ArbGuard.InsufficientFinalBalance.selector, PRINCIPAL + 51, PRINCIPAL + 50
            )
        );
        vm.prank(owner);
        guard.execute(planOverFloor);
    }

    // Reads the fixture at `path`, strips its `expected_digest`'s `sha256:`
    // prefix and returns the raw digest as `bytes32`. Duplicated (not
    // shared) with `PlanEncodingTest._stripSha256Prefix`: each test file
    // parses and asserts the fixture independently, on purpose (see
    // `PlanEncoding.t.sol`'s file doc comment).
    function _stripSha256Prefix(string memory value) internal pure returns (string memory) {
        bytes memory raw = bytes(value);
        bytes memory prefix = bytes("sha256:");
        require(raw.length > prefix.length, "ArbGuardTest: digest too short");
        bytes memory stripped = new bytes(raw.length - prefix.length);
        for (uint256 i = prefix.length; i < raw.length; i++) {
            stripped[i - prefix.length] = raw[i];
        }
        return string(stripped);
    }

    /// Every literal address/amount pulled from `plan-digest.json`, grouped
    /// into a struct (a single stack slot wherever it is passed) so that
    /// deploying the fixture and building its `Plan` can be split into
    /// separate helper functions — reading all eleven fields as loose locals
    /// in one function hits solc's "stack too deep" limit.
    struct FixtureValues {
        address startingAsset;
        address intermediateToken;
        address spendingAccount;
        address executor;
        address pool0;
        address pool1;
        uint256 principal;
        uint256 exactIn0;
        uint256 minOut0;
        uint256 exactIn1;
        uint256 minOut1;
        uint24 feeTier0;
        uint24 feeTier1;
    }

    function _readFixtureValues(string memory json) internal pure returns (FixtureValues memory v) {
        v.startingAsset = vm.parseJsonAddress(json, ".starting_asset");
        v.intermediateToken = vm.parseJsonAddress(json, ".legs[0].token_out");
        v.spendingAccount = vm.parseJsonAddress(json, ".spending_account.address");
        v.executor = vm.parseJsonAddress(json, ".executor");
        v.pool0 = vm.parseJsonAddress(json, ".legs[0].pool");
        v.pool1 = vm.parseJsonAddress(json, ".legs[1].pool");
        v.principal = vm.parseJsonUint(json, ".spending_account.principal");
        v.exactIn0 = vm.parseJsonUint(json, ".legs[0].exact_in");
        v.minOut0 = vm.parseJsonUint(json, ".legs[0].min_out");
        v.exactIn1 = vm.parseJsonUint(json, ".legs[1].exact_in");
        v.minOut1 = vm.parseJsonUint(json, ".legs[1].min_out");
        v.feeTier0 = uint24(vm.parseJsonUint(json, ".legs[0].fee_tier"));
        v.feeTier1 = uint24(vm.parseJsonUint(json, ".legs[1].fee_tier"));
    }

    /// Deploys the fixture's tokens, pools and guard at their own literal
    /// addresses and funds them, then returns the guard's owner (needed to
    /// call `execute`). `deployCodeTo` runs each contract's real constructor
    /// at the target address (via `vm.etch` + a self-call), so `ArbGuard`'s
    /// immutable `owner` and its `isAllowedPool` mapping storage are set
    /// correctly — a plain `vm.etch` of runtime bytecode alone would not do
    /// this for either.
    function _deployFixtureContracts(FixtureValues memory v)
        internal
        returns (address fixtureOwner)
    {
        deployCodeTo(
            "MockERC20.sol:MockERC20", abi.encode("Starting Asset", "START"), v.startingAsset
        );
        deployCodeTo(
            "MockERC20.sol:MockERC20", abi.encode("Intermediate", "MID"), v.intermediateToken
        );

        (address token0, address token1) = _sorted(v.startingAsset, v.intermediateToken);
        // Rates chosen so each leg produces exactly its declared `exact_in`/
        // `min_out`: leg0 turns `exactIn0` into `exactIn1` (matching the next
        // leg's declared input, so `ExactInMismatch` never fires), and leg1
        // turns that into exactly `minOut1`, the fixture's own final floor.
        deployCodeTo(
            "MockV3Pool.sol:MockV3Pool",
            abi.encode(token0, token1, v.exactIn1, v.exactIn0, v.feeTier0),
            v.pool0
        );
        deployCodeTo(
            "MockV3Pool.sol:MockV3Pool",
            abi.encode(token0, token1, v.minOut1, v.exactIn1, v.feeTier1),
            v.pool1
        );

        address[] memory allowedPools = new address[](2);
        allowedPools[0] = v.pool0;
        allowedPools[1] = v.pool1;
        fixtureOwner = makeAddr("fixtureOwner");
        deployCodeTo("ArbGuard.sol:ArbGuard", abi.encode(fixtureOwner, allowedPools), v.executor);

        MockERC20(v.startingAsset).mint(v.spendingAccount, v.principal);
        vm.prank(v.spendingAccount);
        MockERC20(v.startingAsset).approve(v.executor, v.principal);
        MockERC20(v.intermediateToken).mint(v.pool0, v.minOut0);
        MockERC20(v.startingAsset).mint(v.pool1, v.minOut1);
    }

    function _buildFixturePlan(string memory json, FixtureValues memory v)
        internal
        pure
        returns (Plan memory plan)
    {
        Leg[] memory legs = new Leg[](2);
        legs[0] = Leg({
            pool: v.pool0,
            tokenIn: v.startingAsset,
            tokenOut: v.intermediateToken,
            feeTier: uint32(v.feeTier0),
            exactIn: v.exactIn0,
            minOut: v.minOut0
        });
        legs[1] = Leg({
            pool: v.pool1,
            tokenIn: v.intermediateToken,
            tokenOut: v.startingAsset,
            feeTier: uint32(v.feeTier1),
            exactIn: v.exactIn1,
            minOut: v.minOut1
        });
        Allowance[] memory allowances = new Allowance[](1);
        allowances[0] = Allowance({
            token: vm.parseJsonAddress(json, ".allowances[0].token"),
            spender: vm.parseJsonAddress(json, ".allowances[0].spender"),
            amount: vm.parseJsonUint(json, ".allowances[0].amount")
        });
        address[] memory callbackPools =
            vm.parseJsonAddressArray(json, ".callback_authorization.pools");
        plan = Plan({
            chainId: vm.parseJsonUint(json, ".chain_id"),
            executor: v.executor,
            spendingAccount: v.spendingAccount,
            principal: v.principal,
            startingAsset: v.startingAsset,
            legs: legs,
            allowances: allowances,
            deadline: vm.parseJsonUint(json, ".deadline_unix"),
            minFinalBalance: vm.parseJsonUint(json, ".min_final_balance"),
            callbackPools: callbackPools
        });
    }

    /// The linkage proof: the committed fixture plan executes through mock
    /// pools deployed/etched at its own literal addresses, and the guard's
    /// `RouteExecuted.digest` equals the fixture's own `expected_digest` —
    /// the same digest `crates/arb-evm/tests/plan_parity.rs` and
    /// `PlanEncoding.t.sol` check against `BasePlan::digest()`.
    function test_fixturePlanExecutesWithBasePlanDigest() public {
        string memory json = vm.readFile("test/fixtures/plan-digest.json");
        vm.chainId(vm.parseJsonUint(json, ".chain_id"));

        FixtureValues memory v = _readFixtureValues(json);
        address fixtureOwner = _deployFixtureContracts(v);
        Plan memory plan = _buildFixturePlan(json, v);

        string memory expectedDigestField = vm.parseJsonString(json, ".expected_digest");
        bytes32 expectedDigest =
            vm.parseBytes32(string.concat("0x", _stripSha256Prefix(expectedDigestField)));
        uint256 expectedFinalBalance = v.principal - v.exactIn0 + v.minOut1;

        vm.expectEmit(true, true, true, true, v.executor);
        emit ArbGuard.RouteExecuted(expectedDigest, expectedFinalBalance);

        vm.prank(fixtureOwner);
        ArbGuard(v.executor).execute(plan);

        assertEq(MockERC20(v.startingAsset).balanceOf(v.spendingAccount), expectedFinalBalance);
    }
}
