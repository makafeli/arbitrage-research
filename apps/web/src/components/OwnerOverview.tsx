import { useState } from 'react';
import type { CommandReceipt, ControlApi, Network, Session } from '../api/client';
import { useResearchResource } from '../hooks/useResearchResource';
import type { Resource } from '../hooks/useResearchResource';
import { EmptyResearch, Exact, OriginBadge } from './ResearchShared';
import {
  statusSummary, healthSummary, findingsSummary, whatIfSummary, roadToLiveChecklist, copy, parseProgressFile,
} from '../domain/ownerOverview';
import type { Lang } from '../domain/ownerOverview';
// Build-time JSON import: bundled by Vite. This file is maintained by hand outside this module's
// control, so its shape is validated at runtime via parseProgressFile() below rather than trusted
// with a cast; tests/ownerOverview.test.ts also validates the real file directly.
import progressFile from '../../../../planning/implementation-progress.json' with { type: 'json' };

const LANG_STORAGE_KEY = 'owner-overview-lang';
function loadLang(): Lang {
  try { const stored = localStorage.getItem(LANG_STORAGE_KEY); return stored === 'en' ? 'en' : 'nl'; } catch { return 'nl'; }
}
function saveLang(lang: Lang) { try { localStorage.setItem(LANG_STORAGE_KEY, lang); } catch { /* ponytail: best-effort only, a lost preference is not a failure */ } }

// A single, bilingual status line per resource group instead of one raw ResourceStatus per
// resource (L4): the Dutch page previously carried five English-only lines. Kept local to this
// component rather than folded into the shared ResearchShared.tsx, which other, deliberately
// English-only pages also use.
function CombinedResourceStatus({ resources, lang }: { resources: Resource<unknown>[]; lang: Lang }) {
  const loading = resources.some(r => r.loading);
  const failed = resources.filter(r => r.error !== null);
  const latestAt = resources.reduce<number | null>((max, r) => r.at !== null && (max === null || r.at > max) ? r.at : max, null);
  const anyMissingData = failed.some(r => r.data === null);
  return <>
    {loading && <p className="notice" role="status">{lang === 'nl' ? 'Onderzoeksgegevens laden…' : 'Loading research records…'}</p>}
    {failed.length > 0 && <div className="notice error-notice" role="alert">
      <strong>{lang === 'en' ? (anyMissingData ? 'Research records unavailable.' : 'Stale snapshot retained.') : (anyMissingData ? 'Onderzoeksgegevens niet beschikbaar.' : 'Verouderde momentopname behouden.')}</strong>
    </div>}
    {latestAt !== null && <p className="tiny space-top">{lang === 'nl' ? `Momentopname ontvangen ${new Date(latestAt).toISOString()}. Handmatige verversing; dit paneel claimt geen doorlopende dekking.` : `Snapshot received ${new Date(latestAt).toISOString()}. Explicit refresh; this panel does not claim continuous coverage.`}</p>}
  </>;
}

