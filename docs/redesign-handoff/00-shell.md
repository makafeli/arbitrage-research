# 00 · Gedeelde schil en bouwstenen

Alles wat op elke ingelogde pagina staat. Bron: `apps/web/src/ConnectedApp.tsx` (regels 145–163), `apps/web/src/components/ui/Tabs.tsx`, `apps/web/src/styles.css`, `apps/web/src/tokens.css`, `docs/redesign-handoff/BUILD-SPEC-ARB-070.md`.

Screenshots: elke `01-…` t/m `09-…` in `screenshots/` toont deze schil (header + footer). Light is nu de standaard; dark staat apart in `01-overview-dark-*`.

## 1. Layout

```
a.skip[href=#main]           "Skip to content" (alleen zichtbaar bij focus)
div.shell
├─ header.topbar              zwevende balk, sticky top, rounded 9999, hairline, paper-achtergrond, 76px hoog
│  ├─ div.brand                span.brandmark(aria-hidden, "↗" in een zwarte tegel) + "Arbitrage."
│  ├─ nav.nav[aria-label="Primary navigation"]     7 knoppen (Overview · Owner overview · Opportunities ·
│  │                                                Experiments · Runs · Strategies · System), aria-current="page"
│  │                                                op de actieve; verborgen (`hidden`) in real-modus
│  ├─ nav.trading-modes[aria-label="Trading mode"]  segment pill, 2 knoppen, aria-pressed (ongewijzigd)
│  └─ div.topactions
│     ├─ span.pill             "PAPER TRADING" | "REAL TRADING"
│     ├─ button                 "Change password"  (disabled bij unresolvedRequest)
│     ├─ button[aria-label="Switch to dark theme"|"Switch to light theme"]   tekst "Dark theme" | "Light theme"
│     └─ button                 "Sign out"  (disabled bij unresolvedRequest)
└─ main#main.workspace         max-width ongewijzigd qua opbouw, nieuwe visuele stijl
   ├─ (PasswordSettings)        alleen als "Change password" open is
   ├─ section.panel.real-trading-panel   (hidden in paper-modus) → zie 08-real-trading.md
   └─ div (hidden in real-modus)
      ├─ .demo.connected-banner
      ├─ p#unresolved-mode-note.notice   (conditioneel)
      ├─ div.pagehead
      ├─ .notice.error-notice            (conditioneel)
      ├─ section.panel role=status        (alleen zolang er geen snapshot is)
      ├─ section.controlbar
      └─ … pagina-inhoud …
   ├─ footer.footer
   └─ div.sr role=status aria-live=polite aria-atomic=true   (screenreader-aankondigingen)
dialog                         RecordedDialog (buiten .shell, native <dialog>)
```

De sidebar is verdwenen. `.subbrand` ("TRADING WORKSPACE"), `.navlabel` ("Paper workspace"/"Real workspace") en
`.sidefoot` ("Research first" / "Persisted service evidence." / "Dataset origins stay explicit.") zijn geschrapt.
Navigatie en modus-schakelaar staan nu in de topbar, niet meer in een aside.

## 2. Topbar

| Onderdeel | Class | Tekst / gedrag |
|---|---|---|
| Wordmark | `div.brand` + `span.brandmark` | `↗ Arbitrage.` — de tekst is nu **"Arbitrage."** (met punt, byte-exact uit de bron). De `↗` staat in een zwarte tegel (`.brandmark`), `aria-hidden="true"`. |
| Paginanav | `nav.nav[aria-label="Primary navigation"]` | Zeven knoppen: `Overview`, `Owner overview`, `Opportunities`, `Experiments`, `Runs`, `Strategies`, `System` (volgorde = `pages`-array in `ConnectedApp.tsx`). Actieve knop heeft `aria-current="page"`. Dit is dezelfde `<nav>` als de mobiele bottom-capsule (§5) — CSS herpositioneert hem, het is geen tweede element. Verborgen (`hidden`) zodra `tradingMode === 'real'`. |
| Modus-knoppen | `nav.trading-modes[aria-label="Trading mode"] button` | `Paper trading`, `Real trading`. `aria-pressed` op de actieve. `disabled` zolang er een onopgelost verzoek is (`unresolvedRequest`). |
| Modus-pill | `span.pill` | `PAPER TRADING` / `REAL TRADING`. Verborgen tot en met 1599px; pas vanaf ≥1600px zichtbaar in de topbar (de controlbar-veiligheidsstrip draagt dezelfde badge al op elke breedte). |
| Change password | `button` | `Change password`. Toggle `PasswordSettings`; `disabled` bij `unresolvedRequest`. |
| Thema | `button[aria-label]` | Zichtbare tekst `Dark theme` (light-modus) of `Light theme` (dark-modus). `aria-label` = `Switch to dark theme` / `Switch to light theme`. |
| Sign out | `button` | `Sign out`. `disabled` bij `unresolvedRequest`; aankondiging `Signed out. Existing commands continue independently.` |

