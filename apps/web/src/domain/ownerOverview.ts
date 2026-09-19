// Pure, framework-free derivation logic for the plain-language Owner Overview page (issue #182).
// No React here and no JSON import here: keeps this file runnable by `node --test` without
// Node's ESM JSON-import-attribute rules, and keeps the progress-file shape a caller concern.
import type { Session, CommandReceipt, Network, SessionState } from '../api/client.ts';
import type { CollectionCoverage } from '../api/collection.ts';
import type { Coverage, DecisionGroup, StoredDecision, Origin } from '../api/research.ts';
import { record, text, requireValue } from '../api/research.ts';
import type { StoredCostAssessment } from '../api/costs.ts';

export type Lang = 'nl' | 'en';

// ---- Copy table -----------------------------------------------------------
// Every enum value the owner can see must have a plain-language entry here in both languages.
// Technical values (ids, digests, exact numbers) are never looked up in this table.
export const copy = {
  nl: {
    mode_OBSERVE: 'kijkt alleen mee (OBSERVE)', mode_PAPER: 'oefent met nepgeld (PAPER)', mode_REPLAY: 'speelt oude data af (REPLAY)', mode_LIVE: 'handelt echt (LIVE)',
    state_RECOVERING: 'herstelt van een herstart', state_STOPPED: 'gestopt', state_RUNNING: 'actief', state_PAUSING: 'wordt gepauzeerd', state_PAUSED: 'gepauzeerd', state_DRAINING: 'ronden lopende taken af', state_FAULTED: 'in storing',
    worker_no_heartbeat: 'nog geen hartslag ontvangen van de werker',
    worker_not_reachable: 'niet bereikbaar',
    lag_unknown: 'onbekend — nog geen verzameling ontvangen', lag_caught_up: 'is bij (recent bijgewerkt)', lag_catching_up: 'loopt in (haalt achterstand in)', lag_stale: 'loopt vast — meer dan 5 minuten geen verzameling', lag_not_collecting: 'gestopt, verzamelt niet',
    receipt_none: 'nog geen commando vanuit dit venster verstuurd; de dienst biedt geen volledige commandogeschiedenis',
    receipt_status_PENDING: 'in behandeling', receipt_status_APPLIED: 'toegepast', receipt_status_REJECTED: 'geweigerd', receipt_status_SUPERSEDED: 'ingehaald door een nieuwer commando',
    action_START: 'start', action_PAUSE: 'pauzeren', action_RESUME: 'hervatten', action_STOP: 'stoppen', action_DISARM: 'ontwapenen',
    health_red: 'rood', health_amber: 'oranje', health_green: 'groen', health_unknown: 'onbekend',
    health_reason_fault: 'de sessie staat in storing', health_reason_unreachable: 'de werker is niet bereikbaar', health_reason_stopped: 'de sessie verzamelt momenteel niet (gestopt of gepauzeerd)', health_reason_stale: 'meer dan 5 minuten geen verzameling ontvangen', health_reason_catching_up: 'de verzameling loopt nog achterstand in', health_reason_ok: 'geen storing, geen achterstand, en er is minstens één verzameling toegelaten', health_reason_no_data: 'nog geen verzameling toegelaten om te beoordelen',
    health_failed_attempts_note: 'mislukte pogingen worden geteld sinds het begin van de sessie; providerfouten tellen hierin mee en worden niet apart getoond',
    findings_evidence_label: 'kandidaat — niet gesimuleerd',
    whatif_caveat_not_executed: 'OBSERVE-kandidaten worden niet uitgevoerd, niet volledig gesimuleerd en zijn geen winst.',
    whatif_no_leverage: 'Geen hefboom of flashlening getoond: het uitvoerbewakingsharnas (ARB-028/029) is alleen als los onderdeel getest, en een echt kostenmodel (ARB-025) is nog niet geaccepteerd.',
    whatif_no_candidates: 'geen toegelaten kandidaten in dit start-bezit om te schalen',
    whatif_other_asset: 'kandidaten in een ander start-bezit worden niet geschaald voor deze inzet',
    whatif_no_cost: 'geen kostenbeoordeling in de eerste 25 vastgelegde beoordelingen',
    whatif_linear_scaling: 'De cijfers schalen lineair met je inzet (verhouding, geen echte orderboek-simulatie) en gaan uit van USDC als start-bezit.',
    stake_ticket_note: 'Het ticket noemde "2 ETH" als voorbeeldinzet; deze sessie gebruikt USDC omdat dat het start-bezit van de sessie is.',
    top_candidates_source: 'uit de eerste 25 opgeslagen beslissingen',
    checklist_completed: 'geaccepteerd', checklist_implemented_pending_acceptance: 'gebouwd, wacht op acceptatie', checklist_in_progress: 'in uitvoering', checklist_planned: 'gepland',
    not_available_yet: 'nog niet beschikbaar',
  },
  en: {
    mode_OBSERVE: 'watching only (OBSERVE)', mode_PAPER: 'practicing with fake money (PAPER)', mode_REPLAY: 'replaying old data (REPLAY)', mode_LIVE: 'trading for real (LIVE)',
    state_RECOVERING: 'recovering from a restart', state_STOPPED: 'stopped', state_RUNNING: 'running', state_PAUSING: 'pausing', state_PAUSED: 'paused', state_DRAINING: 'finishing in-flight work', state_FAULTED: 'in a fault state',
    worker_no_heartbeat: 'no heartbeat received from the worker yet',
    worker_not_reachable: 'not reachable',
    lag_unknown: 'unknown — no collection received yet', lag_caught_up: 'is caught up (recently updated)', lag_catching_up: 'is catching up (working through a backlog)', lag_stale: 'is stuck — no collection for over 5 minutes', lag_not_collecting: 'stopped, not collecting',
    receipt_none: 'no command has been sent from this workspace yet; the service does not expose a full command history',
    receipt_status_PENDING: 'pending', receipt_status_APPLIED: 'applied', receipt_status_REJECTED: 'rejected', receipt_status_SUPERSEDED: 'superseded by a newer command',
    action_START: 'start', action_PAUSE: 'pause', action_RESUME: 'resume', action_STOP: 'stop', action_DISARM: 'disarm',
    health_red: 'red', health_amber: 'amber', health_green: 'green', health_unknown: 'unknown',
    health_reason_fault: 'the session is in a fault state', health_reason_unreachable: 'the worker cannot be reached', health_reason_stopped: 'the session is not currently collecting (stopped or paused)', health_reason_stale: 'no collection received for over 5 minutes', health_reason_catching_up: 'collection is still catching up', health_reason_ok: 'no fault, no backlog, and at least one collection has been admitted', health_reason_no_data: 'no collection has been admitted yet to judge',
    health_failed_attempts_note: 'failed attempts are counted since session start; provider failures are included in this count, not shown separately',
    findings_evidence_label: 'candidate, not simulated',
    whatif_caveat_not_executed: 'OBSERVE candidates are not executed, not simulated end-to-end, and are not profit.',
    whatif_no_leverage: 'No leverage or flash loan shown: the execution guard harness (ARB-028/029) has only been tested in isolation, and a real cost model (ARB-025) is not yet accepted.',
    whatif_no_candidates: 'no admitted candidates in this start asset to scale',
    whatif_other_asset: 'candidates in a different start asset are not scaled for this stake',
    whatif_no_cost: 'no cost assessment in the first 25 stored assessments',
    whatif_linear_scaling: 'These numbers scale linearly with your stake (a ratio, not a real order-book simulation) and assume USDC as the start asset.',
    stake_ticket_note: 'The ticket wording used "2 ETH" as the example stake; this session uses USDC since that is the session’s starting asset.',
    top_candidates_source: 'from the first 25 stored decisions',
    checklist_completed: 'accepted', checklist_implemented_pending_acceptance: 'built, waiting on acceptance', checklist_in_progress: 'in progress', checklist_planned: 'planned',
    not_available_yet: 'not available yet',
  },
} as const;
export type CopyKey = keyof typeof copy.nl;
export function t(lang: Lang, key: CopyKey): string { return copy[lang][key]; }

