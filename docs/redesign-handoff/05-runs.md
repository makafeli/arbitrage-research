# 05 · Runs

Bron: `apps/web/src/components/pages/RunsPage.tsx`, `components/PaperWorkspace.tsx` (`PaperWorkspace`, `PaperCreation`), `FrozenSessionExport.tsx`, `components/SessionCard.tsx` (`SessionsSection`).

Screenshots: `screenshots/04a-runs-sessions-desktop.png` · `-mobile.png` (tab "Sessions", sessie `session-paper` gekozen) · `screenshots/04b-runs-ledger-desktop.png` · `-mobile.png` (tab "Ledger", run `run-original` geopend) · `screenshots/04c-runs-journal-desktop.png` · `-mobile.png` (tab "Journal") · `screenshots/04d-runs-reservations-desktop.png` · `-mobile.png` (tab "Reservations").

Schil: zie [00-shell.md](00-shell.md). h1: `Runs`.

## Doel

Sessies besturen (zelfde kaarten als Overview) én de **paper accounting** bekijken: hypothetische saldi, journaal en reserveringen per paper-run. Ook: een nieuwe hypothetische run starten met vaste beginbedragen. Nu verdeeld over vier tabs — vóór v3 stonden sessiekaarten en accounting onder elkaar op één lange pagina.

## Opbouw

```
(schil)
div.tabs[role=tablist][aria-label="Runs sections"]   Sessions (standaard) · Ledger · Journal · Reservations
section.section-spacer.research-workspace   PaperWorkspace  (aria-labelledby paper-workspace-title, blijft altijd gemount)
├─ .sectionhead      h2 "Paper accounting workspace" + p "Durable hypothetical inventory, reservations and journal evidence. Balances are not returns."  |  span.pill.paper "HYPOTHETICAL"
├─ [capability paper_ledger uit]  p.notice "Paper accounting is unavailable in this API version. No balances or performance estimates have been invented."
├─ .panel.research-toolbar   select#paper-session "Paper session" · button "Refresh paper records"
├─ [geen sessie]  p.notice "Select a PAPER session to inspect retained runs and hypothetical accounting."
└─ [sessie gekozen]
   ├─ TabPanel#panel-runs-sessions       (§A — sessiekaarten, export, aanmaken, runs-tabel)
   ├─ TabPanel#panel-runs-ledger         (§C — geselecteerde run: saldi)
   ├─ TabPanel#panel-runs-journal        (§D — journaal)
   └─ TabPanel#panel-runs-reservations   (§E — reserveringen)
```

Anders dan op Opportunities zijn dit hier **echte** `TabPanel`s (`role=tabpanel`, elk met eigen `hidden`) — geen
handmatige `hidden`-divs. `PaperWorkspace` blijft wel één keer gemount zodat de gekozen sessie en het gekozen
`runId` bewaard blijven bij tabwissel. De toolbar (sessie-select + "Refresh paper records") staat boven alle vier
de panelen en is dus altijd zichtbaar, ongeacht welke tab actief is.

Select `Paper session`: `Choose a PAPER session` + alleen sessies met `mode === 'PAPER'`: `{session_id} · {network_id} · {observed_state}`. Disabled zolang een aanmaak loopt.

## A. Sessions-tab (`TabPanel tab="runs-sessions"`)

```
SessionsSection            "Research sessions" — identiek aan 02-overview.md §SessionCard, dezelfde component
p.tiny                     "Session scope: {id}. The chain selector filters choices only. Existing runs and their original balances remain retained when a new run is created."
FrozenSessionExport        (zie 03-opportunities.md §B — identieke component en teksten)
p.notice[role=status]      [locked] "Paper creation scope is locked while delivery is unresolved. Retries use the original session, amounts and idempotency key."
PaperCreation               section.panel  (§B)
.sectionhead                h3 "Retained paper runs" + p "Up to 25 runs per page. A new run does not reset a previous ledger."
ResourceStatus (runs)
.research-scroll > table.research-table   (§B2)
Pagination "paper runs"
```

