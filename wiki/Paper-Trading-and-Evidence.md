# Paper trading and evidence

Paper mode models outcomes against virtual inventory. It has no trading key, signer or broadcast capability. The current scaffold provides explanatory fixtures and domain foundations; a real paper engine is still a delivery milestone.

## Evidence labels

| Label | What has been established |
|---|---|
| CANDIDATE | Exact local route quote/calculation against identified state, including negative results |
| SIMULATED | Successful complete intended atomic-transaction simulation for the exact plan/state |
| ESTIMATED_EXECUTABLE | Full simulation plus coherent fresh complete state, complete costs, principal/fee reservations, atomic guards/limits and a named delay/inclusion scenario |
| REALIZED | Actual LIVE submission reconciled under the configured chain settlement policy |

These are not guaranteed consecutive steps. A failed complete simulation is a rejection, not SIMULATED. Local quote arithmetic and mathematical replay remain CANDIDATE. An estimated executable paper result is hypothetical and never REALIZED. A realized transaction can lose money.

## Costs and inventory

The starting asset's route surplus is final output minus input and any financing fee. Convert it using a timestamped valuation policy and subtract external execution costs exactly once. Period operating costs are allocated separately. Pool swap fees and price impact already included in adapter output must not be subtracted again.

Maintain USDC trading principal and native ETH/SOL fee reserves separately. Virtual trades reserve capital, so mutually exclusive routes cannot all spend the same balance. Repeated observations of one persisting discrepancy form an opportunity episode, not unlimited independent fills. Record the assumptions used to reconcile counterfactual state with incoming real observations.

Unknown fees or valuation inputs block executable estimates. A fee cap describes affordability, not inclusion. Included failures, expired unlanded attempts and local rejections have different fee consequences. Do not turn them into one optimistic success denominator. Stablecoins are not unconditionally valued at one dollar.

## Delay and competition

Use declared scenarios: immediate-state diagnostic bound, measured-delay re-evaluation, competitive displacement and fee stress. These are sensitivity cases, not probability estimates or statistical confidence intervals. Record source resolution: end-of-block data cannot recreate unavailable intra-block events.

Split captured observations chronologically into development, calibration and untouched evaluation windows. Freeze the strategy before evaluating the holdout. Do not optimize on that window or treat multiple replays of one interval as independent observations.

## Comparing chains

Compare matched observation windows, route classes, capital/fee budgets and valuation rules. Report eligible pool-hours and downtime beside outcomes. Broader venue coverage is a sampling advantage, not proof of a better chain. Suppress aggregate ranking when comparable data is insufficient.

M3 requires at least one supported complete atomic route simulation on each chain under declared virtual funding. A synthetic fixture can establish mechanics but cannot enter a market-return report. Without that gate, label the release a quote-research preview.

The [trading and paper model](https://github.com/makafeli/arbitrage-research/blob/main/docs/04-TRADING-AND-PAPER-MODEL.md) defines exact cost treatment, fixtures and methodology; the [contracts](https://github.com/makafeli/arbitrage-research/blob/main/docs/08-DATA-AND-API-CONTRACTS.md) define serialized evidence fields.
