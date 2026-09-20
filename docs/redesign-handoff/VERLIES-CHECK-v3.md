# Verlies-check · wat de app heeft en v3 niet

Datum: 20 sep 2026. Aanvulling op [VERSCHIL-RAPPORT-v3.md](VERSCHIL-RAPPORT-v3.md). Bron: de handoff-docs 00–09 (huidige app) tegenover `docs/arbitrage-mobbin-v3/assets/app.js` (prototype).

## De korte versie

Het prototype is een **ontwerp van de jas**, niet van de inhoud. Het laat elk scherm zien met één of twee voorbeeldrecords. Veel echte onderdelen van de app zijn in v3 óf weggelaten, óf vervangen door een tekstblokje dat zegt "in productie komt hier X".

Regel voor het bouwen: **v3 = jas, app = inhoud.** Elk onderdeel hieronder blijft. Het krijgt alleen de v3-stijl en een plek in de v3-indeling (tab, lade, paneel). Er verdwijnt niets.

Legenda:
- ✅ zit in v3, zelfde functie
- 🟡 zit in v3, maar versimpeld of als placeholder-tekst → app-versie gebruiken
- ❌ ontbreekt in v3 → app-versie in de v3-jas zetten

Telling: **✅ 23 · 🟡 22 · ❌ 38** (83 onderdelen). Bijna de helft van de app staat niet in het prototype. Dat is normaal voor een ontwerp; het is geen reden om iets te schrappen.

---

## 00 · Schil

| Onderdeel in de app | v3 | Plek in v3 |
|---|---|---|
| Skip-link `Skip to content` | ❌ | Bovenaan, alleen zichtbaar bij focus |
| Screenreader-aankondigingen (aria-live) | ✅ (`#announcer`) | Zelfde; app-teksten overnemen |
| Paper / Real schakelaar (`Trading mode`) | ❌ (Real is een pagina) | Segment-pill in de header |
| `Sign out` | ❌ | Account-menu |
| `Change password` + formulier | 🟡 (dialoog met uitgeschakelde velden) | Account-menu → lade met het echte formulier |
| Notice `#unresolved-mode-note` + knoppen disabled bij open verzoek | ❌ | Onder de safety strip |
| Fout-notice `Connection degraded…` + knop `Retry connection` | 🟡 (tekst als preview, geen knop) | Onder de safety strip |
| Laad-paneel `Loading service records…` | 🟡 (preview) | Zelfde plek |
| Controlbar: `API CONNECTED` / `STALE / LAST KNOWN` + snapshot-tijd + aantal sessies | 🟡 (safety strip) | Safety strip |
| Chips per verstuurd commando (`{id}: {ACTION} · {STATUS}`) | ❌ | In de safety strip, onder de tekst |
| `Stop all loaded sessions ({n})` + `Refresh` | ✅ (met bevestigingsdialoog) | Safety strip rechts |
| Chain-filter + scope-note | ✅ | Pagehead |
| Banner `PAPER TRADING` + tekst | 🟡 (andere tekst) | Safety strip, app-tekst |
| Footer-teksten | 🟡 (slogans) | Footer, app-tekst |
| Thema-knop | ✅ | Header |

## 01 · Sign in

| Onderdeel | v3 | Plek |
|---|---|---|
| Sign-in formulier | 🟡 (velden disabled) | Zelfde kaart, echt formulier |
| Hulpknop + help-notice | ✅ | Zelfde |
| Variant `Set your password` (activatie via `#activate=`) | ❌ | Zelfde kaart, tweede stand |
| Fout- en status-notices, `Checking session…` / `Please wait…` | ❌ | Boven het formulier / in de knop |
| Voetregel `Private account access…` | ✅ | Zelfde |

## 02 · Overview

| Onderdeel | v3 | Plek |
|---|---|---|
| Sessiekaart: chain · mode, id, state-badge, health / revisies / attempts, hartslag, digest | ✅ | Sessiepaneel |
| Vier knoppen Start / Pause / Resume / Stop, altijd alle vier zichtbaar | 🟡 (in de sessie-lade, niet op de kaart) | Op de kaart **én** in de lade mag; minimaal één plek met alle vier |
| Receipt-notices (PENDING / APPLIED / rejected), revision-gap-notice, DRAINING-notice | ✅ | Kaart / lade |
| `Delivery uncertain` + `Retry same request` | ❌ | Kaart / lade |
| Uitleg-regel onder de knoppen (5 varianten) | ❌ | Onder de knoppen |
| Blokkering bij stale snapshot / signed out / pending | ❌ | Gedrag, geen UI |
| Tabel `Recorded opportunities` (route, evidence, input, net, quality/provenance, Inspect) | 🟡 (andere kolommen) | Tabel "latest evidence", app-kolommen |
| Badge `CAPTURED DATA ONLY` | ❌ | Sectiekop |
| Notices: capture uit / meer records buiten deze pagina | ❌ | Boven en onder de tabel |
| Record-dialoog (kosten-tabel, provenance, route & eligibility, reasons) | ✅ (lade met tabs Summary / Route & costs / Provenance) | Lade |
| Lege staten | ✅ | Zelfde |

