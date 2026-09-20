# 07 · System

Bron: `apps/web/src/components/pages/SystemPage.tsx`, `components/AdapterSupport.tsx`, `components/CollectionHealth.tsx`, `components/CapabilitiesPanel.tsx`.

Screenshots: `screenshots/06a-system-adapters-desktop.png` · `-mobile.png` (tab "Adapters", configuratie geladen) · `screenshots/06b-system-collection-desktop.png` · `-mobile.png` (tab "Collection", sessie `session-paper` gekozen) · `screenshots/06c-system-capabilities-desktop.png` · `-mobile.png` (tab "Capabilities").

Schil: zie [00-shell.md](00-shell.md). h1: `System`.

## Doel

Wat kan deze service, en hoe gezond is het verzamelen van data? Drie blokken, nu verdeeld over drie tabs
(`div.tabs[role=tablist][aria-label="System sections"]`, echte `TabPanel`s):
1. **Adapters** (standaard) — adapter support catalog: per configuratie en per chain wat is geïmplementeerd, welk registry is geladen.
2. **Collection** — collection attempt health per sessie: batch-uitkomsten van de worker en een lijst van pogingen met een "wat te checken"-advies.
3. **Capabilities** — feiten uit `/v1/capabilities` + een nieuwe "Capability gates"-tabel.

## Opbouw

```
(schil)
div.tabs[role=tablist][aria-label="System sections"]   Adapters (standaard) · Collection · Capabilities
TabPanel tab="sys-adapters"       section.adapter-support.research-workspace.section-spacer   AdapterSupport  (§A)
TabPanel tab="sys-collection"     section.research-workspace.section-spacer                   CollectionHealth  (§B)
TabPanel tab="sys-capabilities"   CapabilitiesPanel  → "Capabilities and data quality" (§C) + "Capability gates" (§C2, nieuw)
```

Alle drie zijn echte `TabPanel`s (`role=tabpanel`, elk met eigen `hidden`), net als op Runs — geen handmatige
`hidden`-divs. Elke tab is onafhankelijk: geen gedeelde state die bij tabwissel bewaard moet blijven (in
tegenstelling tot Opportunities en Runs).

## A. AdapterSupport (aria-labelledby `adapter-support-title`)

```
.sectionhead
├─ p.eyebrow   "Code and immutable scope"
├─ h2          "Adapter support catalog"
├─ p           "Inspect what this service implements and which loaded registry each configuration authorizes."
└─ button      "Refresh adapter catalog"  (disabled zonder capability of tijdens laden)
p.notice   [capability adapter_support uit] "Adapter support catalog is unavailable in this API version. Configuration counts do not establish adapter coverage."
p.notice[role=status]   "Loading adapter support catalog…"
.notice.error-notice[role=alert]   strong "Previous adapter catalog retained." | "Adapter catalog unavailable."  · p "The service request failed or returned an unsupported catalog. This is a transport or contract error, not a statement that registries are absent."
p.tiny     "Catalog received {ISO}. This snapshot describes the API process’s locally loaded registries; worker deployment state and provider health remain separate."
p.notice   "Local structural authorization does not qualify current program code, providers, quotes or execution. No chain is marked ready to trade by this catalog."
p.notice   [geen configs] "No immutable configurations are registered in this service."
per configuratie: section.panel.support-config  (aria-label "Adapter configuration {digest}")
├─ h3 "Immutable configuration"
├─ p.tiny.mono  digest
└─ .support-networks  (grid 2 kolommen; ≤700px 1)  → per chain article.support-card  (gefilterd op chain-filter)
```

**NetworkCard** (`article.support-card`, aria-label `Base adapter support` / `Solana adapter support`):

```
.sectionhead   h4 "Base" | "Solana" · p.tiny {venue_family}  |  span.pill.blue "Configured enabled" | span.pill.amber "Configured disabled"
p strong       "Registry not loaded" | "Locally authorized registry" | "Loaded registry blocked"
p.tiny (per reason)  "No registry for this chain was loaded by this API process." | "This immutable configuration disables this chain." | "No loaded registry matches the digest declared by this configuration." | "The selected registry does not satisfy this configuration’s pool or asset scope."
dl.support-capabilities   (label links, waarde rechts)
   Read / decode code → Implemented · Research quote math → Implemented · Qualified quote → Unqualified · Transaction build → Unavailable · Full transaction simulation → Unavailable · Submission → Unavailable
p.notice       "Protocol equivalence and token behavior remain unqualified. Full transaction simulation has not run."
dl.research-facts   Declared asset identities · Declared pool identities · Loaded authorized pool relations · Chain age assumption ("{ms} ms maximum" | "Not configured")
p.tiny         "These counts describe configuration scope and local registry structure. They do not measure provider coverage, available markets or executable routes."
details.support-scope   summary "Inspect immutable identities"
   p.tiny.mono "Declared registry: {digest|None declared}" · p.tiny.mono "Selected loaded registry: {digest|No matching selection}"
   [niet uitgeklapt] p.notice "The declared scope exceeds this response’s identity expansion limit. Counts retain the full scope; no partial identity list is presented."
   [uitgeklapt] p.tiny "Declared assets" + ul.mono (of "None declared") · p.tiny "Declared pools" + ul.mono (of "None declared")
   per pool: .notice  p.mono pool_id · p.tiny "{asset} ↔ {asset}" · per program p.tiny.mono "{role}: {address}" / "Code digest: {digest|Not supplied for this program role}"
```

