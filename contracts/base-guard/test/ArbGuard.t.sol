// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {ArbGuard, Plan, Leg} from "../src/ArbGuard.sol";
import {PlanEncoding} from "../src/PlanEncoding.sol";
import {MockERC20} from "./mocks/MockERC20.sol";
import {MockV3Pool} from "./mocks/MockV3Pool.sol";

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

    function setUp() public {
        owner = makeAddr("owner");
        spender = makeAddr("spender");

        MockERC20 first = new MockERC20("Token First", "FIRST");
        MockERC20 second = new MockERC20("Token Second", "SECOND");
        (tokenA, tokenB) = address(first) < address(second) ? (first, second) : (second, first);

        (address token0, address token1) = _sorted(address(tokenA), address(tokenB));
        poolAB = new MockV3Pool(token0, token1, 2, 1);
        poolBA = new MockV3Pool(token0, token1, 21, 40);

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
            exactIn: PRINCIPAL,
            minOut: 1900
        });
        legs[1] = Leg({
            pool: address(poolBA),
            tokenIn: address(tokenB),
            tokenOut: address(tokenA),
            exactIn: LEG0_OUT,
            minOut: 1000
        });
        return Plan({
            spendingAccount: spender,
            startingAsset: address(tokenA),
            legs: legs,
            deadline: block.timestamp + 1 days,
            minFinalBalance: PRINCIPAL + 50 - 1 // 1049: principal + profit - 1
        });
    }

    function _expectedDigest(Plan memory plan) internal view returns (bytes32) {
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

    function test_routeExecutesAtomicallyAndReturnsProfit() public {
        Plan memory plan = _validPlan();
        bytes32 expectedDigest = _expectedDigest(plan);

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

        vm.expectRevert(abi.encodeWithSelector(ArbGuard.LegDiscontinuity.selector, uint256(0)));
        vm.prank(owner);
        guard.execute(plan);
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
}
