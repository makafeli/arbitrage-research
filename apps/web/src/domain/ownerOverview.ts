// Pure, framework-free derivation logic for the plain-language Owner Overview page (issue #182).
// No React here and no JSON import here: keeps this file runnable by `node --test` without
// Node's ESM JSON-import-attribute rules, and keeps the progress-file shape a caller concern.
import type { Session, CommandReceipt, Network } from '../api/client.ts';
import type { CollectionAttempt } from '../api/collection.ts';
import type { Coverage, DecisionGroup, StoredDecision } from '../api/research.ts';
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
    lag_unknown: 'onbekend — nog geen verzameling ontvangen', lag_caught_up: 'is bij (recent bijgewerkt)', lag_catching_up: 'loopt in (haalt achterstand in)', lag_stale: 'loopt vast — meer dan 5 minuten geen verzameling',
    receipt_none: 'nog geen commando vanuit dit venster verstuurd; de dienst biedt geen volledige commandogeschiedenis',
    receipt_status_PENDING: 'in behandeling', receipt_status_APPLIED: 'toegepast', receipt_status_REJECTED: 'geweigerd', receipt_status_SUPERSEDED: 'ingehaald door een nieuwer commando',
    health_red: 'rood', health_amber: 'oranje', health_green: 'groen', health_unknown: 'onbekend',
    health_reason_fault: 'de sessie staat in storing', health_reason_stale: 'meer dan 5 minuten geen verzameling ontvangen', health_reason_catching_up: 'de verzameling loopt nog achterstand in', health_reason_provider_failure: 'een provider faalde in dit venster', health_reason_ok: 'geen storing, geen achterstand en geen providerfout gezien', health_reason_no_data: 'nog geen verzameling ontvangen om te beoordelen',
    findings_evidence_label: 'kandidaat — niet gesimuleerd',
    whatif_caveat_not_executed: 'OBSERVE-kandidaten worden niet uitgevoerd, niet volledig gesimuleerd en zijn geen winst.',
    whatif_no_leverage: 'Geen hefboom of flashlening getoond: het uitvoerbewakingsharnas (ARB-028/029) is alleen als los onderdeel getest, en een echt kostenmodel (ARB-041) is nog niet geaccepteerd.',
    whatif_no_candidates: 'geen toegelaten kandidaten in dit start-bezit om te schalen',
    whatif_other_asset: 'kandidaten in een ander start-bezit worden niet geschaald voor deze inzet',
    whatif_no_cost: 'geen kostenbeoordeling vastgelegd voor deze kandidaat',
    checklist_completed: 'geaccepteerd', checklist_implemented_pending_acceptance: 'gebouwd, wacht op acceptatie', checklist_in_progress: 'in uitvoering', checklist_planned: 'gepland',
    not_available_yet: 'nog niet beschikbaar',
  },
  en: {
    mode_OBSERVE: 'watching only (OBSERVE)', mode_PAPER: 'practicing with fake money (PAPER)', mode_REPLAY: 'replaying old data (REPLAY)', mode_LIVE: 'trading for real (LIVE)',
    state_RECOVERING: 'recovering from a restart', state_STOPPED: 'stopped', state_RUNNING: 'running', state_PAUSING: 'pausing', state_PAUSED: 'paused', state_DRAINING: 'finishing in-flight work', state_FAULTED: 'in a fault state',
    worker_unknown: 'no collection received since this session started',
    lag_unknown: 'unknown — no collection received yet', lag_caught_up: 'is caught up (recently updated)', lag_catching_up: 'is catching up (working through a backlog)', lag_stale: 'is stuck — no collection for over 5 minutes',
    receipt_none: 'no command has been sent from this workspace yet; the service does not expose a full command history',
    receipt_status_PENDING: 'pending', receipt_status_APPLIED: 'applied', receipt_status_REJECTED: 'rejected', receipt_status_SUPERSEDED: 'superseded by a newer command',
    health_red: 'red', health_amber: 'amber', health_green: 'green', health_unknown: 'unknown',
    health_reason_fault: 'the session is in a fault state', health_reason_stale: 'no collection received for over 5 minutes', health_reason_catching_up: 'collection is still catching up', health_reason_provider_failure: 'a provider failed in this window', health_reason_ok: 'no fault, no backlog and no provider failure seen', health_reason_no_data: 'no collection received yet to judge',
    findings_evidence_label: 'candidate, not simulated',
    whatif_caveat_not_executed: 'OBSERVE candidates are not executed, not simulated end-to-end, and are not profit.',
    whatif_no_leverage: 'No leverage or flash loan shown: the execution guard harness (ARB-028/029) has only been tested in isolation, and a real cost model (ARB-041) is not yet accepted.',
    whatif_no_candidates: 'no admitted candidates in this start asset to scale',
    whatif_other_asset: 'candidates in a different start asset are not scaled for this stake',
    whatif_no_cost: 'no cost assessment recorded for this candidate yet',
    checklist_completed: 'accepted', checklist_implemented_pending_acceptance: 'built, waiting on acceptance', checklist_in_progress: 'in progress', checklist_planned: 'planned',
    not_available_yet: 'not available yet',
  },
} as const;
export type CopyKey = keyof typeof copy.nl;
export function t(lang: Lang, key: CopyKey): string { return copy[lang][key]; }

