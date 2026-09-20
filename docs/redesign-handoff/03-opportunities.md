# 03 · Opportunities

Bron: `apps/web/src/components/pages/OpportunitiesPage.tsx`, `components/DecisionExplorer.tsx`, `components/RecordedOpportunities.tsx`, `FrozenSessionExport.tsx`, `CaptureAuditPanel.tsx`, `CostAssessmentWorkspace.tsx`, `DecisionContinuityEvidence.tsx`, `ChainFreshnessEvidence.tsx`, `ResearchShared.tsx`, `components/ui/Tabs.tsx`.

Screenshots:
- `screenshots/02a-opp-captured-desktop.png` · `-mobile.png` — tab "Captured"
- `screenshots/02b-opp-decisions-desktop.png` · `-mobile.png` — tab "Decisions", sessie gekozen
- `screenshots/02c-opp-costs-desktop.png` · `-mobile.png` — tab "Costs", cost-workspace open na "Assess hypothetical costs"
- `screenshots/02d-opp-exports-desktop.png` · `-mobile.png` — tab "Exports", frozen export voorbereid
- `screenshots/02e-decision-drawer-desktop.png` · `-mobile.png` — drawer "Decision evidence detail"
- Record dialog: `screenshots/01b-record-dialog-*` (zelfde als Overview)

Schil: zie [00-shell.md](00-shell.md). h1: `Opportunities`.

## Doel

Bewijs bekijken, nu verdeeld over vier tabs in plaats van één lange pagina: de ruwe opportunity-records
(`Captured`), de **Decision evidence explorer** (`Decisions`, standaard tab), een werkruimte om handmatige
kostenaannames op één quote toe te passen (`Costs`), en een bevroren database-export (`Exports`). Alles is
"persisted evidence": niets hier is een winst of een uitvoerbare trade.

## Opbouw (van boven naar beneden)

```
(schil)
div.tabs[role=tablist][aria-label="Opportunities sections"]   Captured · Decisions (standaard) · Costs · Exports
div.tabpanel[role=tabpanel]#panel-opp-captured[aria-labelledby=tab-opp-captured]   RecordedSection — identiek aan 02-overview.md §RecordedTable
section.section-spacer.research-workspace      DecisionExplorer  (aria-labelledby decision-explorer-title, blijft altijd gemount zolang de Opportunities-pagina actief is)
├─ .sectionhead                      h2 "Decision evidence explorer" + p  |  span.pill.blue "PERSISTED EVIDENCE"
├─ .panel.research-toolbar           select "Decision session" · select "Origin on this page" · button "Refresh evidence"  (verborgen zodra de gate-tekst hieronder geldt)
├─ div.tabpanel[role=tabpanel]#panel-opp-decisions[aria-labelledby=tab-opp-decisions]
│     [capability decision_history uit]  p.notice "Decision history is unavailable in this API version. Missing observations and execution accounting are unknown, not zero."
│     [geen sessie]  p.notice "Select a session to inspect its stored evidence. No dataset has been substituted."
│     [sessie gekozen]  §D coverage-paneel, §E observaties-tabel, §G groepen-tabel
├─ div.tabpanel[role=tabpanel]#panel-opp-costs[aria-labelledby=tab-opp-costs]
│     dezelfde gate-tekst als hierboven, óf (als capability aan staat en er een sessie is) "Assess hypothetical
│     costs from a quoted observation on the Decisions tab to load a cost workspace." totdat een quote is
│     gekozen — dan CostAssessmentWorkspace (§F)
└─ div.tabpanel[role=tabpanel]#panel-opp-exports[aria-labelledby=tab-opp-exports]
      dezelfde gate-tekst, óf FrozenSessionExport (§B) → CaptureAuditPanel (§C)
dialog.research-dialog "Decision evidence detail"   (§H)
```

