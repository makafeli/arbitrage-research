# 02 · Overview

Bron: `apps/web/src/components/pages/OverviewPage.tsx`, `components/SessionCard.tsx`, `components/RecordedOpportunities.tsx`.

Screenshots:
- `screenshots/01-overview-desktop.png` · `screenshots/01-overview-mobile.png` (light, standaard)
- `screenshots/01-overview-dark-desktop.png` · `screenshots/01-overview-dark-mobile.png` (dark)
- `screenshots/01b-record-dialog-desktop.png` · `-mobile.png` (drawer na "Inspect")

Schil (topbar, nav, banner, pagehead, controlbar, footer): zie [00-shell.md](00-shell.md). h1 op deze pagina: `Paper trading overview`.

## Doel

Startpagina na inloggen. Eén blik: hoeveel sessies en records dit geladen scherm telt, welke sessies er zijn en in welke staat, en welke opportunity-records er zijn op de laatste API-pagina. Sessies kun je hier ook besturen (Start / Pause / Resume / Stop).

## Opbouw

```
(schil: banner, pagehead, controlbar)
div.stats                       NIEUW — grid 4 → 2 → 1, vier .fact-tegels (§A)
p.tiny                          "Counts describe this loaded page and view filter only."
.sectionhead                    h2 "Research sessions" + p "Observed states remain unchanged until a fresh service response arrives."
.cards                          grid, gap 12px — één SessionCard per sessie (gefilterd op chain)
   of Empty                     "No sessions in this view"
section.section-spacer
├─ .sectionhead                 h2 "Recorded opportunities" + p "{n} records on the latest API page. Counts are not full-history totals."  |  span.pill "CAPTURED DATA ONLY"
├─ .notice (conditioneel)       als capabilities.opportunity_capture = false
├─ RecordedTable  of  Empty     "No captured quote records on this page"
└─ p.notice (conditioneel)      "More records exist outside this page. Full-history browsing and comparison are not available in this view."
```

De "Research sessions"- en "Recorded opportunities"-blokken zijn `SessionsSection` en `RecordedSection`, geëxporteerd
uit `OverviewPage.tsx` en hergebruikt: `SessionsSection` ook op **Runs → Sessions**-tab, `RecordedSection` ook op
**Opportunities → Captured**-tab. Zelfde componenten, zelfde teksten — de `.stats`-strip hieronder bestaat
alleen op Overview.

## A. `.stats` — mini-statistieken (nieuw in v3)

Vier `.fact`-tegels, alle vier afgeleid uit props die de schil al doorgeeft (geen extra API-call):

| Label | Waarde |
|---|---|
| `Sessions in view` | `visibleSessions.length` (na chain-filter) |
| `Running` | aantal zichtbare sessies met `observed_state === 'RUNNING'` |
| `Unresolved commands` | aantal commando's met `sending` of `uncertain` waar |
| `Records on this page` | `rows.length` (opportunity-rijen op de laatste API-pagina, na chain-filter) |

Direct eronder: `p.tiny` `Counts describe this loaded page and view filter only.` — deze regel is bewust, ze
voorkomt dat de tegels als volledige-historie-totalen worden gelezen.

## SessionCard (`section.panel`, aria-label `Session {session_id}`)

```
.sectionhead
├─ h3        "{Base|Solana} · {mode}"          bijv. "Base · OBSERVE"
├─ p.mono    session_id
└─ span.pill observed_state                    STOPPED | RUNNING | PAUSED | DRAINING | …
.session-states                                 vier chips .session-status:
   "Health: {health}" · "Desired revision: {n}" · "Applied revision: {n}" · "Unresolved attempts: {n}"
p.tiny       "Last worker heartbeat: {ISO}"  of  "Last worker heartbeat: Unknown; no heartbeat reported"
p.tiny.mono  "Configuration: {configuration_digest}"
— conditionele notices (zie hieronder) —
.controls.session-actions        [Start] [Pause] [Resume] [Stop]  (+ [Retry same request])
p.tiny       uitleg-regel (zie hieronder)
```

