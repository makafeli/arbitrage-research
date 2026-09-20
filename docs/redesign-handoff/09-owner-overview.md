# 09 · Owner overview

Bron: `apps/web/src/ConnectedApp.tsx` (`<div hidden={page !== 'Owner overview'}><OwnerOverview .../></div>`, navitem 2 van 7), `components/OwnerOverview.tsx`, teksten in `domain/ownerOverview.ts` (`copy.nl` / `copy.en`). Toegevoegd in #185 (`d29995c`, 19 sep 2026). Structuur ongewijzigd door ARB-070 — geen tabs op deze pagina, alleen de schil eromheen (topbar/nav/footer) is v3.

Screenshots: `screenshots/09-owner-overview-desktop.png` · `screenshots/09-owner-overview-mobile.png` (sessie `session-paper` gekozen, taal Nederlands).

Schil: zie [00-shell.md](00-shell.md). h1: `Owner overview`. Boven de pagina staat het gewone controlbar-paneel (API CONNECTED / Stop all / Refresh) zoals op Overview.

## Doel

Eén pagina in gewone taal voor de eigenaar: wat doet de sessie nu, is hij gezond, wat is gevonden, wat zou een inzet opleveren (hypothetisch), en hoe ver is de weg naar PAPER/live. Tweetalig; **Nederlands is de standaard** (bewaard in `localStorage` `owner-overview-lang`).

## Opbouw

```
(schil + controlbar)
section.research-workspace.section-spacer   (aria-labelledby owner-overview-title)
├─ .sectionhead
│  ├─ h2 "Eigenaaroverzicht" | "Owner overview" · p "Wat de sessie nu doet, in gewone taal." | "What the session is doing right now, in plain language."
│  └─ nav.trading-modes (aria-label "Taal" | "Language")  button "Nederlands" · button "English"  (aria-pressed)
├─ .panel.research-toolbar   select#owner-overview-session "Sessie" | "Session" ("Kies een sessie" | "Choose a session" + "{id} · {network} · {mode}") · button "Vernieuwen" | "Refresh"
├─ [geen sessie]  EmptyResearch "Kies een sessie om het overzicht te zien." | "Choose a session to see the overview."
└─ [sessie gekozen]
   ├─ CombinedResourceStatus:  p.notice[role=status] "Onderzoeksgegevens laden…" · .notice.error-notice[role=alert] strong "Onderzoeksgegevens niet beschikbaar." | "Verouderde momentopname behouden." · p.tiny "Momentopname ontvangen {ISO}. Handmatige verversing; dit paneel claimt geen doorlopende dekking."
   ├─ section.panel.space-top  h3 "1. Status"          → .research-facts  5 × .fact  (§1)
   ├─ section.panel.space-top  h3 "2. Gezondheid (sinds start sessie)" | "2. Health (since session start)"  (§2)
   ├─ section.panel.space-top  h3 "3. Bevindingen" | "3. Findings"      (§3)
   ├─ section.panel.space-top  h3 "4. Wat als (hypothetisch)" | "4. What if (hypothetical)"  (§4)
   └─ section.panel.space-top  h3 "5. Route naar PAPER/live" | "5. Road to PAPER/live"  (§5)
```

### §1 Status — 5 feiten

| Label (nl / en) | Waarde |
|---|---|
| Modus / Mode | `oefent met nepgeld (PAPER)` · `kijkt alleen mee (OBSERVE)` · `speelt oude data af (REPLAY)` · `handelt echt (LIVE)` |
| Toestand / State | `gestopt` · `actief` · `wordt gepauzeerd` · `gepauzeerd` · `ronden lopende taken af` · `in storing` · `herstelt van een herstart` |
| Werker actief (leeftijd hartslag) / Worker alive (heartbeat age) | leeftijd, of `nog geen hartslag ontvangen van de werker`, of `niet bereikbaar` |
| Bronachterstand / Source lag | `is bij (recent bijgewerkt)` · `loopt in (haalt achterstand in)` · `loopt vast — meer dan 5 minuten geen verzameling` · `gestopt, verzamelt niet` · `onbekend — nog geen verzameling ontvangen` |
| Laatste commando-ontvangst / Last command receipt | `{actie} · {status}` of `nog geen commando vanuit dit venster verstuurd; de dienst biedt geen volledige commandogeschiedenis` |

### §2 Gezondheid