## 03 · Opportunities

| Onderdeel | v3 | Plek (tab) |
|---|---|---|
| Select `Decision session` | ❌ (vaste tekst) | Toolbar boven de tabs |
| Select `Origin on this page` | ❌ (v3 heeft een `Evidence state`-filter) | Toolbar; beide mogen |
| `Refresh evidence` | ✅ | Toolbar |
| Coverage-paneel, 8 feiten (`Raw observations` … `Reconciled transactions`) + notices | ❌ | Tab Decisions, bovenaan |
| Tabel `Raw decision observations` + `Inspect evidence` + `Assess hypothetical costs` | ✅ | Tab Decisions |
| Paginering (echt, met cursor) | 🟡 (statische knoppen) | Onder elke tabel |
| `Export observation page JSON` | ❌ | Sectiekop tab Decisions |
| Tabel `Grouped observation windows` | ❌ | Tab Decisions, onder de observaties |
| Frozen export: `Prepare` / `Download JSON` / `Download CSV` | ✅ | Tab Exports |
| Frozen export: digest-verificatie, zes tellingen, notices, dependency-details | 🟡 | Tab Exports |
| Capture audit: request downloaden, lokaal rapport importeren, gebonden rapport downloaden | ❌ (v3: lijstje "Not performed") | Tab Exports, onder de export |
| Kosten: scenario-id, versie, assumption reference | ❌ | Tab Costs |
| Kosten: 7 kostensoorten (v3 heeft er 6; `Account setup` ontbreekt) + "Missing / Known" per soort | 🟡 | Tab Costs |
| Kosten: native-valuatie-ratio, `Valuation as of`, max leeftijd | ❌ | Tab Costs |
| Kosten: `Funding assumption`, `Shared operating overhead` + allocatie | ❌ | Tab Costs |
| Kosten: review-checkbox | ✅ | Tab Costs |
| Kosten: opslaan via API (idempotent, `Retry identical…`, `Cost assessment saved`) | ❌ (v3 slaat alleen lokaal op) | Tab Costs |
| Kosten: `AssessmentResult` (feiten, `Net remains unknown`, componenten-tabel, referenties) | 🟡 | Tab Costs |
| Kosten: `Retained cost assessments` (historie, refresh, export, paginering) | ❌ | Tab Costs, onderaan |
| Decision-lade: `Source continuity` | ✅ | Lade, tab Source continuity |
| Decision-lade: feitenlijst (18 regels) | 🟡 | Lade, tab Decision |
| Decision-lade: `Captured chain age` (ChainFreshnessEvidence, per capture) | ❌ | Lade, tab Decision |
| Decision-lade: route-inputs, capture references, grouping/reasons/diagnostics | 🟡 | Lade, tab Decision |
| Decision-lade: `Export selected trace JSON`, `Refresh selected trace` | ❌ | Lade, onderaan |

## 04 · Experiments

| Onderdeel | v3 | Plek |
|---|---|---|
| Select `Validated configuration` | ✅ | Stap 01 |
| Select `Session network` (alleen enabled networks) | ❌ (readonly tekst) | Stap 01 |
| Input `Experiment reference` | ✅ | Stap 02 |
| Notice `Review immutable settings` + checkbox | ✅ | Aside |
| Notice "geen configuraties" / "geen netwerken" | ❌ | Boven het formulier |
| Knop met 4 teksten + pending/uncertain-gedrag | ❌ | Stap 03 |
| Succes-notice `Created {id} · {state}. No start command was sent.` | 🟡 | Stap 03 |

## 05 · Runs