// ---- Shared freshness helper (Status + Health blocks) ----------------------
export const STALE_MS = 5 * 60 * 1000; // Health turns red once no collection has been seen for longer than this.
export const CATCHING_UP_MS = 60 * 1000; // Below STALE_MS but above this, the source is treated as still catching up.
export type FreshnessLabel = 'unknown' | 'caught_up' | 'catching_up' | 'stale';
export interface Freshness { ageMs: number | null; label: FreshnessLabel }
export function collectionFreshness(lastCollectionAt: string | null, nowMs: number): Freshness {
  if (!lastCollectionAt) return { ageMs: null, label: 'unknown' };
  // Clamp to 0: clock skew between the client and the fetch timestamp used as `nowMs` must never
  // report a collection as being from the future.
  const ageMs = Math.max(0, nowMs - Date.parse(lastCollectionAt));
  if (ageMs > STALE_MS) return { ageMs, label: 'stale' };
  if (ageMs > CATCHING_UP_MS) return { ageMs, label: 'catching_up' };
  return { ageMs, label: 'caught_up' };
}

// A session only collects while it is actually running (or recovering back into running); a
// deliberately STOPPED/PAUSED/PAUSING/DRAINING session must never be judged for staleness — it is
// not stuck, it was told to stop. See tests: "a deliberately stopped session".
const ACTIVE_STATES: readonly SessionState[] = ['RUNNING', 'RECOVERING'];
function isActiveState(state: SessionState): boolean { return ACTIVE_STATES.includes(state); }

