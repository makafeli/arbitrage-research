# Redesign handoff — `apps/web`

Eén bestand per pagina. Elk bestand geeft: doel, screenshots, opbouw (van boven naar beneden), alle teksten, data, states, interacties, toegankelijkheid, mobiel gedrag en de regels die bij een redesign vast staan.

Datum: 2026-09-20. Bron: `feat/ARB-070-v3-monochrome-shell` na wave 1 (schil, `9039277`) en wave 2 (paginalayouts met tabs, drawers, split sign-in, `c25d3b1`) van ARB-070.

## Wat er is veranderd in ARB-070

Deze map beschrijft de v3-redesign van de dashboard-schil, niet alleen een visuele restyle. De belangrijkste
structurele wijzigingen t.o.v. de vorige (Cobalt, ARB-069) versie:

- **Sidebar weg, floating topbar erbij.** Navigatie zit nu in `header.topbar` (wordmark, primaire nav, trading-mode-knoppen, acties), niet meer in een linker sidebar. ≤1280px wordt de primaire nav een vaste onderste capsule (alle 7 items, scrollbaar).
- **Licht is nu de standaardstand**, donker de toggle — vóór v3 was het omgekeerd (`body.light` als override op een donkere standaard; nu `body.dark` als override op een lichte standaard).
- **Tabs op drie pagina's.** Opportunities (Captured/Decisions/Costs/Exports), Runs (Sessions/Ledger/Journal/Reservations) en System (Adapters/Collection/Capabilities) zijn niet meer één lange scroll maar een `Tabs`/`TabPanel`-primitief (WAI-ARIA tabs pattern, roving tabindex). Zie elk bestand de sectie "Regels bij redesign" voor welk patroon (echte `TabPanel`'s vs. handmatige `hidden`-divs) elke pagina gebruikt en waarom dat verschil ertoe doet.
- **Dialogen zijn nu drawers.** `RecordedDialog` (Overview/Opportunities-Captured) en `ResearchDialog` (Opportunities-Decisions) zijn niet meer gecentreerde modals maar rechter zijpanelen (`dialog.drawer` / `dialog.research-dialog`, 660px, 100% ≤760px).
- **Nieuw op Overview:** de `.stats`-strip (4 mini-statistieken).
- **Nieuw op System:** de "Capability gates"-tabel (letterlijke spiegel van de 10 vlaggen uit `/v1/capabilities`), naast het bestaande `.facts`-blok.
- Alle onderliggende teksten, componenten en API-contracten zijn ongewijzigd, tenzij een bestand hieronder expliciet een tekstwijziging noemt (alleen de nieuwe tab-labels en de `.stats`/`.steps`-labels zijn nieuwe strings).

Volledige punt-voor-punt vergelijking: [VERSCHIL-RAPPORT-v3.md](VERSCHIL-RAPPORT-v3.md) (prototype v3 vs. huidige app vs. `design.md`) en [VERLIES-CHECK-v3.md](VERLIES-CHECK-v3.md) (wat de app had en waar het in de v3-indeling terugkomt). De schil-contractdocumenten die deze redesign hebben gestuurd staan in [BUILD-SPEC-ARB-070.md](BUILD-SPEC-ARB-070.md).

## Bestanden