### Notices op een kaart

| Wanneer | Class | Tekst |
|---|---|---|
| desired ≠ applied revision én geen receipt | `.notice` | `Desired and applied revisions differ. This can reflect unapplied, rejected or superseded history; a revision gap alone does not establish a pending worker acknowledgement. The service validates new requests.` |
| receipt aanwezig | `.notice[role=status]` | strong `{ACTION} · {STATUS}` · p `Command {command_id} · revision {revision}` · p: PENDING → `Accepted by the API. Awaiting worker acknowledgement; the admission fence is not yet confirmed.` / APPLIED → `Worker acknowledged at {applied_at}. Admission fence effective.` of `… See the separately refreshed observed session state.` / anders → `This command did not apply. Review the latest session before issuing another command.` |
| verzenden | `.notice[role=status]` | `Sending {ACTION}. Acceptance is not yet confirmed.` |
| fout | `.notice.error-notice[role=alert]` | strong `Delivery uncertain` / `Receipt status may be stale` / `Command rejected` · p foutmelding · (uncertain) p `The server may have accepted this request. Retry reuses the same idempotency key and exact payload.` |
| state DRAINING | `.notice` | `The admission fence has applied, but {n} previously emitted attempts remain unresolved. A stop cannot recall an emitted transaction.` |

### Knoppen

Vier vaste knoppen, altijd alle vier gerenderd. `Start` en `Resume` zijn `.primary`. Welke actief zijn hangt af van `availableActions(session)` (`api/client.ts:363`):

| observed_state | beschikbaar |
|---|---|
| STOPPED | Start (+ Stop als desired ≠ applied) |
| RUNNING | Pause, Stop |
| PAUSED | Resume, Stop |
| overig / LIVE | niets |

Extra blokkering: signed out, stale snapshot (behalve Stop), bezig met verzenden, delivery uncertain, receipt PENDING. `Retry same request` verschijnt alleen bij `uncertain`.

### Uitleg-regel onder de knoppen (één van deze)

- `LIVE sessions have no controls in this interface.`
- `Controls unavailable while signed out.`
- `Snapshot is stale. Stop remains available as a request using the last known revision; only a worker receipt can confirm it.`
- `Wait for the pending acknowledgement or resolve delivery uncertainty before another command.`
- `Controls apply only to this session. Modes and configuration cannot be changed after creation.`

### Mobiel

≤540px: `.session-actions` wordt 2-koloms grid, knoppen volle breedte; oneven laatste knop over volle breedte.

## Empty (`div.panel.space-top`)

h3 + p.muted. Twee varianten op deze pagina:
- `No sessions in this view` / `Create a session from an operator-validated configuration in Experiments. The chain filter changes only this view.`
- `No captured quote records on this page` / `This filtered response does not establish collection coverage or adapter status. Inspect sessions and decision traces for retained observations, rejection evidence and input quality.`

## RecordedTable (`div.tablewrap.space-top > table`)

Caption (sr-only): `Captured market observations. Amounts remain exact integer minor units of the named start asset.`

| Kolom | Inhoud |
|---|---|
| `Route / chain` | `span.route` = venue families met ` → ` · `span.route-sub` = `{Base|Solana} · {opportunity_id}` |
| `Evidence` | `span.pill.amber` `REJECTED · simulation failed` (bij simulation_status FAILED) of `span.pill.blue` met `evidence_label` (underscores → spaties) |
| `Input · minor units` (`.num`) | `amount_in_minor` |
| `Net · minor units` (`.num`) | `span.mobilelabel` `Net · start asset minor units` (alleen mobiel) · `span.net` = `net_after_explicit_costs_minor` of `Unknown — costs incomplete` · `span.route-sub` = `start_asset_id` |
| `Quality / provenance` | drie `route-sub` regels: `Coherent snapshot` / `Incomplete / inconsistent` · `Fresh at evaluation` / `Stale / ineligible at evaluation` · `Observed {observed_at}` |
| (sr `Details`) | `button.cellbtn` `Inspect`, aria-label `Inspect {opportunity_id}` |