// Worker reachability comes straight from the session's own `health` field, which the service
// derives from the worker's lease (crates/arb-storage/src/lib.rs): DEGRADED means the lease is
// still active (the worker is alive), UNREACHABLE means the lease expired, UNKNOWN means no
// heartbeat has ever been recorded. HEALTHY is never emitted by the service.
export type WorkerReachability = 'alive' | 'not_reachable' | 'unknown';
export function workerReachability(session: Session): WorkerReachability {
  if (session.health === 'DEGRADED') return 'alive';
  if (session.health === 'UNREACHABLE') return 'not_reachable';
  return 'unknown';
}

// ---- Block 1: Status --------------------------------------------------------
export interface StatusSummary {
  hasSession: boolean;
  modeLabel: string; stateLabel: string;
  workerAliveLabel: string; sourceLagLabel: string;
  lastCommandReceiptLabel: string;
}
export function statusSummary(session: Session | null, receipt: CommandReceipt | null, coverage: CollectionCoverage | null, nowMs: number, coverageAtMs: number, lang: Lang): StatusSummary {
  if (!session) return { hasSession: false, modeLabel: t(lang, 'not_available_yet'), stateLabel: t(lang, 'not_available_yet'), workerAliveLabel: t(lang, 'not_available_yet'), sourceLagLabel: t(lang, 'not_available_yet'), lastCommandReceiptLabel: t(lang, 'receipt_none') };

  const reachability = workerReachability(session);
  // Heartbeat age uses the live clock: the parent re-renders this component on its own poll tick
  // (ConnectedApp polls /v1/sessions every 5s), and last_heartbeat_at is refreshed on every poll.
  const heartbeatAgeMs = session.last_heartbeat_at ? Math.max(0, nowMs - Date.parse(session.last_heartbeat_at)) : null;
  const workerAliveLabel = reachability === 'alive' && heartbeatAgeMs !== null ? `${Math.round(heartbeatAgeMs / 1000)}s`
    : reachability === 'not_reachable' ? t(lang, 'worker_not_reachable')
    : t(lang, 'worker_no_heartbeat');

  // Source lag is judged from the whole-session collection coverage window, not a client-side
  // attempt page: collection-coverage is a single un-paginated, always-complete aggregate
  // (crates/arb-storage/src/collection.rs), so this can never be truncated the way a paged
  // attempt window could. Freshness is judged against the coverage's own fetch time, not the live
  // clock: collection-coverage is fetched once per session selection (not re-polled), so ageing it
  // off a live `nowMs` would turn a healthy session "stale" purely because the page sat open (F1).
  const fresh = coverage?.window_end_at ? collectionFreshness(coverage.window_end_at, coverageAtMs) : { ageMs: null, label: 'unknown' as FreshnessLabel };
  const sourceLagLabel = !isActiveState(session.observed_state) ? t(lang, 'lag_not_collecting')
    : fresh.label === 'stale' ? t(lang, 'lag_stale')
    : fresh.label === 'catching_up' ? t(lang, 'lag_catching_up')
    : fresh.label === 'caught_up' ? t(lang, 'lag_caught_up')
    : t(lang, 'lag_unknown');

  const lastCommandReceiptLabel = receipt ? `${t(lang, `receipt_status_${receipt.status}` as CopyKey)} (${t(lang, `action_${receipt.action}` as CopyKey)})` : t(lang, 'receipt_none');
  return { hasSession: true, modeLabel: t(lang, `mode_${session.mode}` as CopyKey), stateLabel: t(lang, `state_${session.observed_state}` as CopyKey), workerAliveLabel, sourceLagLabel, lastCommandReceiptLabel };
}