De sessiekaarten staan dus **binnen** de Sessions-tab van deze pagina, niet erboven — ze zijn dezelfde
`SessionsSection`-component als op Overview, doorgegeven via de `sessionsSection`-prop van `RunsPage`.

## B. PaperCreation (`section.panel.space-top`, aria-labelledby `paper-create-title`)

```
h3          "Initialize a new hypothetical run"
p.muted     "A new ledger uses this session's frozen configuration. Existing runs and initial balances remain unchanged."
p.notice    één van (of niets):
            "Creation unavailable: the API has not advertised paper run creation."
            "Creation unavailable: the session's exact PAPER configuration is not enabled in the registry."
            "Creation unavailable: the validated configuration must expose token principal and a separate native fee asset."
            "Creation requires a STOPPED PAPER session. The service atomically checks pending commands before initializing a run."
form.connected-form > fieldset
├─ select#paper-principal-asset   "Validated principal asset"  → "Choose a validated token" + tokens uit configuration.paper_assets voor dit netwerk
├─ input#paper-principal          "Initial token principal · exact minor units"   (numeric, pattern [0-9]+, max 78)
├─ input#paper-native             "Initial native fee reserve · exact minor units" (numeric, max 78)
│  p.tiny  "Native asset: {identity|Unavailable}. Enter an explicit amount; zero means no native fee inventory, not an unknown or free network fee."
├─ .notice   strong "Review immutable run settings"
│     p "Session: {session_id} · {network_id} · PAPER"
│     p.mono "Configuration: {digest}"
│     p "Principal: {token|Choose an asset} · {principal|Not entered} minor units"
│     p "Separate native inventory: {native|Not entered} minor units"
└─ label.checkbox-label  "I reviewed these hypothetical amounts and the frozen configuration."
p.notice.error-notice[role=alert]   (fout)
button.primary   "Create hypothetical paper run" | "Creating paper run…" | "Retry same paper creation" | "Paper run created"
.notice[role=status]  [na succes] "Created {run_id} with immutable initial balances. HYPOTHETICAL only; no market execution was authorized."  + button "Prepare another new run"
p.tiny      "Amounts use canonical decimal integer strings. No wallet keys, token approvals, settlement or ledger reset controls are provided."
```

Geldig = toegestaan + geregistreerd + sessie STOPPED/PAPER/niet execution_authorized + token gekozen + principal > 0 + native ingevuld (0 mag) + checkbox. Elke veldwijziging zet de checkbox uit. Onzekere levering: `Creation delivery is uncertain. Retry the same request to reconcile its durable result.`

## B2. Tabel "Retained paper runs" (onderaan de Sessions-tab)

Leeg: `No paper runs were returned for this page. No virtual funds have been initialized for this selection.`

`table.research-table`, caption `Persisted hypothetical paper runs`:

| Kolom | Inhoud |
|---|---|
| `Run / frozen configuration` | strong run_id · route-sub.mono digest · route-sub created_at · `span.pill.paper` `HYPOTHETICAL` |
| `Revision` | Exact |
| `Outstanding reservations` | getal |
| `Inspect` | button `Inspect run` (aria-label `Inspect paper run {id}`, `aria-pressed` als gekozen) — **schakelt de pagina naar de Ledger-tab** |

## C. Ledger-tab (`TabPanel tab="runs-ledger"`) — Run-detail (`section.space-top`, aria-labelledby `paper-run-detail-title`)

Leeg zolang geen run gekozen is (geen placeholdertekst — de sectie rendert simpelweg niet totdat "Inspect run" is aangeklikt op de Sessions-tab).