De zes capability-regels zijn hard-coded (geen API-data). Ze zeggen bewust "Unavailable" voor alles wat met uitvoeren te maken heeft.

## B. CollectionHealth (aria-labelledby `collection-health-title`)

```
.sectionhead   h2 "Collection attempt health" + p "Durable batch outcomes from the worker, including failures before any decision exists."  |  span.pill.blue "RECORDED ATTEMPTS"
[capability collection_telemetry uit]  p.notice "Collection telemetry is unavailable in this API version. Zero stored decision errors cannot establish successful acquisition."
.panel.research-toolbar   select#collection-session "Collection health session" ("Choose a session" + "{id} · {network} · {mode}") · button "Refresh collection health"
[geen sessie]  p.notice "Select a session to inspect recorded collection attempts."
[sessie gekozen]
├─ p.tiny   "Session scope: {id}. Counts and live pages are separate received snapshots. Changing the chain filter does not change this selected scope."
├─ ResourceStatus (coverage)
├─ section.panel.space-top  (aria-label "Recorded collection attempt counts")
│  ├─ h3 "Batch outcomes · all recorded attempts"
│  ├─ .research-metrics  12 × .fact:
│  │    Recorded batch attempts · Readiness batches · Research batches · No terminal outcome ·
│  │    Readiness completed · Batches with recorded decisions · Acquisition failed · Evaluation failed ·
│  │    Deadline exceeded · Suppressed by control fence · Worker cancelled · Decision rows recorded
│  ├─ p.notice "Scheduled collection completeness: UNKNOWN. The denominator is recorded batch attempts. It excludes work that was never recorded and does not measure all scheduled work, market activity, opportunities or fills."
│  └─ p.tiny "Recorded attempt window: {start|Unknown} → {end|Unknown}. A batch without a terminal outcome may still be running or may have been interrupted; it is not classified as a failure."
├─ p.notice "Attempt origin is not retained. These batches may include synthetic fixtures or real provider work; attempt counts alone cannot qualify market data provenance. Inspect linked decisions for their retained dataset origin."
├─ .sectionhead  h3 "Live collection attempt page" + p "Failures, deliberate control suppression and unresolved batches retain separate outcomes."
├─ ResourceStatus (attempts)
├─ p.notice (leeg) "No collection attempts were returned for this page. Acquisition reliability and scheduled completeness remain unknown."
├─ .research-scroll[tabindex=0] (aria-label "Collection attempt table scroll area") > table.research-table  caption "Recorded batch attempts and suggested checks"
└─ Pagination "collection attempts"
```

Tabel-kolommen:

| Kolom | Inhoud |
|---|---|
| `Attempt / purpose` | strong.mono attempt_id · route-sub `{purpose} · {started_at}` · route-sub `Finished: {finished_at|No terminal timestamp}` |
| `Outcome / scope` | pill (amber bij IN_PROGRESS, ACQUISITION_FAILED, EVALUATION_FAILED, DEADLINE_EXCEEDED): `NO TERMINAL OUTCOME` of outcome met spaties · route-sub `Generation {n} · worker epoch {n}` · route-sub reason of `No reason code` |
| `Recorded evidence` | `{n} captured pools` · route-sub `{n} decision rows` · route-sub `{n} ms elapsed` of `Elapsed time unavailable` |
| `Suggested check` | adviestekst (zie hieronder) |