Voet `.tablefoot`: `Raw minor units preserve precision. Start-asset decimals are not supplied by this API. Paper results remain hypothetical.`

Notice boven de tabel als `opportunity_capture` false: `The API process does not collect market data. Workers may independently persist captured evidence; returned records and provider status must be assessed separately.`

Mobiel ≤800px: thead weg; rij = grid-kaart; kolom "Input" verborgen; "Net" krijgt het mobilelabel; Inspect-knop rechtsonder.

## RecordedDialog — nu een drawer (`dialog.drawer`, aria-labelledby `record-title`)

Screenshot `01b-record-dialog-*`. Was een gecentreerde modal (`min(590px, 100% − 32px)`), is nu een **rechter
zijpaneel**: `dialog.drawer`, 660px breed (100% ≤760px), volle hoogte, radius 24 op de linkerrand, slide-in
180ms (uit bij reduced motion). Inhoud en teksten zijn ongewijzigd — alleen de plaatsing/vorm is anders. Zie
[00-shell.md](00-shell.md) §8 voor de drawer-primitief.

```
.dialoghead     p.eyebrow "Captured market record" · h2 {opportunity_id}  |  button "Close record detail"
span.pill.blue  evidence-tekst (zelfde als tabel)
p               "{mode} · {Base|Solana} · {start_asset_id}"
table.detailcosts   rijen: "Input · minor units" · "Quoted output · minor units" · "Net after explicit costs · minor units" (of "Unknown: costs incomplete") · per cost: kind + "Valuation: {ref}" | bedrag + "Native {n} minor units · {asset}"
p.notice (als geen costs)   "No explicit cost rows supplied. Missing cost evidence must not be interpreted as a zero network fee."
.notice "Provenance"        Observed · State · Snapshot · Provider · "Age at evaluation: {n} ms · {finality_label}" · Finality · Plan (of "No execution plan supplied") · Scenario (of "No inclusion scenario supplied")
h3 "Route and eligibility"  per leg p.tiny "{asset_in} → {asset_out} · {venue} · {pool_id}"; per eligibility check p.tiny "{check}: Pass | Not satisfied"
.notice                     reason_codes met " · " (of "No rejection reasons supplied.") · p "Simulation: {status}. This record is not a realized trading result." (of "Transaction: {id}" bij REALIZED)
p.tiny                      "Pool fees and price impact are included in the quoted output. Infrastructure costs and unallocated failures are separate."
```

Gedrag: `showModal()`, focus op sluitknop, Tab blijft op de sluitknop (single-element trap), `Escape` sluit, focus terug naar de Inspect-knop.

## Data

- Sessies: `GET /v1/sessions` → `session_id, network_id, mode, observed_state, health, desired_revision, applied_revision, outstanding_attempts, execution_authorized, last_heartbeat_at, configuration_digest`. LIVE-sessies worden weggefilterd.
- Opportunities: `GET /v1/opportunities` → zie `specs/opportunity.example.json`.
- Commando: `POST /v1/sessions/{id}/commands` met idempotency key; receipt via `GET /v1/commands/{command_id}`, elke 5 s bijgewerkt zolang PENDING.

## Regels bij redesign

- De vier `.stats`-labels blijven exact zo en tellen alleen dit geladen scherm — nooit herformuleren als "totaal" of "alle sessies".
- Badge `CAPTURED DATA ONLY` blijft.
- De vier knoppen blijven allemaal zichtbaar (ook disabled). Dat is bewust: de gebruiker ziet wat níet kan.
- Elke notice-tekst blijft. Ze beschrijven onzekerheid die de API niet kan wegnemen.
- Geen kleur voor "goed/slecht" buiten accent (blue) en warn (amber).