```
h3#paper-run-detail-title   "Selected paper run: {run_id}"
ResourceStatus (run)
.panel.space-top
├─ .sectionhead   h3 "Hypothetical balances · exact asset minor units" + p "Revision {n}. Each asset is separate; no conversion, decimals or portfolio value are inferred."  |  ExportButton "Export selected run JSON"
├─ p.tiny.mono    "Frozen configuration: {digest}"
├─ .research-scroll > table  caption "Current free, reserved and total hypothetical inventory"
│     kolommen: "Asset / kind" (identity + route-sub "NATIVE FEE ASSET" | "TOKEN PRINCIPAL ASSET") · "Free" · "Reserved" · "Total"
├─ details  summary "Immutable initial balances"  → per asset p.tiny "{kind} · {identity}: {amount}"
└─ p.notice  "HYPOTHETICAL accounting only. These balances do not establish full transaction simulation, actual fills or realized performance. Run comparisons are unavailable without comparable coverage, explicit valuation and simulation evidence."
```

## D. Journal-tab (`TabPanel tab="runs-journal"`)

Leeg zolang geen run gekozen is.

```
.sectionhead   h3 "Paper journal" + p "Stored commands and double-entry postings, in their original asset units."  |  ExportButton "Export journal page JSON"
ResourceStatus (journal)
p.notice (leeg) "No journal entries were returned for this page."
per event: section.panel.space-top  (aria-label "Journal event {seq}")
   .sectionhead  h3 "#{sequence} · {command.kind}" · p "{recorded_at} · {event_id}" · p.mono "Command: {command_id}"  |  span.pill.paper "HYPOTHETICAL"
   .research-scroll > table  caption "Postings for journal event {seq}"   kolommen: "Asset" (identity + route-sub kind) · "Account" · "Side" · "Minor units"
Pagination "journal entries"
```

## E. Reservations-tab (`TabPanel tab="runs-reservations"`)

Leeg zolang geen run gekozen is.

```
h3 "Reservation history"
p.tiny  "Amounts below are original reservation requests. The current balances above are authoritative for inventory still reserved."
ResourceStatus (reservations)
p.notice (leeg) "No reservation records were returned for this page."
.research-scroll > table  caption "Original reservation requests and reconciliation states"
   kolommen: "Attempt / asset" (attempt_id + route-sub principal_asset) · "Principal requested" · "Original native fee budget" · "State" (pill; amber bij UNKNOWN)
Pagination "reservations"
p.tiny  "Balances, journal and reservations are separate received snapshots. Page exports contain only the selected bounded data, not a complete ledger audit. Journal export includes command kind and postings; original command inputs are omitted."
```

## Data

`GET /v1/sessions/{id}/paper-runs` · `POST /v1/sessions/{id}/paper-runs` (body `{initial_balances:[{asset,amount},{asset,amount}]}`, idempotency key) · `GET /v1/paper-runs/{run}` · `GET /v1/paper-runs/{run}/journal` · `GET /v1/paper-runs/{run}/reservations`. Types in `api/research.ts` (`PaperRunRecord`, `InitialBalance`, journal, reservation).

## Mobiel

Sessiekaarten: zie Overview. Toolbar 1 kolom (≤540px). Tabellen scrollen horizontaal. Formulier-knop volle breedte.

## Regels bij redesign

- De vier tab-labels (`Sessions`, `Ledger`, `Journal`, `Reservations`) zijn nieuwe strings; alle overige tekst blijft copy-bevroren.
- `HYPOTHETICAL` staat drie keer op deze pagina (kop, run-rij, journaal-event). Dat is bewust. Minimaal de kop-badge én de run-rij blijven.
- Saldi nooit omrekenen of optellen tussen assets. Geen "portfolio value".
- De review-notice + checkbox bij het aanmaken blijven.
- `PaperWorkspace` blijft één keer gemount over alle vier de tabs — het gekozen `runId` en de gekozen sessie moeten bewaard blijven bij tabwissel, net als bij `DecisionExplorer` op Opportunities.
- "Inspect run" op de Sessions-tab springt naar de Ledger-tab; dat gedrag blijft.