| Bestand | Wat |
|---|---|
| [00-shell.md](00-shell.md) | Gedeelde schil: topbar, nav (± bottom capsule ≤1280px), modus-knoppen, banners, pagehead + chain filter, controlbar, footer, "Change password", en alle bouwstenen (button, pill, notice, panel, table, dialog/drawer, tabs, form) |
| [01-signin.md](01-signin.md) | Sign in / Set your password — nu een split (aside + kaart) ≥1120px |
| [02-overview.md](02-overview.md) | Overview — met de nieuwe `.stats`-strip |
| [03-opportunities.md](03-opportunities.md) | Opportunities: 4 tabs (Captured/Decisions/Costs/Exports), recorded table, record drawer, decision explorer, frozen export, capture audit, cost research, decision drawer |
| [04-experiments.md](04-experiments.md) | Experiments: sessie aanmaken (`ol.steps`) |
| [05-runs.md](05-runs.md) | Runs: 4 tabs (Sessions/Ledger/Journal/Reservations), sessiekaarten + paper accounting |
| [06-strategies.md](06-strategies.md) | Strategies: configuratie-register, nu als tabel |
| [07-system.md](07-system.md) | System: 3 tabs (Adapters/Collection/Capabilities), adapter catalog, collection health, capabilities + nieuwe capability-gates-tabel |
| [08-real-trading.md](08-real-trading.md) | Real trading: uitgeschakelde werkruimte |
| [09-owner-overview.md](09-owner-overview.md) | Owner overview: eigenaaroverzicht in gewone taal (NL standaard) |
| [VERSCHIL-RAPPORT-v3.md](VERSCHIL-RAPPORT-v3.md) | Prototype v3 vs. huidige app vs. design.md, punt voor punt, met keuze A/B per punt |
| [VERLIES-CHECK-v3.md](VERLIES-CHECK-v3.md) | Wat de app heeft en v3 niet (✅/🟡/❌ per onderdeel) en waar het in de v3-indeling komt |
| [BUILD-SPEC-ARB-070.md](BUILD-SPEC-ARB-070.md) | De schil-/tabs-/primitievencontract dat wave 1 en 2 heeft gestuurd |

## Screenshots

Map `screenshots/`. Per scherm `-desktop.png` (1440 × 1000) en `-mobile.png` (390 × 844), full-page tenzij anders genoemd. Overview toont zowel het lichte (standaard) als het donkere thema; de rest alleen het standaardthema.

| Bestand | Scherm |
|---|---|
| `00-signin-*` | Sign in |
| `00-signin-help-*` | Sign in met help-notice open |
| `01-overview-*` | Overview (light, standaard) |
| `01-overview-dark-*` | Overview (dark) |
| `01b-record-dialog-*` | Drawer "Captured market record" (niet full-page) |
| `02a-opp-captured-*` | Opportunities → tab Captured |
| `02b-opp-decisions-*` | Opportunities → tab Decisions (standaardtab) |
| `02c-opp-costs-*` | Opportunities → tab Costs (na "Assess hypothetical costs") |
| `02d-opp-exports-*` | Opportunities → tab Exports (na "Prepare frozen session export") |
| `02e-decision-drawer-*` | Drawer "Decision evidence detail" (niet full-page) |
| `03-experiments-*` | Experiments met ingevuld formulier |
| `04a-runs-sessions-*` | Runs → tab Sessions (standaardtab) |
| `04b-runs-ledger-*` | Runs → tab Ledger (na "Inspect paper run") |
| `04c-runs-journal-*` | Runs → tab Journal |
| `04d-runs-reservations-*` | Runs → tab Reservations |
| `05-strategies-*` | Strategies |
| `06a-system-adapters-*` | System → tab Adapters (standaardtab) |
| `06b-system-collection-*` | System → tab Collection, met collection-sessie gekozen |
| `06c-system-capabilities-*` | System → tab Capabilities, met de nieuwe capability-gates-tabel |
| `07-real-trading-*` | Real trading |
| `08-change-password-*` | Paneel "Change password" open (niet full-page) |
| `09-owner-overview-*` | Owner overview met sessie gekozen (Nederlands) |

De data is synthetisch (fixtures uit `apps/web/tests/*.fixture.ts`). Geen echte marktdata. De API is gestubd op `/v1/**`.

### Opnieuw maken

Script staat in `tools/`. Vanuit de repo-root:

```bash
ln -sfn ../../../apps/web/node_modules docs/redesign-handoff/tools/node_modules
```

```bash
cd apps/web && npm run build && npx playwright test -c ../../docs/redesign-handoff/tools/pw.config.ts
```

