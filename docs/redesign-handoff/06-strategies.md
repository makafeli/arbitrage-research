# 06 · Strategies

Bron: `apps/web/src/components/pages/StrategiesPage.tsx`.

Screenshots: `screenshots/05-strategies-desktop.png` · `screenshots/05-strategies-mobile.png` (twee configuraties: PAPER · Base en OBSERVE · Base, Solana). Geen tabs op deze pagina; naamgeving ongewijzigd t.o.v. de vorige versie.

Schil: zie [00-shell.md](00-shell.md). h1: `Strategies`.

## Doel

Alleen-lezen register van de gevalideerde configuraties die de server kent. Geen knoppen. Een configuratie is onveranderlijk; een ander experiment vraagt een nieuwe validatie door de operator.

## Opbouw

Was een `.listrow`-lijst, is nu dezelfde tabelprimitief als op Opportunities/Runs (`div.research-scroll > table.research-table`):

```
(schil)
section.panel
├─ h2        "Validated configuration registry"
├─ p.muted   "Configurations are immutable server references. A changed experiment requires a newly validated configuration."
└─ [configuraties aanwezig]
   div.research-scroll[tabIndex=0][aria-label="Registered validated configurations table scroll area"]
   └─ table.research-table
      ├─ caption   "Registered validated configurations"
      ├─ thead     "Mode" · "Networks" · "Configuration digest" · "Strategies" · span.sr "Status"
      └─ tbody  per configuratie (key = configuration_digest)
         ├─ td   mode
         ├─ td   enabled_networks → "Base, Solana" (via `names[n]`, ", "-gescheiden) of "No enabled networks"
         ├─ td.mono   configuration_digest
         ├─ td   strategy_ids.join(', ') of "None registered"
         └─ td   span.pill "IMMUTABLE"
   of Empty:  h3 "No configurations registered" · p "An operator must validate and register a research configuration before a session can be created."
```

## Data

`capabilities.registered_configurations[]` → `mode`, `enabled_networks` (→ `Base` / `Solana` via `names` uit `SessionCard.tsx`), `configuration_digest`, `strategy_ids`. Zelfde bron als de select op Experiments.

## States

- Tabel met ≥1 config.
- Leeg (`Empty`-paneel in het paneel).
- Geen laad- of foutstate: de data komt uit de snapshot die de schil al heeft.

## Mobiel

De tabel zit in dezelfde `.research-scroll`-primitief als Opportunities/Runs: horizontaal scrollbaar, geen kolommen die verdwijnen of wrappen (zie [00-shell.md](00-shell.md) §8).

## Regels bij redesign

- Badge `IMMUTABLE` blijft, nu in de laatste tabelkolom (kopregel screenreader-only via `span.sr`).
- Geen bewerk-, kopieer- of verwijderknoppen. Dit is bewust een register, geen editor.
- Digest volledig tonen (mono, wrap anywhere). Niet afkorten zonder kopieer-optie; de digest is de identiteit.
- `caption` blijft (screenreader-context bij een scrollbare tabel); niet verwijderen omdat hij visueel niet opvalt.
