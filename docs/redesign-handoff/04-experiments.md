# 04 · Experiments

Bron: `apps/web/src/components/SessionCreation.tsx`, gemount door `ConnectedApp.tsx` in een `div[hidden]` (geen eigen page-wrapper).

Screenshots: `screenshots/03-experiments-desktop.png` · `screenshots/03-experiments-mobile.png` (formulier ingevuld: configuratie PAPER, netwerk Base, referentie `redesign-example`, checkbox nog uit).

Schil: zie [00-shell.md](00-shell.md). h1: `Experiments`.

## Doel

Eén ding: een nieuwe research-sessie aanmaken uit een door de operator gevalideerde configuratie. Aanmaken start géén worker. Modus, netwerk en digest liggen daarna vast. De drie stappen stonden al vóór ARB-070 als genummerde stappen in de bron (`ol.steps`); v3 voegt er alleen de `.steps`-primitief met `span.stepnum` aan toe (zie [00-shell.md](00-shell.md) §8) — de structuur zelf is ongewijzigd.

## Opbouw

```
(schil)
section.panel
├─ h2            "Create a research session"
├─ p.muted       "Choose a validated configuration. Mode, network and digest are immutable. Creation does not start a worker; recovery must finish before a stopped session can start."
├─ .notice       [geen configuraties] "Creation unavailable: no validated configurations are registered. Ask the service operator to register an eligible research configuration."
└─ form.connected-form > ol.steps
   ├─ li  span.stepnum(aria-hidden) "01"  h3 "Choose a validated configuration"
   │      fieldset
   │      ├─ label+select#configuration      "Validated configuration"  → "Choose a configuration" + per config "{mode} · {digest}"
   │      └─ .notice   [config gekozen]  strong "Review immutable settings"
   │            p "Mode: {mode} · Network: {Base|Solana|Select an enabled network}"
   │            p.mono "Digest: {digest}"
   │            p "Strategies: {ids met ', '}"  of  "Strategies: None; configuration cannot create a session"
   │            p "Virtual principal, native fee reserves, token eligibility and delay scenarios come from the validated configuration. This API does not expose their detailed values yet; review the source configuration before confirming."
   ├─ li  span.stepnum(aria-hidden) "02"  h3 "Name the session"
   │      fieldset
   │      ├─ label+select#session-network    "Session network"          → "Choose an enabled network" + Base / Solana (alleen enabled_networks van de gekozen config)
   │      ├─ p.notice   [config zonder netwerken] "Creation unavailable: this configuration has no enabled networks."
   │      └─ label+input#experiment-id       "Experiment reference"     (required, maxLength 128, autocomplete off)
   └─ li  span.stepnum(aria-hidden) "03"  h3 "Review and create"
          fieldset
          └─ label.checkbox-label  "I reviewed the source configuration, including virtual principal and separate native fee reserves."
          .notice.error-notice[role=alert]   foutmelding (+ p "Creation outcome is uncertain. Retry reuses the same exact request; settings remain locked." bij pending)
          button.primary   "Create research session" | "Creating session…" | "Retry same creation request" | "Session created"
          .notice[role=status]   [na succes] "Created {session_id} · {observed_state}. No start command was sent."
p.tiny (buiten de form)   "LIVE is unavailable. Pool selection, amounts and scenario editing require a separately validated configuration revision."
```

## States

| State | Gedrag |
|---|---|
| geen configuraties | notice; selects leeg; knop disabled |
| formulier ongeldig | knop disabled. Geldig = config gekozen + netwerk in enabled_networks + ≥1 strategy + modus in capabilities.modes + referentie 1–128 tekens + checkbox aan |
| stale snapshot of signed out | hele fieldset disabled |
| sending | fieldset disabled, knop `Creating session…` |
| pending (levering onzeker) | fieldset blijft disabled, knop `Retry same creation request`, extra fout-regel, modus-switch geblokkeerd (schil-notice `#unresolved-mode-note`) |
| rejected (400/401/403/409/422/429) | fout getoond, formulier weer vrij |
| created | knop `Session created` (disabled), status-notice. Elke veldwijziging reset `created`. |

Config wisselen reset netwerk, checkbox en created. Netwerk wisselen reset checkbox en created.

## Interacties

- Submit → `POST /v1/sessions` met body `{configuration_digest, mode, network_id, experiment_id, strategy_ids}` en een idempotency key (`crypto.randomUUID()`).
- Na succes: aria-live `Session {id} created {state}. No start command was sent.` en de sessielijst wordt ververst (nieuwe kaart op Overview/Runs).

## Data

`capabilities.registered_configurations[]`: `configuration_digest, mode, enabled_networks, strategy_ids, paper_assets?`. `capabilities.modes` (bijv. `OBSERVE, PAPER, REPLAY`).

## Toegankelijkheid

Alle velden met `label[for]`. Fout `role=alert`, succes `role=status`. Native `required`. Checkbox 20 px, accent.

## Mobiel

≤540px: formulier volle breedte, knop volle breedte.

## Regels bij redesign

- De review-notice + checkbox zijn een bewuste drempel. Niet weghalen, niet vooraf aanvinken.
- De knop heeft vier teksten. Alle vier blijven.
- Geen "advanced" velden toevoegen: bedragen en pools komen uit de configuratie, niet uit dit formulier.
- De drie `span.stepnum`-cijfers (`01`/`02`/`03`) zijn `aria-hidden`; de stapvolgorde blijft voor screenreaders leesbaar via de gewone `h3`-koppen, niet via het cijfer.