Alle vier de tabs — `Captured`, `Decisions`, `Costs`, `Exports` — zijn echte `TabPanel`s (`div.tabpanel[role=tabpanel]`,
`hidden` op het element, `id=panel-opp-…`, `aria-labelledby=tab-opp-…`) en blijven allemaal gemount zolang de
Opportunities-pagina actief is. `DecisionExplorer` rendert de laatste drie panels zelf (`opp-decisions`, `opp-costs`,
`opp-exports`) binnen één component, zodat state (gekozen sessie, geopende cost-workspace) bewaard blijft bij
tabwissel — maar elk panel is een eigen `TabPanel`-element met eigen `id`/`aria-labelledby`, niet een gedeelde
`hidden`-div. De gate-tekst ("Decision history is unavailable…" / "Select a session to inspect its stored
evidence…") rendert daardoor *binnen elk van de drie panels* zolang die van toepassing is — niet één keer boven
de tabs. Klikken op "Assess hypothetical costs for …" in de Decisions-tabel schakelt de Costs-tab actief **en**
zet toetsenbordfocus op de Costs-tabknop (`focusTab`).

## A. Sectiekop en toolbar (Decisions-tab)

- h2 `Decision evidence explorer`
- p `Persisted observations, grouped candidates and rejection evidence. Quotes alone do not establish arbitrage profit.`
- pill `PERSISTED EVIDENCE` (blue)
- Capability uit: `Decision history is unavailable in this API version. Missing observations and execution accounting are unknown, not zero.`

Toolbar `.panel.research-toolbar` (zichtbaar ongeacht welke tab actief is, want hij staat boven de drie
tabpanels, niet er binnenin):
| Veld | id | Opties |
|---|---|---|
| `Decision session` | `decision-session` | `Choose a session` + per sessie `{session_id} · {network_id} · {mode}`. Gefilterd op chain-filter. Disabled zolang een kostenverzoek loopt. |
| `Origin on this page` | `decision-origin` | `All recorded origins` · `RECORDED LIVE` · `SYNTHETIC` · `MANUALLY CONSTRUCTED` |
| knop | — | `Refresh evidence` (disabled tijdens laden) |

Scope-regel (p.tiny): `Session scope: {id}. The chain selector filters session choices only; changing it does not change this selected evidence scope. Origin filtering affects the observation page below. Aggregate counts include all origins in this session. Counts and pages are separate received snapshots and can differ while recording continues.`

## B. Exports-tab — FrozenSessionExport (`section.panel.space-top.frozen-export`, aria-label `Frozen session export`)

Capability `session_export` uit → alleen `p.notice`: `Frozen session exports are unavailable in this API version. Page exports remain limited received snapshots.`

```
.sectionhead   h3 "Freeze a complete stored session" + p "A single database snapshot includes decisions, paper accounting, collection attempts, manual cost assessments and capture dependencies."  |  span.pill.blue "FROZEN EXPORT"
p.tiny         "Selected session: {id}. Live browsing pages can change independently. The six defined source datasets are decisions, paper runs, paper journal events, capture catalog entries, collection attempts and cost assessments. This is not a full database backup: configuration content, audit records and control history are outside the export. Up to 10,000 source rows and 8 MiB are read together; larger sessions fail without truncation."
.research-actions   [Prepare frozen session export | Preparing frozen snapshot… | Prepare a new frozen snapshot]  [Download frozen JSON]  [Download frozen CSV]
p.notice.error-notice[role=alert]   (bij fout, zie lijst)
p.tiny[role=status]                 "Reading one database snapshot and verifying its content digest…"  (tijdens laden)
— na succes —
.notice[role=status]   strong "Frozen database snapshot verified" · p "{exported_at} · {export_id}" · p.mono "Content SHA-256: {hash}" · p "Both downloads use this exact received bundle. Downloading does not refresh it."
dl.research-facts      zes .export-count: "decisions", "paper runs", "paper journal events", "capture catalog entries", "collection attempts", "cost assessments" → exact getal
p.notice               "Defined research datasets: COMPLETE within this session and database transaction. Scheduled collection completeness: UNKNOWN. This export cannot establish unrecorded activity, market coverage, executable opportunities or realized profit."
p.tiny                 "Amounts remain exact base-unit integer strings. Token decimals are not retained in this database, configuration content is represented by its digest, and source quote costs remain unknown. Manual cost assessments retain their separate hypothetical assumptions. Paper reasons are redacted and paper identifiers are pseudonymized consistently; source event digests identify original journal payloads."
CaptureAuditPanel      (§C)
details                summary "Capture dependency availability: {n} missing catalog entries · {m} raw artifacts unverified"
                       p.notice "A present catalog entry does not prove that its raw files remain available. Raw artifacts are excluded and unverified; expiration status is UNKNOWN. This bundle alone cannot reproduce capture-based calculations."
                       max 20 × p.tiny "{catalog_status} · {capture_id}" / "{manifest_digest} · expiration UNKNOWN · raw artifact NOT VERIFIED"
                       (>20) p.tiny "Showing the first 20 dependencies. Both frozen downloads contain every dependency in the snapshot."
p.tiny                 "CSV contains one typed row per record with its complete payload in a JSON cell. Parse payload_json to retain exact large integers and nested ledger commands. Formula guards protect scalar cells; amounts are not converted to spreadsheet numbers."
```

Foutteksten (`failure()` in `FrozenSessionExport.tsx`):
- limiet/413: `This session exceeds the complete export bound of 10,000 source rows or 8 MiB. No partial export was prepared. Retain the database and use an operator-reviewed extraction for a larger session.`
- EXPORT_BUSY: `Another export is using the service export capacity. Retry preparation when it finishes; the previous frozen bundle remains available.`
- 429: `The service request limit was reached. Wait before retrying preparation; the previous frozen bundle remains available.`
- 401/403: `Your session cannot read this export. Sign in again with an operator that can read the selected research session.`
- 404: `The selected session export is unavailable to this operator.`
- anders: `The complete snapshot could not be prepared or its schema, counts or content digest could not be verified. No new download is available; check the service and retry.`
- serialisatie: `The bounded download could not be serialized. No partial file was prepared.`

Downloads: `arbitrage-frozen-{export_id}.json` / `.csv` via Blob-link. Geen server-roundtrip.

## C. CaptureAuditPanel (`section.notice.space-top`, aria-label `Local capture audit`)

Genest in de Exports-tab, onder FrozenSessionExport.

```
h4              "Check the retained capture files"
p.tiny          "Download a request for every reference in this frozen export. Run the local auditor beside the quiesced capture volume, then import its JSON result here. No filesystem paths, credentials or report files are sent to the API."
.research-actions   [Download capture audit request]  label "Import local audit JSON" + input[type=file].audit-file  ([Download bound audit report] na import)
p.tiny          "Audit request covers {n} capture references. Maximum: 1,000; larger requests are refused, never truncated."
details         summary "Local audit command and trust limits"
                p.tiny "Run from the checked-out repository with private local paths and an explicit audit time in Unix milliseconds. See docs/25-EXPORT-CAPTURE-AUDIT.md. Save output outside the capture root."
                p.mono.tiny "python3 scripts/export_capture_audit.py --root /private/captures --request /private/request.json --now-ms AUDIT_TIME_MS > /private/report.json"
                p.tiny "This browser validates the report structure and its link to this export, not who executed it or the truth of a supplied timestamp. A hash is not a signature. Empty reference sets do not establish complete coverage."
p[role=status]  "Checking local report binding..."  (busy)
p[role=alert|status]   fout: "No audit request was prepared. This export exceeds the 1,000-reference limit or contains unsupported capture identities."  of  "The local report is invalid, too large, incomplete in its reference list, or belongs to a different frozen export. No result was attached."
— na import —
div[role=status]   strong "Imported local audit: {status}" · p.tiny "Operator-supplied, not independently authenticated. Checked at Unix milliseconds: {ms}." · p.tiny "{reported} of {requested} references reported. The original frozen JSON/CSV is unchanged; this is a separate point-in-time report." · p.tiny "{STATUS}: {count} · …" · max 20 × p.tiny per dependency · p.tiny "Replay: NOT ASSESSED. Market eligibility: false. Execution authorization: false. This result does not certify continuous retention, every worker volume or market coverage."
```

## D. Decisions-tab — Coverage-paneel (`section.panel.space-top`, aria-label `Session evidence counts`)

h3 `Stored observation counts · all origins`. `.research-metrics` (4 kolommen, ≤900px 2, ≤540px compact) met 8 `.fact`:
`Raw observations` · `Quoted candidates` · `Unique quoted groups` · `Rejected` · `No route` · `Data unavailable` · `Eligible attempts` · `Reconciled transactions`. Waarde = `Exact` (mono, tabular; `Unknown` bij null).

- p.notice: `Collection completeness: UNKNOWN. Stored observations are not a census of market activity. Unique quoted groups include negative gross quotes; they are not counts of profitable trades.`
- p.tiny: `Stored observation window: {start ISO|Unknown} → {end ISO|Unknown}. This range does not establish uninterrupted collection.`

## E. Decisions-tab — Tabel "Raw decision observations"

Sectiekop: h3 `Raw decision observations` · p `{n} shown on this loaded page. External costs and full transaction simulation remain outside this gross quote evidence.` · rechts `ExportButton` `Export observation page JSON`.

Leeg: `No matching observations on this page. Other pages and uncollected market activity remain separate.`

`table.research-table`, caption `Stored decision observations · exact start asset minor units`:

| Kolom | Inhoud |
|---|---|
| `Observation / origin` | strong.mono observation_id · route-sub `{network_id} · {mode}` · `OriginBadge` |
| `Result / reasons` | pill: `CANDIDATE · GROSS QUOTE` (QUOTED) of status met spaties · route-sub: `Net unknown — external costs incomplete` of reason codes met ` · ` (of `No reason supplied`) |
| `Gross delta` | Exact gross_delta_minor (alleen QUOTED, anders `Unknown`) · route-sub start asset (of `Start asset unavailable`) |
| `Evidence` | button `Inspect evidence` (aria-label `Inspect decision {id}`) · [QUOTED + capability cost_assessments] button `Assess hypothetical costs` (aria-label `Assess hypothetical costs for {id}`) — **schakelt de pagina naar de Costs-tab** |

`OriginBadge`: `RECORDED LIVE INPUT` (blue) · `SYNTHETIC DATASET` (amber) · `MANUALLY CONSTRUCTED` (amber).

Pagination: `Previous observations` · `Page {n} · up to 25 records` · `Next observations`.

## F. Costs-tab — CostAssessmentWorkspace (`section.panel.space-top.cost-workspace`)

Verschijnt op de Costs-tab pas nadat op "Assess hypothetical costs" is geklikt op de Decisions-tab (vóór die klik
is de Costs-tab leeg — er is geen placeholdertekst, de sectie rendert simpelweg niet). Eén werkruimte per
gekozen observatie (key = sessie:observatie). Zolang een verzoek loopt is de sessie-select op de Decisions-tab
disabled en de modus-switch geblokkeerd.

```
.sectionhead    h3 "Hypothetical cost research" + p "Apply explicit manual assumptions to one retained gross quote."  |  span.pill.amber "MANUAL ASSUMPTIONS"
.notice         p "Decision: {observation_id} · {network_id}" · p "Start asset: {asset}" · p "Historical gross delta: {n} start-asset base units." · OriginBadge · p "Source origin and manual scenario origin stay separate. Every result remains a hypothetical candidate. Saving an assessment creates no paper bookings and changes no quote."
form.connected-form.cost-form > fieldset
├─ .cost-field-grid   input#cost-scenario "Scenario identifier" (placeholder "e.g. conservative-fees", max 64) · input#cost-version "Scenario version" (default "1")
├─ input#cost-reference "Manual assumption reference" (placeholder "e.g. research-note-2026-09", max 128)
│  p.tiny "Use an identifier or SHA-256 digest, starting with a letter or number. Letters, numbers, dots, colons, underscores and hyphens only. Never paste provider URLs or secrets."
├─ h4 "Transaction cost components"
│  p.tiny "All amounts are exact integer base units. Blank amounts are not zero. Choose “Known manual amount” and enter 0 to declare zero explicitly."
│  p.notice  Base: "Base execution includes its priority part. L1 data is separate; priority must not be subtracted again. Pool fees and price impact are already in the gross quote."
│            Solana: "Solana base execution and priority fees are separate. Base L1 data does not apply. Pool fees and price impact are already in the gross quote."
├─ .cost-component-list  per kind een .cost-component (2 kolommen):
│     links strong {naam} + p.tiny "Native base units · {network}" | "Start-asset base units"
│     rechts .research-field: select "{naam} input" (Missing · unknown | Known manual amount) + [Known] input "{naam} base units"
│     kinds: Network execution · Base L1 data (alleen Base) · Solana priority fee (alleen Solana) · Relay tip · Funding cost · Account setup · Other transaction costs
├─ details.cost-valuation (open zodra een native kind Known is)
│     summary "Native fee valuation · exact manual ratio"
│     p.tiny "Convert native base units into start-asset base units. Supply both sides of the ratio; decimals must already be accounted for. Costs round upward. Leaving the ratio empty keeps native fee valuation unknown, including when a native amount is zero."
│     .cost-field-grid  input#cost-numerator "Start-asset base units · numerator" · input#cost-denominator "Native base units · denominator"
├─ .cost-field-grid   input#cost-valued-at "Valuation as of · Unix milliseconds" (default = observed_at) · input#cost-max-age "Maximum historical valuation age · ms" (default 60000)
│  p.tiny "Initial as-of value equals this historical decision’s timestamp ({ISO}); it is a manual assumption, not a retrieved valuation. Future valuations are rejected. Maximum age is 1–86,400,000 ms."
├─ select#cost-funding "Funding assumption": "Unknown · net remains unknown" | "Own virtual capital · hypothetical"
├─ select#cost-overhead "Shared operating overhead": "Not allocated · fully allocated net unknown" | "Unknown allocation" | "Explicit allocation in start-asset units"
│  [ALLOCATED] input#cost-allocation "Overhead allocation · start-asset base units" · input#cost-allocation-method "Allocation method identifier" · p.tiny "Allocation uses the scenario version and assumption reference above."
└─ .checkbox-label "I reviewed the exact units, historical valuation and hypothetical assumptions."
p.notice.error-notice[role=alert]   (fout)
button.primary   "Save hypothetical cost assessment" | "Saving cost assessment…" | "Retry identical cost assessment" | "Cost assessment saved"
p.notice[role=status]  (pending) "Assessment scope is locked until delivery is resolved. Retries retain the original session, observation, assumptions and idempotency key."
— na succes —
.notice[role=status] "Saved immutable assessment {record_id}. Source quote and paper balances are unchanged."
AssessmentResult   (zie hieronder)
button "Prepare another cost scenario"
.sectionhead   h3 "Retained cost assessments" + p "Showing this observation within the loaded session page. Other pages can contain further scenarios."  |  button "Refresh cost history"
ResourceStatus
p.notice (leeg) "No assessments for this observation on the loaded page. Missing history does not imply zero costs."
per assessment: details.cost-history  summary "{scenario_id} · v{version} · {recorded_at}"  →  AssessmentResult + ExportButton "Export cost assessment {record_id}"
Pagination "cost assessments"
```

Elke wijziging in het formulier zet de checkbox weer uit. Fout bij onzekere levering: `Assessment delivery is uncertain. Retry the identical request to recover its durable result.`

**AssessmentResult** (`section.space-top`, aria-label `Cost result {id}`):
- `span.pill.amber` `HYPOTHETICAL · CANDIDATE`
- `dl.research-facts`: `Gross after quote-included costs` · `Net after transaction costs` · `Net after allocated overhead` · `Amount unit` (`{asset} · base units`) · `Assumptions` (`{id} · {version} · MANUALLY_CONSTRUCTED`) · `Funding / overhead` · `Maximum historical valuation age` · `Calculation version`
- `.notice` strong `Net remains unknown` + één p per incomplete_reason (als aanwezig)
- tabel caption `Retained cost assumptions and exact valuation`: `Component` · `Declared amount / asset` · `In start-asset base units`
- p.notice `A positive hypothetical net is not a simulated fill, execution eligibility or realized profit. Negative values are retained. Unallocated overhead remains unknown.`
- `details` summary `Immutable assessment references` → p.mono `Assessment:` · `Scenario:` · `Source decision:` · `Configuration:` · p `Assumption reference:`

Mobiel ≤540px: `.cost-field-grid` en `.cost-component` 1 kolom.

## G. Decisions-tab — Tabel "Grouped observation windows"

Sectiekop: h3 `Grouped observation windows · all origins` · p `Grouping separates dataset origin, network and configuration. This page includes rejected and unavailable observation groups.`

Leeg: `No grouped observations were returned for this page.`

`table.research-table`, caption `Grouping evidence by stored window`: `Group / origin` (mono key · route-sub `{version} · {ISO}` · OriginBadge) · `Raw` · `Quoted` · `Rejected` · `No route` · `Unavailable`.

Pagination `groups`.

## H. Drawer "Decision evidence detail" (`dialog.research-dialog`)

Screenshot `02e-decision-drawer-*`. Was een gecentreerde modal (`min(850px, 100% − 32px)`), is nu een **rechter
zijpaneel**: `dialog.drawer`-stijl maar 850px breed (100% ≤760px), volle hoogte, slide-in 180ms. Opent bij "Inspect
evidence" op de Decisions-tab. Focus trap over alle knoppen/inputs, `Escape` sluit. Component: `ResearchDialog`
in `ResearchShared.tsx`.

```
.dialoghead     h2#research-detail-title "Decision evidence detail"  |  button "Close evidence detail"
ResourceStatus (detail)
DecisionContinuityEvidence   section.panel  (§H1)
DecisionDetail               .research-detail  (§H2)
button "Refresh selected trace"
```

### H1. Source continuity (`section.panel.space-top`, aria-label `Source continuity`)

- `.sectionhead` h3 `Source continuity` | button `Refresh source status`
- p.tiny `A separate database check of the sources linked to this historical decision. This does not update the captured market price or authorize trading.`
- geen data: `Source status unavailable. Missing evidence is not a healthy source.`
- p.notice strong: `No known source invalidation` / `Source invalidated` / `Source continuity not tracked` / `Source continuity unverifiable`
- (inactief/fout) `Last received source status only. Current continuity is unknown until a successful refresh.`
- dl: `Database check received` · `Bound capture references` (`{bound} of {count}`) · `Continuity policy`
- per invalidation reason p.notice: `The recorded stream lost continuity.` / `The source stream halted after a provider failure.` / `The source stream halted at a resource limit.` / `The source stream halted after invalid input.` + mono code
- UNTRACKED: `No complete recorded continuity links exist for this decision. Historical data is not retroactively approved.` · UNVERIFIABLE: `The available source evidence or this chain is not covered by the Base continuity policy.`
- p.tiny `Continuity only: freshness, tick coverage, simulation, costs and profitability require separate evidence. Original decision and capture history remain unchanged.`

### H2. DecisionDetail

- `.research-actions`: OriginBadge + pill status (`CANDIDATE · GROSS QUOTE` of status)
- `dl.research-facts`: `Observation` · `Durable trace` · `Stored at` · `Observed at` · `Capture / evaluation elapsed time` (`{n} ms` + tiny `Processing duration at observation time. Captured chain age is shown separately below.`) · `Session / experiment` · `Network / mode` · `Source kind` · `Frozen configuration` · `Calculation / generation` · `Strategy` · `Start asset` · `Input · minor units` · [QUOTED] `Quoted output · minor units` · `Gross delta · minor units` · `Pool fees included in quote` · `Net after external costs` = `Unknown — external costs incomplete` · `Capture artifact retention` = `Unknown — references do not prove raw artifacts remain available`
- **ChainFreshnessEvidence** (`section.freshness-evidence`, hairline bovenrand): eyebrow `Historical input quality` · h3 `Captured chain age` · pill `Within recorded policy` (blue) / `Older than policy` / `Future chain time` / `Unknown chain time` / `Unmeasured legacy record` (amber) · p.muted `Age at this recorded evaluation. This observation does not measure current provider or service health.` · dl (`Observation reference`, `Evaluation elapsed`, `Declared maximum chain age`, `Policy`) · p.tiny formule-uitleg · `.freshness-sources` grid met `article.freshness-source` per capture (h4 `Capture {n}`, pill, mono id, dl `Source`/`Block number`|`Finalized slot`/`Chain timestamp`/`Captured chain age`, status-notice) · slot-notice `Passing this age policy does not qualify provider correctness, account coherence, protocol equivalence, costs or transaction execution. Dataset origin remains {origin}.`
- h3 `Route and calculation inputs` → per leg `.notice` (`{venue} · {pool}` / `{in} → {out}`) of `No route was supplied for this result.`
- h3 `Capture references` → per ref `dl.research-facts` (`Capture`, `Manifest digest`, `Snapshot`) of notice `No capture references supplied. Dataset origin remains {origin}.`
- `.notice`: `Grouping: {version} · {key} · {ms} ms window.` · `Reasons: …` · `Diagnostics: …` · `This evidence does not establish an executable or realized trading result.`
- ExportButton `Export selected trace JSON`

## Gedeelde research-onderdelen (`ResearchShared.tsx`)

| Component | Render |
|---|---|
| `ResourceStatus` | laden: `p.notice[role=status]` `Loading research records…` · fout: `.notice.error-notice` strong `Stale snapshot retained.` / `Research records unavailable.` + melding · ontvangen: `p.tiny` `Snapshot received {ISO}. Explicit refresh; this panel does not claim continuous coverage.` |
| `Pagination` | `nav.research-actions` aria-label `{label} pagination` |
| `ExportButton` | secondary knop; disabled zonder data; download `arbitrage-research-{scope}.json`; fout in `p.notice[role=alert]` |
| `EmptyResearch` | `p.notice` |
| `Exact` | `span.research-exact` (mono, tabular), `Unknown` bij null |

## Data

`GET /v1/decisions?session_id&limit=25&cursor` · `GET /v1/decisions/{observation_id}` · `GET /v1/sessions/{id}/decisions/{obs}/continuity` · `GET /v1/decision-coverage?session_id` · `GET /v1/decision-groups?session_id` · `GET /v1/sessions/{id}/export` · `GET|POST /v1/sessions/{id}/cost-assessments`. Types in `api/research.ts`, `api/costs.ts`, `api/frozenExport.ts`, `api/continuity.ts`, `api/freshness.ts`.

## Regels bij redesign

- De vier tab-labels (`Captured`, `Decisions`, `Costs`, `Exports`) zijn nieuwe strings; alle overige tekst op deze pagina is copy-bevroren en blijft byte-identiek.
- Badges blijven: `CAPTURED DATA ONLY`, `PERSISTED EVIDENCE`, `FROZEN EXPORT`, `MANUAL ASSUMPTIONS`, `HYPOTHETICAL · CANDIDATE`, origin-badges.
- Amber = "let op / onzeker / handmatig", blue = "vastgelegd". Geen groen/rood.
- Alle "Unknown"-waarden blijven expliciet zichtbaar. Nooit leeg laten of 0 tonen.
- `DecisionExplorer` blijft één keer gemount over alle drie de niet-Captured tabs — een redesign mag dat niet
  opsplitsen in drie losse componenten, want de geselecteerde sessie en de geopende cost-workspace moeten
  bewaard blijven bij tabwissel.
- Tabellen met `.research-scroll` scrollen horizontaal; op mobiel niet naar kaarten omzetten zonder de caption te behouden.