De symlink staat niet in git (`tools/.gitignore` sluit `node_modules` en `test-results/` uit) — elke checkout moet hem opnieuw aanmaken. `pw.config.ts` draait de preview-server op **poort 4174**, niet 4173: `apps/web/playwright.config.ts` (de eigen e2e-suite van de app) gebruikt zelf 4173 voor zijn preview-server, en een andere poort voorkomt een botsing als beide suites naast elkaar draaien.

## Bronbestanden

| Wat | Waar |
|---|---|
| Design system (vast, leidend) | `design.md` (repo-root, zelfde als `DESIGN.md`) |
| Schil-/tabs-/primitievencontract voor ARB-070 | `docs/redesign-handoff/BUILD-SPEC-ARB-070.md` |
| Tokens (kleur, font, spacing, type, radius, motion) | `apps/web/src/tokens.css` |
| Alle styling (plain CSS, class-based) | `apps/web/src/styles.css` |
| App shell (topbar, nav, modus-knoppen, controlbar, footer) | `apps/web/src/ConnectedApp.tsx` |
| Tabs/TabPanel-primitief | `apps/web/src/components/ui/Tabs.tsx` |
| Paginacomponenten | `apps/web/src/components/pages/*.tsx` |
| Overige componenten | `apps/web/src/components/*.tsx` |
| API client + types | `apps/web/src/api/*.ts` |
| UX-document (IA, woordenlijst) | `docs/05-UX-DESIGN.md` |
| HTML entry (fonts via Google Fonts) | `apps/web/index.html` |

Niet in productie: `DemoApp.tsx`, `ResearchViews.tsx`, `OpportunityTable.tsx`, `OpportunityDialog.tsx`. Alleen `tests/account.test.ts` gebruikt ze. Negeer ze.

## Techniek in het kort

- React 19 + Vite. Geen router. Pagina = state `page` in `ConnectedApp.tsx`. Sub-werkruimtes blijven gemount met `hidden`. Binnen een pagina met tabs: zie de betreffende `0X-*.md` voor het gebruikte tabpatroon (echte `TabPanel` vs. handmatige `hidden`-div — het verschil bepaalt of state bij tabwissel bewaard blijft).
- Polling elke 5 s. Na 15 s zonder antwoord: pill wordt `STALE / LAST KNOWN` (amber).
- Capability-flags van `/v1/capabilities` bepalen welke secties tonen: `decision_history`, `paper_ledger`, `paper_run_creation`, `collection_telemetry`, `session_export`, `cost_assessments`, `adapter_support` — dezelfde tien vlaggen (plus `live_execution`, `market_data`, `opportunity_capture`) staan nu ook letterlijk in de nieuwe "Capability gates"-tabel op System → Capabilities.
- Fonts: Space Grotesk (display), Inter (body), JetBrains Mono (mono).

## Harde regels bij het redesign (uit `design.md`)

1. Honesty-badges blijven, en blijven uppercase mono: `PAPER TRADING`, `HYPOTHETICAL`, `CAPTURED DATA ONLY`, `PERSISTED EVIDENCE`, `MANUAL ASSUMPTIONS`, `RECORDED ATTEMPTS`, `FROZEN EXPORT`, `IMMUTABLE`, `STALE / LAST KNOWN`, `API CONNECTED`.
2. Accent (cobalt) ≤ 5 % van elk viewport. Geen gradients. Eén signaalkleur.
3. Stille success. Geen toasts, geen confetti.
4. Geen hero, geen decoratie op app-pagina's. Hairline borders, radius 10 px, geen schaduw (alleen de dialog/drawer).
5. Teksten blijven inhoudelijk gelijk. Elke tekst zegt precies wat wel en niet bekend is. Herschrijven is een aparte beslissing.
6. Toegankelijkheid blijft: skip link, `aria-live` status, focus rings, 44 px touch targets, native `<dialog>` met focus trap, `h1` krijgt focus bij navigeren, tabs volgen het WAI-ARIA tabs pattern (roving tabindex, pijltjestoetsen).
7. Geen uppercase buiten mono-labels en badges. Geen cursieve koppen.