- Eén regel: `span.pill` (`red` / `amber` / `green` / geen) met `rood` `oranje` `groen` `onbekend` — ` — {reden}`. Redenen: `de sessie staat in storing` · `de werker is niet bereikbaar` · `de sessie verzamelt momenteel niet (gestopt of gepauzeerd)` · `meer dan 5 minuten geen verzameling …`
- `.research-metrics` 3 × `.fact`: `Verzamelpogingen` · `Toegelaten aandeel` (bijv. `0/3`) · `Mislukte pogingen`.
- p.tiny: `mislukte pogingen worden geteld sinds het begin van de sessie; providerfouten tellen hierin mee en worden niet apart getoond`.

Dit is de **enige plek in de app met een groen/oranje/rood-pill**. Grenzen: >5 min geen verzameling = rood (`STALE_MS`), 1–5 min = "loopt in" (`CATCHING_UP_MS`).

### §3 Bevindingen

- Leeg: `Nog geen bevindingen voor deze sessie.`
- `.research-metrics` 5 × `.fact`: `Ruwe observaties` · `Genoteerde kandidaten` · `Afgewezen` · `Geen route` · `Data onbeschikbaar`.
- p.tiny `Eerste {n} groepen geladen.` · p.tiny `Eerste batch {ISO|onbekend} · laatste batch {ISO|onbekend}.` · ul.tiny per batch `{ISO} — {n} kandidaten`.
- Tabel "Beste kandidaten" (`.research-scroll[tabindex=0]` > `table.research-table`, caption `Beste kandidaten op gemodelleerde marge, uit de eerste 25 opgeslagen beslissingen`): Observatie (mono) · Route (`{in} → {out}`) · Bruto marge (minor) · Bewijsniveau (`kandidaat — niet gesimuleerd`) · Herkomst (OriginBadge).

### §4 Wat als (hypothetisch)

- `.research-field` input#owner-stake `Inzet (USDC)` (inputmode decimal, standaard `2`).
- p.tiny `Het ticket noemde "2 ETH" als voorbeeldinzet; deze sessie gebruikt USDC omdat dat het start-bezit van de sessie is.`
- p.notice[role=alert] `Ongeldige inzet.`
- Leeg: `geen toegelaten kandidaten in dit start-bezit om te schalen`. Anders tabel (caption `Gemodelleerde bruto marge bij deze inzet, uit de eerste 25 opgeslagen beslissingen`): Observatie · Gemodelleerde bruto marge (minor) · Vastgelegde kostenbeoordeling · Herkomst.
- p.tiny `kandidaten in een ander start-bezit worden niet geschaald voor deze inzet ({n})`.
- p.notice per caveat: `OBSERVE-kandidaten worden niet uitgevoerd, niet volledig gesimuleerd en zijn geen winst.` · `De cijfers schalen lineair met je inzet (verhouding, geen echte orderboek-simulatie) en gaan uit van USDC als start-bezit.`
- p.notice `Geen hefboom of flashlening getoond: het uitvoerbewakingsharnas (ARB-028/029) is alleen als los onderdeel getest, en een echt kostenmodel (ARB-025) is nog niet geaccepteerd.`

### §5 Route naar PAPER/live

`ul.research-checklist` > li: `strong {ARB-id}` — `span.pill {gepland | in uitvoering | gebouwd, wacht op acceptatie | geaccepteerd}` · `a issue` (target _blank) · p.tiny {resterende acceptatie}. Bron: `planning/implementation-progress.json` (build-time import, gevalideerd met `parseProgressFile`).

## Data

`GET /v1/sessions/{id}/collection-coverage` · `GET /v1/decision-coverage` · `GET /v1/decision-groups` · `GET /v1/decisions` · `GET /v1/sessions/{id}/cost-assessments` · `commands[sessionId].receipt` uit de schil. Eén keer geladen per sessie-keuze; alleen de hartslag volgt de 5 s-poll.

## Mobiel

Zelfde als de andere research-workspaces: `.research-facts`/`.research-metrics` 2 kolommen ≤900px, 1 kolom ≤540px; tabellen scrollen horizontaal; taal-knoppen wrappen onder de kop.

## Regels bij redesign

- Nederlands blijft standaard op deze pagina; de rest van de app is Engels. Dat is bewust (eigenaar leest Nederlands).
- De vijf genummerde koppen blijven, in deze volgorde. Tests kijken naar de letterlijke tekst.
- De gezondheids-pill is de enige kleur-pill in de app. Houden, maar ook hier: kleur nooit als enige drager (de reden staat ernaast).
- §4 mag niet lijken op een winstcalculator. Alle drie de notices blijven.