Klik op nav: `navigate()` zet `page`, kondigt aan via aria-live (`"{Page} selected. Connected service records."`) en zet focus op de `h1`.

## 3. Banner, notes, pagehead

**Banner** `.demo.connected-banner` (altijd zichtbaar in paper-modus):
- strong (mono): `PAPER TRADING`
- span: `Research data and hypothetical accounting only. Automatic paper fills are not enabled. Closing this page does not stop workers.`
- ≤800px: kolom-layout.

**Unresolved note** `p#unresolved-mode-note.notice` (alleen als er een verzoek nog niet is afgerond: sending, uncertain, of een aanmaak-/kostenverzoek loopt):
`An unresolved request is retained in this Paper trading workspace. Mode switching is disabled until its outcome is reconciled. Reloading or closing the page discards the local retry key; inspect authoritative records before creating equivalent work.`

**Pagehead** `div.pagehead`:
- `h1` (tabIndex −1, krijgt focus bij navigeren): `Paper trading overview` op Overview, anders de paginanaam (`Owner overview`, `Opportunities`, `Experiments`, `Runs`, `Strategies`, `System`).
- `p.subtitle`: `Inspect evidence and control each immutable research session.`
- `.filter`: label `View chain` + `select#connected-chain` met opties `All chains` / `Solana` / `Base`. Eronder `p.scope-note`: `View filter only. Stop controls retain their explicitly named session scope.`
- Het filter filtert sessiekaarten, opportunity-rijen, sessie-keuzelijsten en adapter-kaarten. Het verandert nooit een al gekozen sessie-scope.
- ≤760px: kolom, select volle breedte (schil stapelt hier, zie §6).

**Error notice** `.notice.error-notice[role=alert]` (bij fetch-fout):
- strong: `Connection degraded. Last known snapshot retained.` (als er al een snapshot was) of `Service unavailable or request rejected.`
- p: foutmelding · p: `No synthetic records have been substituted.`
- knop `Retry connection` (disabled tijdens refresh).

**Loading panel** `section.panel[role=status]` (zolang er nog geen snapshot is):
- h2: `Loading service records…` of `No service snapshot available`
- p.muted: `Waiting for sessions, capabilities and opportunity responses.`

## 4. Controlbar

`section.controlbar` aria-label `Connected session control summary`. v3 "safety strip": `--color-paper-2`-achtergrond, radius 16, flex wrap, gap 12.

Links:
- `span.pill.blue` `API CONNECTED` of `span.pill.amber` `STALE / LAST KNOWN` (stale = offline óf snapshot ouder dan 15 s).
- `p.statustext`: `Snapshot received {ISO-tijd}. {n} sessions loaded.` (+ ` · additional sessions are outside this page` bij `next_cursor`).
- `p.tiny`: `These controls target registered sessions. Separately launched capture processes have their own lifecycle. API connectivity does not establish feed freshness or worker health.`
- `.session-states` aria-label `All loaded session command results`: per verstuurd commando een chip `.session-status`: `{session_id}: {ACTION} · {SENDING|PENDING|APPLIED|REJECTED|DELIVERY UNCERTAIN|…}`.

Rechts `.controls`:
- `Stop all loaded sessions ({n})` (of `Stop loaded sessions ({n})` bij `next_cursor`). Disabled als niets te stoppen is of er iets verzonden wordt.
- `Refresh`. Disabled tijdens refresh.

## 5. Responsive (breakpoints uit BUILD-SPEC §2)

Breakpoints: **1600 / 1480 / 1280 / 940 / 760 / 359**.