Adviesteksten (`guidance` in `CollectionHealth.tsx`) per reason:
- PROVIDER_UNAVAILABLE: `Check the configured provider availability and credentials in the service environment, then inspect the next recorded attempt.`
- INPUT_VALIDATION_FAILED: `Review the pool registry and the frozen configuration for unsupported or inconsistent inputs.`
- CAPTURE_STORAGE_UNAVAILABLE: `Check the capture store and database availability before starting another research attempt.`
- RESOURCE_LIMIT: `Review the configured pool count and capture bounds. Reduce the workload before retrying.`
- ACQUISITION_UNAVAILABLE: `Inspect provider availability and the configured pool inputs before retrying collection.`
- ACQUISITION_DEADLINE: `Check provider latency and request limits. This attempt exceeded its acquisition budget.`
- EVALUATION_REJECTED: `Review supported route inputs and the frozen strategy configuration.`
- EVALUATION_DEADLINE: `Review evaluation workload and resource pressure. This attempt exceeded its evaluation budget.`
- GENERATION_FENCED: `Compare this generation with the current session command. A stopped or superseded generation cannot admit decisions.`
- WORKER_SHUTDOWN: `Check the intended worker lifecycle and restart status. Shutdown does not establish a completed research batch.`
- TASK_FAILED: `Inspect the worker health and restricted service diagnostics, then verify a later attempt completes.`
- zonder reason, READINESS_COMPLETED: `Readiness capture completed. This batch does not establish research decisions or continuous market coverage.`
- zonder reason, anders: `Decisions were durably admitted for this batch. Their quote, rejection and data quality evidence remains separate from executable results.`

## C. Capabilities and data quality (`section.panel`, in de Capabilities-tab)

```
h2 "Capabilities and data quality"
.facts  (grid 2 kolommen; ≤540px 1)  4 × .fact:
├─ "API process market collection"   → "Available" | "Unavailable"   (capabilities.market_data)
├─ "API process opportunity capture" → "Available" | "Unavailable"   (capabilities.opportunity_capture)
├─ "Coverage gaps"                   → "Unknown"  + p.tiny "Stored observation bounds are available in the decision explorer. Continuous collection completeness remains unknown."
└─ "Research modes"                  → "{modes met ', '}" | "None"
.notice  "Worker heartbeat and data age are reported per record or session. HTTP connectivity cannot establish their freshness. No live controls are offered in this research interface."
```

## C2. Capability gates (`section.panel.space-top`, nieuw in v3, direct onder §C in dezelfde Capabilities-tab)

Vlaggen zoals de API ze teruggeeft op `/v1/capabilities`, als tabel in plaats van los proza — zichtbaar naast de
vier feiten van §C, niet ter vervanging ervan.

```
h3       "Capability gates"
p.tiny   "Modes: {capabilities.modes met ', '}"
div.research-scroll[tabIndex=0][aria-label="Capability gates table scroll area"]
└─ table.research-table
   ├─ caption   "Feature flags reported by /v1/capabilities"
   ├─ thead     "Capability" · "Status"
   └─ tbody  per gate (10 vaste sleutels, vaste volgorde)
      ├─ td.mono   sleutelnaam, exact zoals de API ze levert
      └─ td        span.pill  "Available" (waarde `=== true`) of "Unavailable"
```

De tien sleutels, in deze volgorde: `live_execution`, `market_data`, `opportunity_capture`, `decision_history`,
`paper_ledger`, `paper_run_creation`, `collection_telemetry`, `session_export`, `cost_assessments`,
`adapter_support`.

## Data

`GET /v1/adapter-support` (types `api/support.ts`) · `GET /v1/sessions/{id}/collection-coverage` · `GET /v1/sessions/{id}/collection-attempts` (types `api/collection.ts`) · `capabilities` uit de schil-snapshot (ook de bron voor de nieuwe gates-tabel — geen extra request).

## Mobiel

`.support-networks` 1 kolom ≤700px. `.research-metrics` 2 kolommen ≤900px, compacter ≤540px. Attempt-tabel scrollt horizontaal (scroll-area is focusbaar met `tabindex=0`). `.facts` 1 kolom ≤540px. Gates-tabel gebruikt dezelfde `.research-scroll` als de attempt-tabel.

## Regels bij redesign

- De drie tab-labels (`Adapters`, `Collection`, `Capabilities`) zijn nieuwe strings; alle overige tekst blijft copy-bevroren.
- Badge `RECORDED ATTEMPTS` blijft. `Configured enabled/disabled` blijft een pill.
- De zes "Implemented / Unqualified / Unavailable"-regels blijven letterlijk. Ze zijn de eerlijkheidsverklaring van de service.
- "Coverage gaps: Unknown" blijft een feit met waarde `Unknown`, geen lege tegel.
- Twaalf metrics mogen gegroepeerd worden (bijv. start / klaar / mislukt) maar alle twaalf labels blijven.
- De "Capability gates"-tabel toont de tien sleutels in vaste volgorde en als ruwe `sleutelnaam` (mono, geen vertaling/spaties toevoegen) — dit is bewust een letterlijke spiegel van de API-response, geen herschreven featurelijst.