// ---- Block 2: Health ---------------------------------------------------------
export type HealthLevel = 'red' | 'amber' | 'green' | 'unknown';
export interface HealthSummary {
  level: HealthLevel; reasonLabel: string;
  collections: string; admittedLabel: string; failedAttempts: string;
}
export function healthSummary(coverage: CollectionCoverage | null, session: Session | null, coverageAtMs: number, lang: Lang): HealthSummary {
  // Whole-session totals straight from collection-coverage (one O(1) request, always complete —
  // never the truncated 24h attempt-page walk the previous version depended on). `research_attempts`
  // (not `attempts_started`) is the "collection attempts" count and the admitted-share denominator:
  // `attempts_started` also counts READINESS probes recorded while STOPPED/PAUSED (~1 per 46s), which
  // would otherwise pad both numbers with attempts that never tried to collect anything (F3).
  const researchAttempts = coverage ? BigInt(coverage.research_attempts) : 0n;
  const admitted = coverage ? BigInt(coverage.decisions_recorded) : 0n;
  // Every non-admitted *terminal* outcome counts as a failed attempt. SUPPRESSED (a fenced
  // generation — main.rs's `finish_collection(..., Suppressed, GenerationFenced)`) is not a
  // failure and is intentionally excluded: a real source halt surfaces as the session FAULT, not
  // as a suppressed collection attempt.
  const failedAttempts = coverage ? BigInt(coverage.acquisition_failed) + BigInt(coverage.evaluation_failed) + BigInt(coverage.deadline_exceeded) + BigInt(coverage.worker_cancelled) : 0n;
  const admittedLabel = researchAttempts > 0n ? `${admitted}/${researchAttempts}` : t(lang, 'not_available_yet');

  let level: HealthLevel; let reasonKey: CopyKey;
  // A known session fault is authoritative regardless of what collection coverage shows.
  if (session?.observed_state === 'FAULTED') { level = 'red'; reasonKey = 'health_reason_fault'; }
  else if (session && workerReachability(session) === 'not_reachable' && isActiveState(session.observed_state)) { level = 'red'; reasonKey = 'health_reason_unreachable'; }
  else if (!session || !isActiveState(session.observed_state)) { level = 'unknown'; reasonKey = 'health_reason_stopped'; }
  else {
    // Freshness is judged against the coverage's own fetch time, not a live clock (F1): see the
    // matching note in statusSummary.
    const fresh = coverage?.window_end_at ? collectionFreshness(coverage.window_end_at, coverageAtMs) : { ageMs: null, label: 'unknown' as FreshnessLabel };
    // Failed attempts are informational only (surfaced as their own counter below), not a ladder
    // gate: the worker records ACQUISITION_FAILED/ACQUISITION_UNAVAILABLE on every catch-up step
    // short of its anchor and WORKER_CANCELLED on every redeploy, so failed attempts accumulate on
    // every healthy long-running session — gating on them here made green unreachable (F2).
    if (researchAttempts === 0n) { level = 'unknown'; reasonKey = 'health_reason_no_data'; }
    else if (fresh.label === 'stale') { level = 'red'; reasonKey = 'health_reason_stale'; }
    else if (fresh.label === 'catching_up') { level = 'amber'; reasonKey = 'health_reason_catching_up'; }
    else if (admitted > 0n) { level = 'green'; reasonKey = 'health_reason_ok'; }
    else { level = 'unknown'; reasonKey = 'health_reason_no_data'; }
  }

  // While coverage hasn't loaded yet (or failed), report the counters as not-yet-available rather
  // than a fabricated zero, which would read as "zero attempts, zero failures" (F5).
  return {
    level, reasonLabel: t(lang, reasonKey),
    collections: coverage ? researchAttempts.toString() : t(lang, 'not_available_yet'),
    admittedLabel,
    failedAttempts: coverage ? failedAttempts.toString() : t(lang, 'not_available_yet'),
  };
}