| Breedte | Wat verandert |
|---|---|
| ≥1281px | Header houdt brand + nav + segment + acties op één rij (nav en actieknoppen compact, `white-space: nowrap`). |
| ≤1599px | De `PAPER TRADING`/`REAL TRADING`-pill in `.topactions` blijft verborgen (de controlbar-veiligheidsstrip draagt dezelfde badge al op elke breedte); pas vanaf ≥1600px wordt hij zichtbaar. |
| ≤1280px | `nav.nav` wordt een **vaste bottom capsule**: `position: fixed`, onderaan het scherm, rounded 9999, paper-achtergrond, hairline, shadow, horizontaal scrollbaar (`overflow-x: auto`, `scrollbar-width: none`). **Alle 7 items** blijven erin — geen "More"-dialoog, de capsule scrollt. `main` krijgt onderpadding ≥ 88px zodat inhoud niet achter de capsule verdwijnt. Header houdt brand + segment + acties op één rij tot 761px. |
| ≤760px | Header wraps naar **twee rijen**: rij 1 = brand + segment (space-between), rij 2 = de drie actieknoppen (elk `nowrap`, rij mag zelf wrappen). Geen icon-only knoppen. Pagehead stapelt; `.filter` volle breedte; controlbar wraps. |
| ≤359px | De merktegel `↗` en de tekst `Arbitrage.` blijven allebei op elke breedte zichtbaar (geen afkorting, geen verborgen helft — copy is bevroren); wrap is toegestaan. |
| — | Geen horizontale overflow op 320 / 375 / 390 / 414 / 768 / 1440. |

De mobiele bottom-capsule is **hetzelfde** `nav[aria-label="Primary navigation"]`-element als de desktop-nav, alleen herpositioneerd via CSS — er is geen los duplicaat in de DOM.

## 6. Footer en aria-live

`footer.footer`: links `Arbitrage · Paper trading` (of `Real trading`), rechts `No wallet or live execution controls`.

`div.sr[role=status][aria-live=polite][aria-atomic=true]`: onzichtbaar. Alle aankondigingen (navigatie, commando-resultaten, sign-out, sessie aangemaakt). Er zijn géén toasts. Dit is de enige feedback-laag naast inline notices.

## 7. Change password (PasswordSettings)

Screenshot: `screenshots/08-change-password-desktop.png`, `-mobile.png`. Bron: `components/AccountAccess.tsx` → `PasswordSettings`.

`section.panel.account-settings[aria-label="Change password"]` bovenaan de workspace:
- h2 `Change password`
- p `Changing your password signs out all sessions. Your trading data stays unchanged.`
- fout: `p[role=alert].notice` (bijv. `The passwords do not match.`)
- `form.connected-form`: `Current password` · `New password` (min 15, max 128) · `Confirm new password` · `button.primary` `Save password` · `button` `Cancel`
- Na succes: paneel sluit, gebruiker is uitgelogd (login-scherm verschijnt).

## 8. Bouwstenen (primitives) — BUILD-SPEC §3

| Bouwsteen | Class | Nu |
|---|---|---|
| Body | `body` | Light is standaard (`:root`); `body.dark` schakelt om (§9). `body.light` bestaat niet meer. |
| Panel | `.panel` | paper-achtergrond, 1px `--color-rule`, radius `--radius-panel` (24px), padding 24 (16 ≤760) |
| Inset paneel | `.panel.inset` / `.fact` | `--color-paper-2`-achtergrond, radius `--radius-card`, geen rand |
| Pill / badge | `.pill` | mono, uppercase, `--text-xs`, tracking `--tracking-mono`, radius 9999, 1px rand; `.blue` = ink-achtergrond + paper-tekst; `.amber` = warn-tint; `.red`/`.green` alleen op Owner overview |
| Knop (ghost, default) | `button` | transparant, 1px `--color-rule-2`, radius 9999, hoogte ≥ 36 (≥44 op ≤760, `--touch`) |
| Knop (primary) | `.primary` | ink-achtergrond, paper-tekst, radius 9999 (zwarte pil-CTA) |
| Tekstknop | `.textbutton`, `.cellbtn` | underline-on-hover, geen rand (ongewijzigde semantiek) |
| Invoer | `input, select, textarea` | 1px `--color-rule-2`, radius `--radius-input`, 40px hoog, paper-achtergrond |
| Tabs | `div.tabs[role=tablist]` > `button[role=tab][aria-selected]` | geselecteerd = ink-achtergrond + paper-tekst pil; overige ghost. Zie §9. |
| Tabpaneel | `div[role=tabpanel]` | `hidden` als inactief, `margin-top: var(--space-lg)` |
| Drawer | `dialog.drawer` | rechter zijpaneel, 660px breed (100% ≤760), volle hoogte, paper-achtergrond, radius 24 op de linkerrand, `::backdrop` = scrim; slide-in 180ms (uit bij reduced motion) |
| Drawer-kop | `.dialoghead` | titelrij in drawers: h2 + sluitknop |
| Drawer-tabs | `.drawer-tabs` | `.tabs` binnenin een drawer |
| Sectiekop | `.sectionhead` | h2/h3 + p links, pill of knop rechts, flex, wrapt |
| Kaartengrid | `.cards` | grid auto-fill minmax(320px,1fr) |
| Lijstrij | `.listrow` | rij met hairline onderrand, wrapt ≤760 |
| Feitengrid | `.facts`, `.research-facts`, `.research-metrics`, `.chainmetrics` | grid van `.fact`; 2 kolommen ≤900, 1 kolom ≤540 |
| Tabel | `table`, `.research-table`, `.tablewrap`, `.research-scroll` | hairline rijen, mono cijfers, horizontaal scrollbare container met `tabindex=0` |
| Notice | `.notice` | `--color-paper-2`-achtergrond, radius `--radius-card`, 1px rand; `.error-notice` = danger linkerrand 3px |
| Mini-stats | `.stats` | NIEUW: strip van `.fact`-tegels op Overview, grid 4 → 2 → 1 |
| Stappen | `.steps` | NIEUW: op Experiments, `ol.steps > li` met `span.stepnum` "01"/"02"/"03" mono vóór de bestaande labels |
| Controlbar | `.controlbar` | v3 safety strip: `--color-paper-2`-achtergrond, radius 16, flex wrap, gap 12 |
| Schil | `.topbar`, `.brand`, `.brandmark`, `.nav`, `.trading-modes`, `.topactions` | zie §1–§2 |
| Sign-in | `.account-screen`, `.account-card`, `.account-top`, `.account-aside` | zie [01-signin.md](01-signin.md) |