| Onderdeel | v3 | Plek (tab) |
|---|---|---|
| Sessiekaarten | ✅ | Tab Sessions |
| Select `Paper session` | ❌ (vaste tekst) | Toolbar |
| Frozen export ook hier | ❌ (alleen onder Opportunities) | Tab Ledger, zelfde component |
| `Initialize a new hypothetical run` (formulier: asset, principal, native reserve, review, checkbox, 4 knopteksten) | ❌ (v3: lijstje met wat het formulier "moet hebben") | Tab Ledger, boven de runs |
| Tabel `Retained paper runs` + `Inspect run` | ✅ | Tab Ledger |
| Saldi-tabel free / reserved / total + `Immutable initial balances` | ✅ / 🟡 | Tab Ledger |
| Journaal per event met postings-tabel | 🟡 (tijdlijn) | Tab Journal |
| Reserveringen-tabel | ✅ | Tab Reservations |
| `Export selected run JSON`, `Export journal page JSON` | 🟡 (`Export sample`) | Sectiekoppen |
| Paginering runs / journaal / reserveringen | ❌ | Onder elke tabel |
| Badge `HYPOTHETICAL` (3×) | 🟡 (zinsopmaak) | Zelfde plekken |

## 06 · Strategies

| Onderdeel | v3 | Plek |
|---|---|---|
| Tabel met mode, netwerken, digest, strategies | ✅ | Tabel |
| Badge `IMMUTABLE` | 🟡 | Rij |
| Lege staat | ❌ | Paneel |

## 07 · System

| Onderdeel | v3 | Plek (tab) |
|---|---|---|
| Adapter support catalog: per configuratie, per chain een kaart met registry-status, 6 capability-regels, tellingen, `Inspect immutable identities` | ❌ (v3: 3 rijen "Not checked / Offline fixture") | Tab Adapters — **grootste gat** |
| Collection health: sessie-select, 12 metrics, attempts-tabel met `Suggested check`, paginering | ❌ (v3: tekstblok + lijstje) | Tab Collection — **tweede grootste gat** |
| Capabilities: 4 feiten + notice | 🟡 | Tab Capabilities |
| Capability-gate-tabel (capability → sectie → status) | **nieuw in v3, heeft wél data** | Tab Capabilities, boven de 4 feiten |

## 08 · Real trading

| Onderdeel | v3 | Plek |
|---|---|---|
| Twee zinnen (handoff-grens) | 🟡 (andere tekst) | Paneel, app-tekst |
| Disabled knop `Start real trading` | ❌ | Paneel |
| Schil verbergt paginalinks in Real-modus | ❌ | Gedrag |

## 09 · Owner overview

| Onderdeel | v3 | Plek |
|---|---|---|
| Hele pagina (5 secties, NL/EN, gezondheids-pill, wat-als, route naar live) | ❌ | Eigen navitem; op mobiel onder More |

---

## Andersom: wat v3 heeft en de app niet

**Heeft data, kan gebouwd worden**
- Stat `Need attention` (sessies met DEGRADED of `outstanding_attempts > 0`).
- Capability-gate-tabel op System (uit `/v1/capabilities`).
- `Evidence state`-filter op de geladen observatiepagina.
- Link `New experiment` op Overview.
- Sessie-lade (zelfde data als de kaart).
- Tabs, lades, List/Cards-schakelaar (andere vorm, zelfde data). List/Cards zou ik toch overslaan: dubbel onderhoud.
- Decision mini-stats (tellen op de geladen pagina; moet erbij zeggen "loaded page").

**Geen data → weglaten of letterlijk `Unknown`**
- Stat `Net after costs` (altijd Unknown).
- Paneel `What the evidence supports` op paginaniveau. Per record is het wél af te leiden (capture refs, simulation status, costs, eligibility) → kan in de record-lade.
- Zoeken (Cmd/Ctrl+K), breadcrumbs, preview-chip, footer-slogans, System "health cards" per systeem.

**Nooit bouwen (demo-knoppen)**
- Simulate acknowledgement / ready / reconcile, `Explore preview states`, `Re-read fixture`, `Save local scenario`, `View sign-in design`, synthetische fixtures.

---

## Wat dit betekent voor het bouwen

1. De ❌-lijst is geen "extra werk". De componenten bestaan al (`DecisionExplorer.tsx`, `CostAssessmentWorkspace.tsx`, `FrozenSessionExport.tsx`, `CaptureAuditPanel.tsx`, `PaperWorkspace.tsx`, `AdapterSupport.tsx`, `CollectionHealth.tsx`, `OwnerOverview.tsx`, …). Ze krijgen nieuwe CSS en een tab of lade om in te staan.
2. Het echte werk zit in de jas: header, capsule, tokens, tabs, lades, safety strip.
3. Tests blijven de bewaker. Elk ❌-onderdeel heeft nu al tests; als een onderdeel per ongeluk wegvalt, wordt het rood.