interface Props { active: boolean; api: ControlApi; sessions: Session[]; filter: Network | 'all'; commands: Record<string, { receipt?: CommandReceipt }> }
export function OwnerOverview({ active, api, sessions, filter, commands }: Props) {
  const [lang, setLang] = useState<Lang>(loadLang);
  const [sessionId, setSessionId] = useState('');
  const [stakeInput, setStakeInput] = useState('2');
  const c = copy[lang];
  const selected = sessions.find(session => session.session_id === sessionId) ?? null;
  const enabled = active && Boolean(selected);

  // Collection coverage is a single, un-paginated, whole-session aggregate (unlike the old 24h
  // attempt-page walk, it can never be truncated), so it drives both the Status source-lag reading
  // and the Health counters (H1).
  const collectionCoverage = useResearchResource(enabled, 'owner-collection-coverage:' + sessionId, signal => api.collectionCoverage(sessionId, signal));
  const coverage = useResearchResource(enabled, 'owner-coverage:' + sessionId, signal => api.coverage(sessionId, signal));
  const groups = useResearchResource(enabled, 'owner-groups:' + sessionId, signal => api.decisionGroups(sessionId, undefined, signal));
  const decisions = useResearchResource(enabled, 'owner-decisions:' + sessionId, signal => api.decisions(sessionId, undefined, signal));
  const costAssessments = useResearchResource(enabled, 'owner-costs:' + sessionId, signal => api.costAssessments(sessionId, undefined, signal));
  // Live wall-clock time for the worker heartbeat only: ConnectedApp re-renders this component on
  // its own 5s poll tick, and last_heartbeat_at is refreshed on every poll, so the worker-alive
  // reading is never frozen at fetch time. Collection freshness, however, must use the coverage
  // resource's OWN fetch time (`collectionCoverage.at`), not this live clock: collection-coverage is
  // fetched once per session selection, not re-polled, so ageing it against a live clock would turn
  // a healthy session "stale"/red purely because the page stayed open (F1, a re-break of round-2's H1).
  const now = Date.now();
  const coverageAtMs = collectionCoverage.at ?? now;

  const options = sessions.filter(session => filter === 'all' || session.network_id === filter || session.session_id === sessionId);
  const receipt = commands[sessionId]?.receipt ?? null;

  const status = statusSummary(selected, receipt, collectionCoverage.data ?? null, now, coverageAtMs, lang);
  const health = healthSummary(collectionCoverage.data ?? null, selected, coverageAtMs, lang);
  const findings = findingsSummary(coverage.data ?? null, groups.data?.items ?? [], decisions.data?.items ?? [], lang);
  const whatIf = selected ? whatIfSummary(stakeInput, selected.network_id, decisions.data?.items ?? [], costAssessments.data?.items ?? [], lang) : null;
  const checklist = roadToLiveChecklist(parseProgressFile(progressFile), lang);

  return <section className="research-workspace section-spacer" aria-labelledby="owner-overview-title">
    <div className="sectionhead">
      <div><h2 id="owner-overview-title">{lang === 'nl' ? 'Eigenaaroverzicht' : 'Owner overview'}</h2><p>{lang === 'nl' ? 'Wat de sessie nu doet, in gewone taal.' : 'What the session is doing right now, in plain language.'}</p></div>
      <nav className="trading-modes" aria-label={lang === 'nl' ? 'Taal' : 'Language'}>
        <button aria-pressed={lang === 'nl'} onClick={() => { setLang('nl'); saveLang('nl'); }}>Nederlands</button>
        <button aria-pressed={lang === 'en'} onClick={() => { setLang('en'); saveLang('en'); }}>English</button>
      </nav>
    </div>
    <div className="panel research-toolbar">
      <div className="research-field"><label htmlFor="owner-overview-session">{lang === 'nl' ? 'Sessie' : 'Session'}</label>
        <select id="owner-overview-session" value={sessionId} onChange={event => setSessionId(event.target.value)}>
          <option value="">{lang === 'nl' ? 'Kies een sessie' : 'Choose a session'}</option>
          {options.map(session => <option key={session.session_id} value={session.session_id}>{session.session_id} · {session.network_id} · {session.mode}</option>)}
        </select>
      </div>
      <button disabled={!enabled} onClick={() => { collectionCoverage.refresh(); coverage.refresh(); groups.refresh(); decisions.refresh(); costAssessments.refresh(); }}>{lang === 'nl' ? 'Vernieuwen' : 'Refresh'}</button>
    </div>

    {!selected ? <EmptyResearch>{lang === 'nl' ? 'Kies een sessie om het overzicht te zien.' : 'Choose a session to see the overview.'}</EmptyResearch> : <>
      <CombinedResourceStatus resources={[collectionCoverage, coverage, groups, decisions, costAssessments]} lang={lang} />

      <section className="panel space-top" aria-labelledby="owner-status-title">
        <h3 id="owner-status-title">{lang === 'nl' ? '1. Status' : '1. Status'}</h3>
        <div className="research-facts">
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Modus' : 'Mode'}</span><strong>{status.modeLabel}</strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Toestand' : 'State'}</span><strong>{status.stateLabel}</strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Werker actief (leeftijd hartslag)' : 'Worker alive (heartbeat age)'}</span><strong>{status.workerAliveLabel}</strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Bronachterstand' : 'Source lag'}</span><strong>{status.sourceLagLabel}</strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Laatste commando-ontvangst' : 'Last command receipt'}</span><strong>{status.lastCommandReceiptLabel}</strong></div>
        </div>
      </section>

      <section className="panel space-top" aria-labelledby="owner-health-title">
        <h3 id="owner-health-title">{lang === 'nl' ? '2. Gezondheid (sinds start sessie)' : '2. Health (since session start)'}</h3>
        <p><span className={'pill ' + (health.level === 'red' ? 'red' : health.level === 'amber' ? 'amber' : health.level === 'green' ? 'green' : '')}>{c[`health_${health.level}` as keyof typeof c]}</span> — {health.reasonLabel}</p>
        <div className="research-metrics">
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Verzamelpogingen' : 'Collection attempts'}</span><strong><Exact value={health.collections} /></strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Toegelaten aandeel' : 'Admitted share'}</span><strong>{health.admittedLabel}</strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Mislukte pogingen' : 'Failed attempts'}</span><strong><Exact value={health.failedAttempts} /></strong></div>
        </div>
        <p className="tiny">{c.health_failed_attempts_note}</p>
      </section>

      <section className="panel space-top" aria-labelledby="owner-findings-title">
        <h3 id="owner-findings-title">{lang === 'nl' ? '3. Bevindingen' : '3. Findings'}</h3>
        {!findings.hasData ? <EmptyResearch>{lang === 'nl' ? 'Nog geen bevindingen voor deze sessie.' : 'No findings for this session yet.'}</EmptyResearch> : <>
          {findings.totals && <div className="research-metrics">
            <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Ruwe observaties' : 'Raw observations'}</span><strong><Exact value={findings.totals.rawObservations} /></strong></div>
            <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Genoteerde kandidaten' : 'Quoted candidates'}</span><strong><Exact value={findings.totals.quotedCandidates} /></strong></div>
            <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Afgewezen' : 'Rejected'}</span><strong><Exact value={findings.totals.rejected} /></strong></div>
            <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Geen route' : 'No route'}</span><strong><Exact value={findings.totals.noRoute} /></strong></div>
            <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Data onbeschikbaar' : 'Data unavailable'}</span><strong><Exact value={findings.totals.dataUnavailable} /></strong></div>
          </div>}
          <p className="tiny space-top">{lang === 'nl' ? `Eerste ${findings.batches.length} groepen geladen.` : `First ${findings.batches.length} groups loaded.`}</p>
          {(findings.firstBatchAtMs !== null || findings.lastBatchAtMs !== null) && <p className="tiny">{lang === 'nl'
            ? `Eerste batch ${findings.firstBatchAtMs === null ? 'onbekend' : new Date(findings.firstBatchAtMs).toISOString()} · laatste batch ${findings.lastBatchAtMs === null ? 'onbekend' : new Date(findings.lastBatchAtMs).toISOString()}.`
            : `First batch ${findings.firstBatchAtMs === null ? 'unknown' : new Date(findings.firstBatchAtMs).toISOString()} · last batch ${findings.lastBatchAtMs === null ? 'unknown' : new Date(findings.lastBatchAtMs).toISOString()}.`}</p>}
          {findings.batches.length > 0 && <ul className="tiny">
            {findings.batches.map((batch, index) => <li key={index}>{new Date(batch.windowStartMs).toISOString()} — {batch.quotedCandidates} {lang === 'nl' ? 'kandidaten' : 'candidates'}</li>)}
          </ul>}
          {findings.topCandidates.length > 0 && <div className="research-scroll" tabIndex={0} aria-label={lang === 'nl' ? 'Beste kandidaten' : 'Top candidates'}>
            <table className="research-table"><caption>{lang === 'nl' ? `Beste kandidaten op gemodelleerde marge, ${c.top_candidates_source}` : `Top candidates by modeled edge, ${c.top_candidates_source}`}</caption>
              <thead><tr><th>{lang === 'nl' ? 'Observatie' : 'Observation'}</th><th>{lang === 'nl' ? 'Route' : 'Route'}</th><th>{lang === 'nl' ? 'Bruto marge (minor)' : 'Gross edge (minor)'}</th><th>{lang === 'nl' ? 'Bewijsniveau' : 'Evidence ceiling'}</th><th>{lang === 'nl' ? 'Herkomst' : 'Origin'}</th></tr></thead>
              <tbody>{findings.topCandidates.map(candidate => <tr key={candidate.observationId}><td className="mono">{candidate.observationId}</td><td>{candidate.assetIn} → {candidate.assetOut}</td><td><Exact value={candidate.grossDeltaMinor} /></td><td>{candidate.evidenceLabel}</td><td><OriginBadge origin={candidate.datasetOrigin} lang={lang} /></td></tr>)}</tbody>
            </table>
          </div>}
        </>}
      </section>

      <section className="panel space-top" aria-labelledby="owner-whatif-title">
        <h3 id="owner-whatif-title">{lang === 'nl' ? '4. Wat als (hypothetisch)' : '4. What if (hypothetical)'}</h3>
        <div className="research-field"><label htmlFor="owner-stake">{lang === 'nl' ? 'Inzet (USDC)' : 'Stake (USDC)'}</label><input id="owner-stake" value={stakeInput} onChange={event => setStakeInput(event.target.value)} inputMode="decimal" /></div>
        <p className="tiny">{c.stake_ticket_note}</p>
        {whatIf && !whatIf.stakeValid && <p className="notice" role="alert">{lang === 'nl' ? 'Ongeldige inzet.' : 'Invalid stake.'}</p>}
        {whatIf && whatIf.stakeValid && (whatIf.candidates.length === 0 ? <EmptyResearch>{c.whatif_no_candidates}</EmptyResearch> : <div className="research-scroll" tabIndex={0} aria-label={lang === 'nl' ? 'Wat-als kandidaten' : 'What-if candidates'}>
          <table className="research-table"><caption>{lang === 'nl' ? `Gemodelleerde bruto marge bij deze inzet, ${c.top_candidates_source}` : `Modeled gross edge at this stake, ${c.top_candidates_source}`}</caption>
            <thead><tr><th>{lang === 'nl' ? 'Observatie' : 'Observation'}</th><th>{lang === 'nl' ? 'Gemodelleerde bruto marge (minor)' : 'Modeled gross edge (minor)'}</th><th>{lang === 'nl' ? 'Vastgelegde kostenbeoordeling' : 'Recorded cost assessment'}</th><th>{lang === 'nl' ? 'Herkomst' : 'Origin'}</th></tr></thead>
            <tbody>{whatIf.candidates.map(candidate => <tr key={candidate.observationId}><td className="mono">{candidate.observationId}</td><td><Exact value={candidate.modeledGrossEdgeMinor} /></td><td>{candidate.recordedCostLabel}</td><td><OriginBadge origin={candidate.datasetOrigin} lang={lang} /></td></tr>)}</tbody>
          </table>
        </div>)}
        {whatIf && whatIf.skippedOtherAssetCount > 0 && <p className="tiny">{c.whatif_other_asset} ({whatIf.skippedOtherAssetCount})</p>}
        {whatIf?.caveats.map(caveat => <p className="notice" key={caveat}>{caveat}</p>)}
        <p className="notice">{whatIf?.noLeverageNote}</p>
      </section>

      <section className="panel space-top" aria-labelledby="owner-road-title">
        <h3 id="owner-road-title">{lang === 'nl' ? '5. Route naar PAPER/live' : '5. Road to PAPER/live'}</h3>
        <ul className="research-checklist">
          {checklist.map(item => <li key={item.id}><strong>{item.id}</strong> — <span className="pill">{item.stateLabel}</span>{item.issueUrl && <> · <a href={item.issueUrl} target="_blank" rel="noreferrer">{lang === 'nl' ? 'issue' : 'issue'}</a></>}<p className="tiny">{item.remainingAcceptance}</p></li>)}
        </ul>
      </section>
    </>}
  </section>;
}