Typografie: h1 `--text-display` gewicht 600, letter-spacing 0; h2 `--text-xl` 600; h3 `--text-lg` 600; body `--text-base`; `.tiny` `--text-sm` `--color-ink-2`; `.mono` `--font-mono`.

## 9. Tabs-primitief (`components/ui/Tabs.tsx`)

Gebruikt op Opportunities, Runs en System (zie de betreffende docs). Contract:

```tsx
export type Tab<K extends string> = { key: K; label: string };
export function Tabs<K extends string>({ label, tabs, value, onChange }): // div.tabs[role=tablist][aria-label]
  // > button[role=tab][id=`tab-${key}`][aria-selected][aria-controls=`panel-${key}`][tabIndex=selected?0:-1]
  // ArrowLeft/Right/Home/End verplaatsen selectie én focus (roving tabindex). Geen context, geen compound API.
export function TabPanel<K extends string>({ tab, value, children }): // div.tabpanel[role=tabpanel][id=`panel-${tab}`][aria-labelledby=`tab-${tab}`][hidden={tab !== value}]
```

Tab-labels (nieuwe strings, sentence case): Opportunities `Captured` · `Decisions` · `Costs` · `Exports`; Runs `Sessions` · `Ledger` · `Journal` · `Reservations`; System `Adapters` · `Collection` · `Capabilities`.

## 10. Thema

**Light is nu standaard** (`:root`), `body.dark` schakelt om naar donker (voorheen andersom: dark was standaard, `body.light` overschreef). Zelfde tokennamen, andere waarden (zie `tokens.css`). Elke kleur in `styles.css` wijst naar een token. Bij een verdere redesign: nieuwe kleuren alleen als nieuwe tokens in `tokens.css`.

## 11. Regels bij redesign

- Copy is bevroren: elke kop, label, knoptekst, badge, `aria-label`, `id` en `htmlFor` die vandaag bestaat blijft byte-identiek. Badges blijven UPPERCASE mono.
- DOM-contracten die tests gebruiken blijven staan: `nav[aria-label="Primary navigation"]` met precies één `<button>` per pagina (desktop en mobiele capsule zijn hetzelfde element); `nav[aria-label="Trading mode"]`; knoppen `Change password`, `Sign out`; tekst `API CONNECTED` / `STALE / LAST KNOWN`; select `#connected-chain`; `a.skip[href="#main"]`; `main#main`; `div.sr[role=status][aria-live=polite]`.
- Geen "More"-dialoog voor de mobiele nav: alle 7 items blijven zichtbaar in de scrollbare bottom-capsule.
- Accent (monochroom: accent = ink) blijft de enige signaalkleur behalve amber (`STALE / LAST KNOWN`, Owner-overview gezondheidspill) en de rood/groen-pill die uitsluitend op Owner overview voorkomt.
