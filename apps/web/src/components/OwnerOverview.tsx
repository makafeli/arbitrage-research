import { useState } from 'react';
import type { CommandReceipt, ControlApi, Network, Session } from '../api/client';
import { useResearchResource } from '../hooks/useResearchResource';
import { EmptyResearch, Exact, ResourceStatus } from './ResearchShared';
import {
  statusSummary, healthSummary, findingsSummary, whatIfSummary, roadToLiveChecklist, copy,
  parseProgressFile, fetchWindow, uuidV7FloorForTimestamp, MAX_WINDOW_PAGES, DAY_MS,
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

interface Props { active: boolean; api: ControlApi; sessions: Session[]; filter: Network | 'all'; commands: Record<string, { receipt?: CommandReceipt }> }
export function OwnerOverview({ active, api, sessions, filter, commands }: Props) {
  const [lang, setLang] = useState<Lang>(loadLang);
  const [sessionId, setSessionId] = useState('');
  const [stakeInput, setStakeInput] = useState('2');
  const c = copy[lang];
  const selected = sessions.find(session => session.session_id === sessionId) ?? null;
  const enabled = active && Boolean(selected);

  // Attempts are listed oldest-first (see ownerOverview.ts's windowing comment), so Status/Health
  // need the last 24h fetched explicitly rather than page one. `at` below is this fetch's own
  // completion time, used as `now` so Status/Health don't resample Date.now() on every render.
  const attempts = useResearchResource(enabled, 'owner-attempts:' + sessionId, signal =>
    fetchWindow(cursor => api.collectionAttempts(sessionId, cursor, signal), uuidV7FloorForTimestamp(Date.now() - DAY_MS), MAX_WINDOW_PAGES));
  const coverage = useResearchResource(enabled, 'owner-coverage:' + sessionId, signal => api.coverage(sessionId, signal));
  const groups = useResearchResource(enabled, 'owner-groups:' + sessionId, signal => api.decisionGroups(sessionId, undefined, signal));
  const decisions = useResearchResource(enabled, 'owner-decisions:' + sessionId, signal => api.decisions(sessionId, undefined, signal));
  const costAssessments = useResearchResource(enabled, 'owner-costs:' + sessionId, signal => api.costAssessments(sessionId, undefined, signal));
  const now = attempts.at ?? Date.now();

  const options = sessions.filter(session => filter === 'all' || session.network_id === filter || session.session_id === sessionId);
  const receipt = commands[sessionId]?.receipt ?? null;

  const attemptsTruncated = attempts.data?.truncated ?? false;
  const status = statusSummary(selected, receipt, attempts.data?.items ?? [], now, attemptsTruncated, lang);
  const health = healthSummary(attempts.data?.items ?? [], selected, now, attemptsTruncated, lang);
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
      <button disabled={!enabled} onClick={() => { attempts.refresh(); coverage.refresh(); groups.refresh(); decisions.refresh(); costAssessments.refresh(); }}>{lang === 'nl' ? 'Vernieuwen' : 'Refresh'}</button>
    </div>

    {!selected ? <EmptyResearch>{lang === 'nl' ? 'Kies een sessie om het overzicht te zien.' : 'Choose a session to see the overview.'}</EmptyResearch> : <>
      <ResourceStatus resource={attempts} />

      <section className="panel space-top" aria-labelledby="owner-status-title">
        <h3 id="owner-status-title">{lang === 'nl' ? '1. Status' : '1. Status'}</h3>
        <div className="research-facts">
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Modus' : 'Mode'}</span><strong>{status.modeLabel}</strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Toestand' : 'State'}</span><strong>{status.stateLabel}</strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Werker actief (leeftijd laatste verzameling)' : 'Worker alive (age of last collection)'}</span><strong>{status.workerAliveLabel}</strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Bronachterstand' : 'Source lag'}</span><strong>{status.sourceLagLabel}</strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Laatste commando-ontvangst' : 'Last command receipt'}</span><strong>{status.lastCommandReceiptLabel}</strong></div>
        </div>
      </section>

      <section className="panel space-top" aria-labelledby="owner-health-title">
        <h3 id="owner-health-title">{lang === 'nl' ? '2. Gezondheid (laatste 24 uur)' : '2. Health (last 24h)'}</h3>
        <p><span className={'pill ' + (health.level === 'red' ? 'red' : health.level === 'amber' ? 'amber' : health.level === 'green' ? 'green' : '')}>{c[`health_${health.level}` as keyof typeof c]}</span> — {health.reasonLabel}</p>
        <div className="research-metrics">
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Verzamelingen' : 'Collections'}</span><strong>{health.collections}</strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Toegelaten aandeel' : 'Admitted share'}</span><strong>{health.admittedLabel}</strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Halts/storingen' : 'Halts/faults'}</span><strong>{health.halts + health.faults}</strong></div>
          <div className="fact"><span className="metriclabel">{lang === 'nl' ? 'Providerfouten' : 'Provider failures'}</span><strong>{health.providerFailures}</strong></div>
        </div>
        <p className="tiny">{lang === 'nl'
          ? `Gebaseerd op verzamelpogingen van de afgelopen 24 uur (tot ${MAX_WINDOW_PAGES} pagina's).${attempts.data?.truncated ? ' Deze 24 uur bevat meer pogingen dan opgehaald; de telling hierboven is een ondergrens.' : ''}`
          : `Based on collection attempts from the last 24h (up to ${MAX_WINDOW_PAGES} pages).${attempts.data?.truncated ? ' This 24h window holds more attempts than were fetched; the counts above are a lower bound.' : ''}`}</p>
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
          <p className="tiny space-top">{lang === 'nl' ? `${findings.batches.length} oudste beslissingsbatch(es) geladen — niet per se de meest recente.` : `${findings.batches.length} oldest decision batch(es) loaded — not necessarily the most recent.`}</p>
          {findings.topCandidates.length > 0 && <div className="research-scroll" tabIndex={0} aria-label={lang === 'nl' ? 'Beste kandidaten' : 'Top candidates'}>
            <table className="research-table"><caption>{lang === 'nl' ? 'Beste kandidaten op gemodelleerde marge' : 'Top candidates by modeled edge'}</caption>
              <thead><tr><th>{lang === 'nl' ? 'Observatie' : 'Observation'}</th><th>{lang === 'nl' ? 'Route' : 'Route'}</th><th>{lang === 'nl' ? 'Bruto marge (minor)' : 'Gross edge (minor)'}</th><th>{lang === 'nl' ? 'Bewijsniveau' : 'Evidence ceiling'}</th></tr></thead>
              <tbody>{findings.topCandidates.map(candidate => <tr key={candidate.observationId}><td className="mono">{candidate.observationId}</td><td>{candidate.assetIn} → {candidate.assetOut}</td><td><Exact value={candidate.grossDeltaMinor} /></td><td>{candidate.evidenceLabel}</td></tr>)}</tbody>
            </table>
          </div>}
        </>}
      </section>

      <section className="panel space-top" aria-labelledby="owner-whatif-title">
        <h3 id="owner-whatif-title">{lang === 'nl' ? '4. Wat als (hypothetisch)' : '4. What if (hypothetical)'}</h3>
        <div className="research-field"><label htmlFor="owner-stake">{lang === 'nl' ? 'Inzet (USDC)' : 'Stake (USDC)'}</label><input id="owner-stake" value={stakeInput} onChange={event => setStakeInput(event.target.value)} inputMode="decimal" /></div>
        {whatIf && !whatIf.stakeValid && <p className="notice" role="alert">{lang === 'nl' ? 'Ongeldige inzet.' : 'Invalid stake.'}</p>}
        {whatIf && whatIf.stakeValid && (whatIf.candidates.length === 0 ? <EmptyResearch>{c.whatif_no_candidates}</EmptyResearch> : <div className="research-scroll" tabIndex={0} aria-label={lang === 'nl' ? 'Wat-als kandidaten' : 'What-if candidates'}>
          <table className="research-table"><caption>{lang === 'nl' ? 'Gemodelleerde bruto marge bij deze inzet' : 'Modeled gross edge at this stake'}</caption>
            <thead><tr><th>{lang === 'nl' ? 'Observatie' : 'Observation'}</th><th>{lang === 'nl' ? 'Gemodelleerde bruto marge (minor)' : 'Modeled gross edge (minor)'}</th><th>{lang === 'nl' ? 'Vastgelegde kostenbeoordeling' : 'Recorded cost assessment'}</th></tr></thead>
            <tbody>{whatIf.candidates.map(candidate => <tr key={candidate.observationId}><td className="mono">{candidate.observationId}</td><td><Exact value={candidate.modeledGrossEdgeMinor} /></td><td>{candidate.recordedCostLabel}</td></tr>)}</tbody>
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
