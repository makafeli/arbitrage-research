// Pure, framework-free derivation logic for the plain-language Owner Overview page (issue #182).
// No React here and no JSON import here: keeps this file runnable by `node --test` without
// Node's ESM JSON-import-attribute rules, and keeps the progress-file shape a caller concern.
import type { Session, CommandReceipt, Network } from '../api/client.ts';
import type { CollectionAttempt } from '../api/collection.ts';
import type { Coverage, DecisionGroup, StoredDecision } from '../api/research.ts';
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
    worker_unknown: 'geen verzameling ontvangen sinds de sessie begon',
    worker_indeterminate: 'kan niet worden vastgesteld — het venster van pogingen is afgekapt',
    lag_unknown: 'onbekend — nog geen verzameling ontvangen', lag_caught_up: 'is bij (recent bijgewerkt)', lag_catching_up: 'loopt in (haalt achterstand in)', lag_stale: 'loopt vast — meer dan 5 minuten geen verzameling', lag_indeterminate: 'kan niet worden vastgesteld — het venster van pogingen is afgekapt',
    receipt_none: 'nog geen commando vanuit dit venster verstuurd; de dienst biedt geen volledige commandogeschiedenis',
    receipt_status_PENDING: 'in behandeling', receipt_status_APPLIED: 'toegepast', receipt_status_REJECTED: 'geweigerd', receipt_status_SUPERSEDED: 'ingehaald door een nieuwer commando',
    health_red: 'rood', health_amber: 'oranje', health_green: 'groen', health_unknown: 'onbekend',
    health_reason_fault: 'de sessie staat in storing', health_reason_stale: 'meer dan 5 minuten geen verzameling ontvangen', health_reason_catching_up: 'de verzameling loopt nog achterstand in', health_reason_provider_failure: 'een provider faalde in dit venster', health_reason_ok: 'geen storing, geen achterstand en geen providerfout gezien', health_reason_no_data: 'nog geen verzameling ontvangen om te beoordelen', health_reason_truncated: 'het venster van pogingen is afgekapt, dus de status kan niet worden beoordeeld',
    findings_evidence_label: 'kandidaat — niet gesimuleerd',
    whatif_caveat_not_executed: 'OBSERVE-kandidaten worden niet uitgevoerd, niet volledig gesimuleerd en zijn geen winst.',
    whatif_no_leverage: 'Geen hefboom of flashlening getoond: het uitvoerbewakingsharnas (ARB-028/029) is alleen als los onderdeel getest, en een echt kostenmodel (ARB-025) is nog niet geaccepteerd.',
    whatif_no_candidates: 'geen toegelaten kandidaten in dit start-bezit om te schalen',
    whatif_other_asset: 'kandidaten in een ander start-bezit worden niet geschaald voor deze inzet',
    whatif_no_cost: 'geen kostenbeoordeling vastgelegd voor deze kandidaat',
    whatif_linear_scaling: 'De cijfers schalen lineair met je inzet (verhouding, geen echte orderboek-simulatie) en gaan uit van USDC als start-bezit.',
    checklist_completed: 'geaccepteerd', checklist_implemented_pending_acceptance: 'gebouwd, wacht op acceptatie', checklist_in_progress: 'in uitvoering', checklist_planned: 'gepland',
    not_available_yet: 'nog niet beschikbaar',
  },
  en: {
    mode_OBSERVE: 'watching only (OBSERVE)', mode_PAPER: 'practicing with fake money (PAPER)', mode_REPLAY: 'replaying old data (REPLAY)', mode_LIVE: 'trading for real (LIVE)',
    state_RECOVERING: 'recovering from a restart', state_STOPPED: 'stopped', state_RUNNING: 'running', state_PAUSING: 'pausing', state_PAUSED: 'paused', state_DRAINING: 'finishing in-flight work', state_FAULTED: 'in a fault state',
    worker_unknown: 'no collection received since this session started',
    worker_indeterminate: 'cannot be determined — the attempt window was truncated',
    lag_unknown: 'unknown — no collection received yet', lag_caught_up: 'is caught up (recently updated)', lag_catching_up: 'is catching up (working through a backlog)', lag_stale: 'is stuck — no collection for over 5 minutes', lag_indeterminate: 'cannot be determined — the attempt window was truncated',
    receipt_none: 'no command has been sent from this workspace yet; the service does not expose a full command history',
    receipt_status_PENDING: 'pending', receipt_status_APPLIED: 'applied', receipt_status_REJECTED: 'rejected', receipt_status_SUPERSEDED: 'superseded by a newer command',
    health_red: 'red', health_amber: 'amber', health_green: 'green', health_unknown: 'unknown',
    health_reason_fault: 'the session is in a fault state', health_reason_stale: 'no collection received for over 5 minutes', health_reason_catching_up: 'collection is still catching up', health_reason_provider_failure: 'a provider failed in this window', health_reason_ok: 'no fault, no backlog and no provider failure seen', health_reason_no_data: 'no collection received yet to judge', health_reason_truncated: 'the attempt window was truncated, so status cannot be judged',
    findings_evidence_label: 'candidate, not simulated',
    whatif_caveat_not_executed: 'OBSERVE candidates are not executed, not simulated end-to-end, and are not profit.',
    whatif_no_leverage: 'No leverage or flash loan shown: the execution guard harness (ARB-028/029) has only been tested in isolation, and a real cost model (ARB-025) is not yet accepted.',
    whatif_no_candidates: 'no admitted candidates in this start asset to scale',
    whatif_other_asset: 'candidates in a different start asset are not scaled for this stake',
    whatif_no_cost: 'no cost assessment recorded for this candidate yet',
    whatif_linear_scaling: 'These numbers scale linearly with your stake (a ratio, not a real order-book simulation) and assume USDC as the start asset.',
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
function latestAttemptAt(attempts: readonly CollectionAttempt[]): string | null {
  // ponytail: assumes the API's first page is newest-first, matching how every other
  // paginated research view in this codebase treats page one. Not a documented guarantee.
  const withTime = attempts.filter(a => a.finished_at || a.started_at);
  if (withTime.length === 0) return null;
  return withTime.reduce((latest, a) => {
    const at = a.finished_at ?? a.started_at;
    return Date.parse(at) > Date.parse(latest) ? at : latest;
  }, withTime[0].finished_at ?? withTime[0].started_at);
}

// ---- Block 1: Status --------------------------------------------------------
export interface StatusSummary {
  hasSession: boolean;
  modeLabel: string; stateLabel: string;
  workerAliveLabel: string; sourceLagLabel: string;
  lastCommandReceiptLabel: string;
}
export function statusSummary(session: Session | null, receipt: CommandReceipt | null, attempts: readonly CollectionAttempt[], nowMs: number, truncated: boolean, lang: Lang): StatusSummary {
  if (!session) return { hasSession: false, modeLabel: t(lang, 'not_available_yet'), stateLabel: t(lang, 'not_available_yet'), workerAliveLabel: t(lang, 'not_available_yet'), sourceLagLabel: t(lang, 'not_available_yet'), lastCommandReceiptLabel: t(lang, 'receipt_none') };
  // A truncated window only holds the oldest slice of the last 24h (see fetchWindow below), so its
  // latest attempt is not necessarily recent — deriving freshness from it would be able to report a
  // current source as stale. Report freshness as indeterminate instead of guessing from a partial window.
  const fresh = truncated ? { ageMs: null, label: 'unknown' as FreshnessLabel } : collectionFreshness(latestAttemptAt(attempts), nowMs);
  const workerAliveLabel = truncated ? t(lang, 'worker_indeterminate') : fresh.label === 'unknown' ? t(lang, 'worker_unknown') : `${Math.round((fresh.ageMs as number) / 1000)}s`;
  // Status must describe the same freshness Health scores: stale is its own word, never folded into "catching up".
  const sourceLagLabel = truncated ? t(lang, 'lag_indeterminate') : fresh.label === 'stale' ? t(lang, 'lag_stale') : fresh.label === 'catching_up' ? t(lang, 'lag_catching_up') : fresh.label === 'caught_up' ? t(lang, 'lag_caught_up') : t(lang, 'lag_unknown');
  const lastCommandReceiptLabel = receipt ? `${t(lang, `receipt_status_${receipt.status}` as CopyKey)} (${receipt.action})` : t(lang, 'receipt_none');
  return { hasSession: true, modeLabel: t(lang, `mode_${session.mode}` as CopyKey), stateLabel: t(lang, `state_${session.observed_state}` as CopyKey), workerAliveLabel, sourceLagLabel, lastCommandReceiptLabel };
}

// ---- Block 2: Health ---------------------------------------------------------
export type HealthLevel = 'red' | 'amber' | 'green' | 'unknown';
export interface HealthSummary {
  level: HealthLevel; reasonLabel: string;
  collections: number; admittedLabel: string; halts: number; faults: number; providerFailures: number;
}
export const DAY_MS = 24 * 60 * 60 * 1000;
export function healthSummary(attempts: readonly CollectionAttempt[], session: Session | null, nowMs: number, truncated: boolean, lang: Lang): HealthSummary {
  // Window: last 24h. The caller is expected to have already fetched attempts starting near this
  // boundary (see fetchWindow + uuidV7FloorForTimestamp below); this filter is a cheap, redundant
  // guard against the exact boundary rather than the only thing enforcing the window.
  const windowStart = nowMs - DAY_MS;
  const inWindow = attempts.filter(a => Date.parse(a.started_at) >= windowStart);
  const collections = inWindow.length;
  const admitted = inWindow.filter(a => a.outcome === 'DECISIONS_RECORDED').length;
  const halts = inWindow.filter(a => a.outcome === 'SUPPRESSED').length;
  const providerFailures = inWindow.filter(a => a.reason === 'PROVIDER_UNAVAILABLE').length;
  const faults = session?.observed_state === 'FAULTED' ? 1 : 0;
  // See statusSummary's comment: a truncated window's latest attempt is not necessarily recent, so
  // it must not be trusted for a stale/caught-up judgment either.
  const fresh = truncated ? { ageMs: null, label: 'unknown' as FreshnessLabel } : collectionFreshness(latestAttemptAt(attempts), nowMs);
  const admittedLabel = collections > 0 ? `${admitted}/${collections}` : t(lang, 'not_available_yet');

  let level: HealthLevel; let reasonKey: CopyKey;
  // A known session fault is authoritative regardless of what the (possibly incomplete) attempt
  // window shows, so it is judged before both the truncation and no-data cases.
  if (faults > 0) { level = 'red'; reasonKey = 'health_reason_fault'; }
  else if (truncated) { level = 'unknown'; reasonKey = 'health_reason_truncated'; }
  else if (fresh.label === 'unknown' && collections === 0) { level = 'unknown'; reasonKey = 'health_reason_no_data'; }
  else if (fresh.label === 'stale') { level = 'red'; reasonKey = 'health_reason_stale'; }
  else if (fresh.label === 'catching_up') { level = 'amber'; reasonKey = 'health_reason_catching_up'; }
  else if (providerFailures > 0) { level = 'amber'; reasonKey = 'health_reason_provider_failure'; }
  else { level = 'green'; reasonKey = 'health_reason_ok'; }

  return { level, reasonLabel: t(lang, reasonKey), collections, admittedLabel, halts, faults, providerFailures };
}

// ---- Attempts windowing: Status and Health need the last 24h, not the oldest page --------------
// collection-attempts and decisions are both listed oldest-first (`WHERE attempt_id/trace_id > cursor
// ORDER BY attempt_id/trace_id`), and both ids are UUIDv7 (time-ordered). Fetching page one with no
// cursor therefore returns the first records the session ever produced, not its current state. To
// get the last 24h instead, start from a synthetic UUIDv7 "floor" for `now - 24h` and page forward.
export function uuidV7FloorForTimestamp(unixMs: number): string {
  const hex = Math.max(0, Math.trunc(unixMs)).toString(16).padStart(12, '0').slice(-12);
  // version=7, variant=10, all other bits zero: the smallest possible UUIDv7 with this timestamp.
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-7000-8000-000000000000`;
}
export const MAX_WINDOW_PAGES = 4; // ponytail: bounds the forward walk; raise if a real session outpaces 4 pages/24h.
export interface WindowPage<T> { items: T[]; next_cursor: string | null }
export interface WindowResult<T> { items: T[]; truncated: boolean }
export async function fetchWindow<T>(loadPage: (cursor: string | undefined) => Promise<WindowPage<T>>, floorCursor: string, maxPages: number): Promise<WindowResult<T>> {
  const items: T[] = [];
  let cursor: string | undefined = floorCursor;
  for (let page = 0; page < maxPages; page++) {
    const result = await loadPage(cursor);
    items.push(...result.items);
    if (!result.next_cursor) return { items, truncated: false };
    cursor = result.next_cursor;
  }
  return { items, truncated: true };
}

// ---- Block 3: Findings --------------------------------------------------------
export interface TopCandidate { observationId: string; assetIn: string; assetOut: string; grossDeltaMinor: string; evidenceLabel: string }
export interface FindingsSummary {
  hasData: boolean;
  totals: { rawObservations: string; quotedCandidates: string; rejected: string; noRoute: string; dataUnavailable: string } | null;
  batches: { windowStartMs: number; quotedCandidates: string }[];
  topCandidates: TopCandidate[];
}
export function findingsSummary(coverage: Coverage | null, groups: readonly DecisionGroup[], decisions: readonly StoredDecision[], lang: Lang): FindingsSummary {
  const quoted = decisions.filter((d): d is StoredDecision & { trace: { result: { status: 'QUOTED'; quoted_output_minor: string; gross_delta_minor: string; included_pool_fees: string[] } } } => d.trace.result.status === 'QUOTED');
  const topCandidates: TopCandidate[] = [...quoted]
    .sort((a, b) => (BigInt(b.trace.result.gross_delta_minor) > BigInt(a.trace.result.gross_delta_minor) ? 1 : -1))
    .slice(0, 5)
    .map(d => ({ observationId: d.trace.observation_id, assetIn: d.trace.route[0]?.asset_in ?? '', assetOut: d.trace.route[d.trace.route.length - 1]?.asset_out ?? '', grossDeltaMinor: d.trace.result.gross_delta_minor, evidenceLabel: t(lang, 'findings_evidence_label') }));
  return {
    hasData: coverage !== null || groups.length > 0 || decisions.length > 0,
    totals: coverage ? { rawObservations: coverage.raw_observations, quotedCandidates: coverage.quoted_candidates, rejected: coverage.rejected, noRoute: coverage.no_route, dataUnavailable: coverage.data_unavailable } : null,
    batches: groups.map(g => ({ windowStartMs: g.window_start_ms, quotedCandidates: g.quoted_candidates })),
    topCandidates,
  };
}

// ---- Block 4: What if ---------------------------------------------------------
// Both networks' research sessions are configured with USDC as the starting/stake asset
// (config/research.example.toml: [research] default_starting_asset_symbol = "USDC", a global
// setting with no per-network override) — not each network's native coin. USDC has 6 decimals
// on both Base and Solana.
export const STARTING_ASSET_DECIMALS = 6;
export function parseStakeToMinor(stakeWhole: string, decimals: number): bigint | null {
  if (!/^[0-9]+(\.[0-9]+)?$/.test(stakeWhole.trim())) return null;
  const [whole, fraction = ''] = stakeWhole.trim().split('.');
  if (fraction.length > decimals) return null; // refuse silent precision loss rather than round.
  const paddedFraction = fraction.padEnd(decimals, '0');
  return BigInt(whole) * 10n ** BigInt(decimals) + BigInt(paddedFraction || '0');
}
export interface WhatIfCandidate { observationId: string; modeledGrossEdgeMinor: string; recordedCostLabel: string }
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
  // AssetId always serializes as `network:address` (crates/arb-domain/src/identity.rs) — never a
  // bare network name — so the same-asset check must match the network prefix, not full equality.
  const sameAsset = quoted.filter(d => (d.trace.route[0]?.asset_in ?? '').startsWith(`${network}:`));
  const skippedOtherAssetCount = quoted.length - sameAsset.length;

  const candidates: WhatIfCandidate[] = sameAsset.slice(0, 5).map(d => {
    const modeledGrossEdgeMinor = (BigInt(d.trace.result.gross_delta_minor) * stakeMinor / BigInt(d.trace.amount_in_minor)).toString();
    const match = costAssessments.find(c => c.assessment.binding.observation_id === d.trace.observation_id);
    const recordedCostLabel = match ? (match.assessment.report.fully_allocated_net ?? match.assessment.report.transaction_net ?? t(lang, 'whatif_no_cost')) : t(lang, 'whatif_no_cost');
    return { observationId: d.trace.observation_id, modeledGrossEdgeMinor, recordedCostLabel };
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