// ---- Block 3: Findings --------------------------------------------------------
function compareBigintDesc(a: bigint, b: bigint): number { return a > b ? -1 : a < b ? 1 : 0; }
export interface TopCandidate { observationId: string; assetIn: string; assetOut: string; grossDeltaMinor: string; evidenceLabel: string; datasetOrigin: Origin }
export interface FindingsSummary {
  hasData: boolean;
  totals: { rawObservations: string; quotedCandidates: string; rejected: string; noRoute: string; dataUnavailable: string } | null;
  batches: { windowStartMs: number; quotedCandidates: string }[];
  topCandidates: TopCandidate[];
  firstBatchAtMs: number | null; lastBatchAtMs: number | null;
}
export function findingsSummary(coverage: Coverage | null, groups: readonly DecisionGroup[], decisions: readonly StoredDecision[], lang: Lang): FindingsSummary {
  const quoted = decisions.filter((d): d is StoredDecision & { trace: { result: { status: 'QUOTED'; quoted_output_minor: string; gross_delta_minor: string; included_pool_fees: string[] } } } => d.trace.result.status === 'QUOTED');
  // "Top candidates" means a positive modeled edge; a zero-or-negative gross delta is not a
  // candidate worth surfacing here (it would never be picked over doing nothing).
  const positiveEdge = quoted.filter(d => BigInt(d.trace.result.gross_delta_minor) > 0n);
  const topCandidates: TopCandidate[] = [...positiveEdge]
    .sort((a, b) => compareBigintDesc(BigInt(a.trace.result.gross_delta_minor), BigInt(b.trace.result.gross_delta_minor)))
    .slice(0, 5)
    .map(d => ({ observationId: d.trace.observation_id, assetIn: d.trace.route[0]?.asset_in ?? '', assetOut: d.trace.route[d.trace.route.length - 1]?.asset_out ?? '', grossDeltaMinor: d.trace.result.gross_delta_minor, evidenceLabel: t(lang, 'findings_evidence_label'), datasetOrigin: d.trace.dataset_origin }));
  return {
    hasData: coverage !== null || groups.length > 0 || decisions.length > 0,
    totals: coverage ? { rawObservations: coverage.raw_observations, quotedCandidates: coverage.quoted_candidates, rejected: coverage.rejected, noRoute: coverage.no_route, dataUnavailable: coverage.data_unavailable } : null,
    batches: groups.map(g => ({ windowStartMs: g.window_start_ms, quotedCandidates: g.quoted_candidates })),
    topCandidates,
    firstBatchAtMs: coverage?.coverage_window_start_ms ?? null,
    lastBatchAtMs: coverage?.coverage_window_end_ms ?? null,
  };
}

// ---- Block 4: What if ---------------------------------------------------------
// Both networks' research sessions are configured with USDC as the starting/stake asset
// (config/research.example.toml: default_starting_asset_symbol = "USDC", a global setting with
// no per-network override). USDC has 6 decimals on both Base and Solana — a verified property of
// each network's own USDC mint/token contract, not one network's decimals assumed onto the other.
export const STARTING_ASSET_DECIMALS = 6;
// The exact configured starting-asset AssetId per network (scripts/recorded_base_slice.py:27 for
// Base; the Solana USDC mint is the same well-known address used across this repo's Solana
// scripts, e.g. scripts/inspect_pool_candidates.py's SOL_USDC). AssetId always serializes as
// `network:address` (crates/arb-domain/src/identity.rs), so this is compared for exact equality —
// matching only the network prefix would also match a candidate quoting a different asset
// (e.g. WETH) on the same network, which is not comparable to a USDC stake.
export const STARTING_ASSET_ID: Record<Network, string> = {
  'base-mainnet': 'base-mainnet:0x833589fcd6edb6e08f4c7c32d4f71b54bda02913',
  'solana-mainnet': 'solana-mainnet:EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
};
export function parseStakeToMinor(stakeWhole: string, decimals: number): bigint | null {
  if (!/^[0-9]+(\.[0-9]+)?$/.test(stakeWhole.trim())) return null;
  const [whole, fraction = ''] = stakeWhole.trim().split('.');
  if (fraction.length > decimals) return null; // refuse silent precision loss rather than round.
  const paddedFraction = fraction.padEnd(decimals, '0');
  return BigInt(whole) * 10n ** BigInt(decimals) + BigInt(paddedFraction || '0');
}
export interface WhatIfCandidate { observationId: string; modeledGrossEdgeMinor: string; recordedCostLabel: string; datasetOrigin: Origin }
export interface WhatIfSummary {
  stakeValid: boolean; stakeMinor: string | null;
  candidates: WhatIfCandidate[]; skippedOtherAssetCount: number;
  caveats: string[]; noLeverageNote: string;
}
export function whatIfSummary(stakeWhole: string, network: Network, decisions: readonly StoredDecision[], costAssessments: readonly StoredCostAssessment[], lang: Lang): WhatIfSummary {
  const stakeMinor = parseStakeToMinor(stakeWhole, STARTING_ASSET_DECIMALS);
  const caveats = [t(lang, 'whatif_caveat_not_executed'), t(lang, 'whatif_linear_scaling')];
  const noLeverageNote = t(lang, 'whatif_no_leverage');
  if (stakeMinor === null) return { stakeValid: false, stakeMinor: null, candidates: [], skippedOtherAssetCount: 0, caveats, noLeverageNote };

  const quoted = decisions.filter((d): d is StoredDecision & { trace: { amount_in_minor: string; result: { status: 'QUOTED'; gross_delta_minor: string } } } =>
    d.trace.result.status === 'QUOTED' && d.trace.amount_in_minor !== null && BigInt(d.trace.amount_in_minor) > 0n);
  const startingAssetId = STARTING_ASSET_ID[network];
  const sameAsset = quoted.filter(d => (d.trace.route[0]?.asset_in ?? '') === startingAssetId);
  const skippedOtherAssetCount = quoted.length - sameAsset.length;

  const candidates: WhatIfCandidate[] = sameAsset.slice(0, 5).map(d => {
    const modeledGrossEdgeMinor = (BigInt(d.trace.result.gross_delta_minor) * stakeMinor / BigInt(d.trace.amount_in_minor)).toString();
    const match = costAssessments.find(c => c.assessment.binding.observation_id === d.trace.observation_id);
    const recordedCostLabel = match ? (match.assessment.report.fully_allocated_net ?? match.assessment.report.transaction_net ?? t(lang, 'whatif_no_cost')) : t(lang, 'whatif_no_cost');
    return { observationId: d.trace.observation_id, modeledGrossEdgeMinor, recordedCostLabel, datasetOrigin: d.trace.dataset_origin };
  });
  return { stakeValid: true, stakeMinor: stakeMinor.toString(), candidates, skippedOtherAssetCount, caveats, noLeverageNote };
}