// ---- Shared freshness helper (Status + Health blocks) ----------------------
export const STALE_MS = 5 * 60 * 1000; // mandated by the ticket: red at >5 minutes with no collection.
export const CATCHING_UP_MS = 60 * 1000; // interpretive: no exact number is mandated for "catching up".
export type FreshnessLabel = 'unknown' | 'caught_up' | 'catching_up' | 'stale';
export interface Freshness { ageMs: number | null; label: FreshnessLabel }
export function collectionFreshness(lastCollectionAt: string | null, nowMs: number): Freshness {
  if (!lastCollectionAt) return { ageMs: null, label: 'unknown' };
  const ageMs = nowMs - Date.parse(lastCollectionAt);
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
export function statusSummary(session: Session | null, receipt: CommandReceipt | null, attempts: readonly CollectionAttempt[], nowMs: number, lang: Lang): StatusSummary {
  if (!session) return { hasSession: false, modeLabel: t(lang, 'not_available_yet'), stateLabel: t(lang, 'not_available_yet'), workerAliveLabel: t(lang, 'not_available_yet'), sourceLagLabel: t(lang, 'not_available_yet'), lastCommandReceiptLabel: t(lang, 'receipt_none') };
  const fresh = collectionFreshness(latestAttemptAt(attempts), nowMs);
  const workerAliveLabel = fresh.label === 'unknown' ? t(lang, 'worker_unknown') : `${Math.round((fresh.ageMs as number) / 1000)}s`;
  const sourceLagLabel = fresh.label === 'stale' || fresh.label === 'catching_up' ? t(lang, 'lag_catching_up') : fresh.label === 'caught_up' ? t(lang, 'lag_caught_up') : t(lang, 'lag_unknown');
  const lastCommandReceiptLabel = receipt ? `${t(lang, `receipt_status_${receipt.status}` as CopyKey)} (${receipt.action})` : t(lang, 'receipt_none');
  return { hasSession: true, modeLabel: t(lang, `mode_${session.mode}` as CopyKey), stateLabel: t(lang, `state_${session.observed_state}` as CopyKey), workerAliveLabel, sourceLagLabel, lastCommandReceiptLabel };
}

// ---- Block 2: Health ---------------------------------------------------------
export type HealthLevel = 'red' | 'amber' | 'green' | 'unknown';
export interface HealthSummary {
  level: HealthLevel; reasonLabel: string;
  collections: number; admittedLabel: string; halts: number; faults: number; providerFailures: number;
}
const DAY_MS = 24 * 60 * 60 * 1000;
export function healthSummary(attempts: readonly CollectionAttempt[], session: Session | null, nowMs: number, lang: Lang): HealthSummary {
  // ticket-mandated window: last 24h, drawn from the loaded collection-attempts page(s), not from
  // an endpoint parameter (collection-coverage exposes no such time filter).
  const windowStart = nowMs - DAY_MS;
  const inWindow = attempts.filter(a => Date.parse(a.started_at) >= windowStart);
  const collections = inWindow.length;
  const admitted = inWindow.filter(a => a.outcome === 'DECISIONS_RECORDED').length;
  const halts = inWindow.filter(a => a.outcome === 'SUPPRESSED').length;
  const providerFailures = inWindow.filter(a => a.reason === 'PROVIDER_UNAVAILABLE').length;
  const faults = session?.observed_state === 'FAULTED' ? 1 : 0;
  const fresh = collectionFreshness(latestAttemptAt(attempts), nowMs);
  const admittedLabel = collections > 0 ? `${admitted}/${collections}` : t(lang, 'not_available_yet');

  let level: HealthLevel; let reasonKey: CopyKey;
  if (fresh.label === 'unknown' && collections === 0) { level = 'unknown'; reasonKey = 'health_reason_no_data'; }
  else if (faults > 0) { level = 'red'; reasonKey = 'health_reason_fault'; }
  else if (fresh.label === 'stale') { level = 'red'; reasonKey = 'health_reason_stale'; }
  else if (fresh.label === 'catching_up') { level = 'amber'; reasonKey = 'health_reason_catching_up'; }
  else if (providerFailures > 0) { level = 'amber'; reasonKey = 'health_reason_provider_failure'; }
  else { level = 'green'; reasonKey = 'health_reason_ok'; }

  return { level, reasonLabel: t(lang, reasonKey), collections, admittedLabel, halts, faults, providerFailures };
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
// Publicly fixed protocol decimal counts for each network's native asset, used only to convert the
// hypothetical stake input into minor units for a linear projection. Not an invented business number.
export const NATIVE_DECIMALS: Record<Network, number> = { 'base-mainnet': 18, 'solana-mainnet': 9 };
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
  const decimals = NATIVE_DECIMALS[network];
  const stakeMinor = parseStakeToMinor(stakeWhole, decimals);
  const caveats = [t(lang, 'whatif_caveat_not_executed')];
  const noLeverageNote = t(lang, 'whatif_no_leverage');
  if (stakeMinor === null) return { stakeValid: false, stakeMinor: null, candidates: [], skippedOtherAssetCount: 0, caveats, noLeverageNote };

  const quoted = decisions.filter((d): d is StoredDecision & { trace: { amount_in_minor: string; result: { status: 'QUOTED'; gross_delta_minor: string } } } =>
    d.trace.result.status === 'QUOTED' && d.trace.amount_in_minor !== null && BigInt(d.trace.amount_in_minor) > 0n);
  const sameAsset = quoted.filter(d => d.trace.route[0]?.asset_in === network);
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