// ---- Block 5: Road to PAPER/live ------------------------------------------------
export interface ProgressTicket { id: string; state: string; remaining_acceptance?: string; issue_url?: string }
export interface ProgressFile { tickets: ProgressTicket[] }
// Validates the shape this module actually reads out of planning/implementation-progress.json.
// A drift in that file's schema fails here (at build/test time, see tests/ownerOverview.test.ts)
// instead of silently passing through an unchecked `as ProgressFile` cast.
// A plain (possibly empty) string check: unlike research.ts's text(), an empty string is a valid,
// meaningful value here (a completed ticket's "remaining_acceptance" is "" — nothing left).
function optionalPlainString(value: unknown): value is string | undefined { return value === undefined || typeof value === 'string'; }
export function parseProgressFile(value: unknown): ProgressFile {
  const root = record(value);
  requireValue(Array.isArray(root.tickets), 'progress file: "tickets" must be an array.');
  const tickets = (root.tickets as unknown[]).map((raw): ProgressTicket => {
    const r = record(raw);
    requireValue(text(r.id), 'progress ticket: "id" must be a non-empty string.');
    requireValue(text(r.state), 'progress ticket: "state" must be a non-empty string.');
    requireValue(optionalPlainString(r.remaining_acceptance), 'progress ticket: "remaining_acceptance" must be a string when present.');
    requireValue(optionalPlainString(r.issue_url), 'progress ticket: "issue_url" must be a string when present.');
    return { id: r.id as string, state: r.state as string, remaining_acceptance: r.remaining_acceptance as string | undefined, issue_url: r.issue_url as string | undefined };
  });
  return { tickets };
}
export const ROAD_TO_LIVE_TICKET_IDS = ['ARB-035', 'ARB-039', 'ARB-041', 'ARB-042', 'ARB-043', 'ARB-044'] as const;
export interface ChecklistItem { id: string; stateLabel: string; remainingAcceptance: string; issueUrl: string | null }
export function roadToLiveChecklist(progress: ProgressFile, lang: Lang): ChecklistItem[] {
  return ROAD_TO_LIVE_TICKET_IDS.map(id => {
    const ticket = progress.tickets.find(item => item.id === id);
    if (!ticket) return { id, stateLabel: t(lang, 'not_available_yet'), remainingAcceptance: t(lang, 'not_available_yet'), issueUrl: null };
    const stateKey = `checklist_${ticket.state}` as CopyKey;
    return { id, stateLabel: (copy[lang] as Record<string, string>)[stateKey] ?? ticket.state, remainingAcceptance: ticket.remaining_acceptance ?? t(lang, 'not_available_yet'), issueUrl: ticket.issue_url ?? null };
  });
}
