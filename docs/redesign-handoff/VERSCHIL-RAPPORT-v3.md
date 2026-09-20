# Verschil-rapport · prototype v3 vs. huidige app vs. design.md

Datum: 19 sep 2026. Vergelijkt `docs/arbitrage-mobbin-v3` (prototype), `apps/web` op `main` (na #192 en #185) en het vastgelegde `design.md`.

## Hoe je dit gebruikt

- Elk punt heeft een nummer. Bij elk punt staan **A** en **B**. Daaronder staat mijn advies.
- Antwoord met de nummers en je keuze. Bijvoorbeeld: `1B 2A 3B 9A 10B …`. Zeg je niks bij een punt, dan neem ik mijn advies.
- Daarna bouw ik in stappen (schil → tokens → pagina's). Niets is nu gebouwd.

Legenda:
- 🟥 botst met `design.md` → bij keuze A moet `design.md` mee (punt 36).
- 🟨 nieuw in het prototype, maar er is **geen API-data** voor.
- 🟩 past al, geen keuze nodig.

Vaste grenzen die ik in geen enkele keuze loslaat: teksten die de eerlijkheidsgrens dragen blijven letterlijk (zie handoff-docs "Regels bij redesign"), a11y-basis blijft (labels, focus, 44px doelen, kleur nooit als enige drager), geen demo-knoppen uit het prototype (punt 34).

---

## A. Schil en navigatie

### 1. Aantal pagina's
- **v3:** 6 (Overview, Opportunities, Experiments, Runs, Strategies, System). Owner overview ontbreekt.
- **App:** 7. `Owner overview` is navitem 2 (`ConnectedApp.tsx:13`, #185).
- **design.md:** zegt niets.
- **A** 6 pagina's, Owner overview weg · **B** 7 pagina's.
- **Advies: B.** Owner overview is net gebouwd en heeft tests (`ownerOverview.spec.ts`). Zet hem in de hoofdnav na Overview; op tablet/telefoon onder "More".

### 2. Vorm van de navigatie
- **v3:** zwevende sticky header (76px) met pill-links; ≤1119px een vaste capsule onderaan met 4 items (Overview · Evidence · Runs · More) en een "More"-dialoog voor de rest.
- **App:** linker sidebar; ≤800px top-nav.
- **design.md:** "Workbench", geen sidebar-regel. 🟩 qua regel.
- **A** header + capsule van v3 · **B** sidebar houden, alleen restylen.
- **Advies: A.** Het is de kern van het nieuwe ontwerp. Voorwaarde: de nav blijft `nav[aria-label="Primary navigation"]` met **knoppen** (geen `<a href="#/…">`). Dan blijven alle 15 nav-locators in de tests werken. Geen hash-router; de app heeft er nu geen en niemand vraagt om deeplinks. (`ponytail:` router toevoegen als je bookmarks per pagina wilt.)

### 3. Paper / Real schakelaar
- **v3:** geen schakelaar. "Real trading" is een pagina via More.
- **App:** `nav.trading-modes` (aria-label `Trading mode`), knoppen Paper/Real; in Real-modus verbergt de schil de paginalinks en sluit de open dialoog. Tests kijken naar `Trading mode`.
- **design.md:** zegt niets.
- **A** Real als gewone pagina · **B** schakelaar houden.
- **Advies: B.** Gedrag en tests blijven. Visueel wordt het een segment-pill in de header (v3-stijl). De inhoud van de Real-pagina mag het v3-paneel zijn (punt 28).

### 4. Breadcrumbs en preview-chip 🟨
- **v3:** "Private workspace › {pagina}" + chip "Design v3 · Synthetic preview".
- **App:** niet aanwezig.
- **A** overnemen · **B** weglaten.
- **Advies: B.** Eén niveau diep is geen kruimelpad. De chip is prototype-tekst.

### 5. Footer
- **v3:** inverse (zwarte) footer met "Research first. Evidence always." + "Offline design preview" + "Explore preview states".
- **App:** `Arbitrage · Paper trading` + kleine regel.
- **design.md:** geen slogans ("no enrichment").
- **A** v3-footer · **B** huidige tekst, nieuwe stijl.
- **Advies: B.** Slogan is versiering. De inverse stijl mag wel.

### 6. Zoeken (Cmd/Ctrl+K en `/`) 🟨
- **v3:** zoekt in fixtures in het geheugen.
- **App:** geen zoekfunctie; API heeft geen zoek-endpoint.
- **A** bouwen over geladen data · **B** weglaten.
- **Advies: B.** Zonder endpoint zoek je alleen in wat toevallig geladen is. Dat misleidt. Toevoegen als er een `/v1/search` komt.

### 7. Thema-knop
- **v3 en app:** allebei een knop. 🟩 Alleen de standaard verschilt (punt 9).

### 8. Account-knop
- **v3:** account-orb opent een dialoog.
- **App:** losse knoppen `Change password` en `Sign out` in de schil.
- **A** orb + menu met alleen de bestaande twee acties · **B** knoppen los laten staan.
- **Advies: A.** Zelfde acties, minder ruimte in de header. Geen nieuwe acties.

---

## B. Thema en tokens

### 9. Standaardthema 🟥
- **v3:** licht standaard, donker via `data-theme`.
- **App:** donker standaard, licht via `body.light` (knop `Light theme`).
- **design.md:** donker standaard.
- **A** licht standaard · **B** donker standaard.
- **Advies: A.** Het prototype is licht ontworpen; donker is de afgeleide. Mechanisme blijft één class/attribuut; knoplabel wordt `Dark theme`. Breekt alleen `tools/shots.spec.ts` (één regel).

### 10. Kleur 🟥
- **v3:** monochroom. `--canvas #fff`, `--ink #141414`, `--soft #f3f3f3`, `--hairline #f0f0f0`, accent = ink. Geen groen/rood/oranje. Status via icoon + tekst.
- **App:** cobalt-accent (oklch), `--color-warn` oranje voor `.pill.amber` (IN_PROGRESS, failed…), en de gezondheids-pill groen/oranje/rood op Owner overview.
- **design.md:** "one signal colour", accent ≤5%.
- **A** volledig monochroom · **B** monochroom + `--color-warn` behouden voor amber-pills en de Owner-overview-pill.
- **Advies: B.** Zwart/wit als basis. Eén waarschuwingskleur houden voor "let op"-staten; de tekst ernaast blijft altijd staan, dus a11y blijft goed. Volledig monochroom haalt informatie weg die de eigenaar nu gebruikt.

### 11. Hoekradius 🟥
- **v3:** panelen 24px, inputs 16px, pills/knoppen 9999px.
- **App:** `--radius-card 10px`, `--radius-input 6px`, `--radius-chip 4px`.
- **design.md:** 10px surfaces, 6px CTA.
- **A** v3-waarden in dezelfde tokens · **B** houden.
- **Advies: A.** Alleen tokenwaarden wijzigen; geen nieuwe tokens.

### 12. Lettertypes 🟥
- **v3:** `"Saans", Inter, …`. Saans zit **niet** in de map; in de praktijk rendert het prototype in Inter. Koppen weight 652.
- **App:** Space Grotesk (koppen), Inter (tekst), JetBrains Mono (cijfers, badges).
- **design.md:** die drie.
- **A** Inter voor koppen én tekst (weight 650 voor koppen), JetBrains Mono blijft · **B** Space Grotesk houden.
- **Advies: A.** Dat is wat je in het prototype echt ziet. Eén font minder laden. Mono blijft voor hashes, bedragen en badges.

### 13. Letterafstand 🟥
- **v3:** `* { letter-spacing: 0 !important }`.
- **App / design.md:** koppen −0.02em, mono-labels ruimer.
- **A** alles 0 via tokens (`--tracking-*: 0`), zonder de `!important`-regel · **B** houden.
- **Advies: A.** Zelfde effect als v3, zonder de globale hamer.

### 14. Kopgrootte 🟥
- **v3:** h1 48px desktop, 40 → 38 → 36 → 32px naar beneden.
- **App / design.md:** `--text-display` clamp(1.75rem … 2.25rem) ≈ 28–36px.
- **A** clamp(2rem, …, 3rem) · **B** houden.
- **Advies: A.** Alleen de clamp aanpassen.

### 15. Schaduwen · 16. Beweging
- 🟩 Beide plat, geen schaduw (behalve dialoog-lift), `prefers-reduced-motion` zet alles uit. Geen keuze nodig.

### 17. Spacing-tokens
- **v3:** losse pixels in CSS.
- **App / design.md:** 4-punts schaal in `tokens.css`, geen losse pixels in pagina's.
- 🟩 **Geen keuze:** v3-afstanden worden vertaald naar de bestaande tokens. Mechanisme blijft, waarden veranderen.

### 18. Breakpoints
- **v3:** 1600 / 1480 / 1280 / **1119** (capsule) / 940 / **760** (telefoon) / 359.
- **App:** 900 / 800 / 700 / 540.
- **A** v3-breekpunten · **B** houden.
- **Advies: A.** Hoort bij punt 2. Let op: ≤1119px is ook een liggende tablet; die krijgt dan de capsule.

### 19. Woordmerk en knoppen 🟥
- **v3:** zwart merk-tegel + "Arbitrage."; primaire knop = zwarte pill, secundaire = lichtgrijze pill.
- **App / design.md:** `↗ Arbitrage`, glyph in accent; primaire knop = cobalt vulling, 6px radius.
- **A** v3 · **B** houden.
- **Advies: A.** Volgt uit punt 10 en 11.

---

## C. Tekst en badges

### 20. Badges (PAPER TRADING, HYPOTHETICAL, CAPTURED DATA ONLY, API CONNECTED, RECORDED ATTEMPTS …) 🟥
- **v3:** `.badge { text-transform: none }`, zinsopmaak met icoon ("Paper research").
- **App:** mono, hoofdletters. `API CONNECTED` staat letterlijk in 8 testbestanden.
- **design.md:** "project requirement, stay uppercase".
- **A** zinsopmaak · **B** hoofdletters houden, wel de v3-pilvorm.
- **Advies: B.** Het is een projecteis en het breekt anders 8 testbestanden. Vorm (9999px, hairline) mag van v3.

### 21. Koppen met een punt erachter
- **v3:** `pageHead` plakt overal een punt ("Research, at a glance.", "Welcome back.").
- **App:** geen punt. Tests kijken naar exacte koppen: `Paper trading overview`, `Sign in`, `Set your password`, `Real trading is not available`, `1. Status` … `5. Route naar PAPER/live`.
- **A** punt toevoegen en tests aanpassen · **B** geen punt.
- **Advies: B.** Het is stijl, geen betekenis, en het kost testwerk.

### 22. Paginatitels en ondertitels
- **v3:** "Research, at a glance." / "Opportunities" / … met per pagina een eigen ondertitel (bijv. "Inspect evidence. Separate quotes, decisions, and hypothetical assumptions.").
- **App:** h1 `Paper trading overview` op Overview, anders de paginanaam; één vaste ondertitel `Inspect evidence and control each immutable research session.`
- **A** v3-titels · **B** huidige titels, wel de v3-ondertitels per pagina.
- **Advies: B.** Titels staan in tests. De v3-ondertitels zijn inhoudelijk juist en staan in geen test; die kun je overnemen.

### 23. Overige teksten (notices, uitleg, knopnamen)
- **v3:** veel korter en herschreven.
- **App:** lange, precieze teksten. Dit zijn de eerlijkheidsgrenzen (zie elke handoff "Regels bij redesign").
- **design.md:** copy unchanged.
- **A** v3-teksten · **B** teksten letterlijk houden.
- **Advies: B.** Niet onderhandelbaar voor mij: knopnamen en notices dragen tests én de scope-grens uit `AGENTS.md`. Alleen de opmaak verandert.

---

## D. Per pagina

### 24. Overview
- **v3:** 4 stat-tegels (Research sessions · Need attention · Captured evidence · Net after costs "Unknown") + sessies-paneel + attention-blok + "What the evidence supports." (Capture Retained / Simulation Failed / Explicit costs Incomplete / Execution eligibility Unknown) + tabel "latest evidence". Knop "New experiment".
- **App:** controlbar (API CONNECTED / Stop all / Refresh), sessiekaarten, opportunity-tabel, decisions, capabilities-paneel (zie `02-overview.md`).
- **Data:** "Need attention" is af te leiden (DEGRADED of `outstanding_attempts > 0`). "Net after costs" is **altijd** Unknown 🟨. Het readiness-paneel is in v3 van één record afgeleid 🟨.
- **A** v3-indeling met alleen tegels die de app al kan tellen (sessies, records, decisions, need attention) · **B** huidige indeling restylen.
- **Advies: A**, zonder "Net after costs" en zonder het readiness-paneel. Een tegel die altijd "Unknown" zegt is versiering. "New experiment" is een gewone link naar Experiments; mag blijven.

### 25. Record- en decision-inspectie
- **v3:** rechter-lade (`dialog.drawer`, 660px; volledig scherm ≤760px) met tabs Summary / Route & costs / Provenance.
- **App:** modale dialoog met alle inhoud onder elkaar. Tests zoeken op tekst (`Decision evidence detail`).
- **A** lade, inhoud ongewijzigd · **B** dialoog houden.
- **Advies: A.** `<dialog>` blijft `role=dialog`, dus tests blijven werken. Tabs pas als de inhoud er echt om vraagt (`ponytail:` eerst één scrollende lade).

### 26. Opportunities
- **v3:** 4 tabs (captured / decisions / costs / exports) met pijltjestoetsen, zoekveld, List/Cards-schakelaar, decision mini-stats, kostenscenario met review-vinkje en "Save local scenario", export freeze/JSON/CSV.
- **App:** één lange pagina: opportunity-tabel, decision explorer (sessie, coverage, groepen, paginering), cost assessments, frozen export (`03-opportunities.md`).
- **A** de vier blokken worden vier tabs, inhoud gelijk · **B** lange pagina houden.
- **Advies: A.** Tabs zijn puur indeling. **Niet** overnemen: zoekveld (punt 6), List/Cards 🟨, "Save local scenario" 🟨 (de app doet kosten via de API-knop `Assess hypothetical costs for …`). Mini-stats zijn af te leiden uit de geladen pagina; optioneel.

### 27. Experiments
- **v3:** stappen 01/02/03, readonly netwerk, aside "Immutable settings", knop "Create preview session".
- **App:** formulier met `Validated configuration`, `Session network`, `Experiment reference`, knop `Create research session`; plus het onopgeloste-aanvraag-gedrag (`unresolvedRequest`).
- **A** stappen-indeling, zelfde labels en knopnaam · **B** houden.
- **Advies: A.** Labels staan in tests en blijven. Knopnaam blijft `Create research session`. Netwerk blijft een echte select (v3 zet hem readonly omdat het fixture maar één netwerk heeft).

### 28. Real trading
- **v3:** vergrendeld paneel + "Return to paper workspace".
- **App:** paneel met twee zinnen en een **disabled** knop `Start real trading` (handoff-grens).
- **A** v3-paneel met de app-teksten en de disabled knop erin · **B** houden.
- **Advies: A.** Stijl van v3, tekst en knop van de app. "Return to paper workspace" is gewoon de Paper-knop (punt 3), dus niet dubbel.

### 29. Runs
- **v3:** tabs sessions / ledger / journal / reservations; "Stop N loaded sessions" met bevestiging.
- **App:** `Paper session`-select, runs-lijst, run-inspectie (Hypothetical balances, journal, reservations); `Stop all` in de controlbar met expliciete scope-tekst.
- **A** tabs · **B** houden.
- **Advies: A** voor de tabs. `Stop all` blijft hoe hij is (regel 4 van v3 §8: bulk-doelen expliciet benoemd).

### 30. Strategies
- **v3:** tabel.
- **App:** lijst van geregistreerde configuraties (`06-strategies.md`).
- **A** tabel, zelfde velden · **B** houden.
- **Advies: A.** Klein werk.

### 31. System
- **v3:** tabs health / collection / capabilities, met "health cards" (API, worker, source).
- **App:** Adapter support catalog + Collection attempt health + Capabilities and data quality (`07-system.md`).
- **Data:** health cards per systeem bestaan niet in de API; er is `API CONNECTED` en per-sessie hartslag 🟨.
- **A** drie tabs = adapters / collection / capabilities, inhoud gelijk · **B** houden.
- **Advies: A.** Tab 1 heet "Adapters", niet "Health"; geen kaarten verzinnen.

### 32. Sign-in
- **v3:** gesplitste lay-out, linker kolom met marketing-tekst, "Welcome back.", uitgeschakelde velden.
- **App:** kop `Sign in` / `Set your password`, hulpknop `First time here or forgot your password?`, a11y-aankondigingen (`auth-announcement.spec.ts`).
- **A** gesplitste lay-out op desktop, één kolom ≤760px, teksten van de app · **B** houden.
- **Advies: A.** Geen "Welcome back.", geen marketing-kolom; links mag een rustig vlak met het woordmerk.

### 33. Owner overview
- **v3:** bestaat niet.
- **App:** 5 genummerde koppen, Nederlands standaard, gezondheids-pill (`09-owner-overview.md`).
- 🟩 **Geen keuze:** blijft, krijgt dezelfde panelen/tokens. Kleur-pill volgt punt 10.

### 34. Chain-filter en controlbar
- 🟩 Beide hebben een chain-select. Label `View chain` en id `connected-chain` blijven. Controlbar (API CONNECTED / Stop all / Refresh) wordt de v3 "safety strip"-rij: badge links, acties rechts. Geen keuze nodig.

---

## E. Niet porten

### 35. Prototype-eigen demo-knoppen (nooit bouwen)
"Simulate acknowledgement / ready / reconcile", "Explore preview states", "Re-read fixture", "Save local scenario", de preview-scenario-dialoog, alle synthetische fixtures. Het v3-document zegt dit zelf in §8: testknoppen, geen features. 🟩 Geen keuze.

### 36. `design.md` herschrijven 🟥
Kies je A bij 9–14 en 19, dan klopt `design.md` niet meer (Theme, Typography, CTA voice, Per-page allowances, wordmark). Het bestand zegt zelf: "Amend intentionally — the file is the rule."
- **A** ik herschrijf `design.md` vóór het bouwen, als eerste PR · **B** eerst bouwen, dan design.md.
- **Advies: A.** Anders bouwen we tegen de regel in.

---

## Wat er in tests verandert bij mijn adviezen

| Keuze | Testeffect |
|---|---|
| 2 (nav met knoppen + zelfde aria-label) | 0 |
| 3 (`Trading mode` blijft) | 0 |
| 9 (licht standaard, knop `Dark theme`) | 1 regel in `docs/redesign-handoff/tools/shots.spec.ts`; app-tests raken het niet |
| 20–23 (badges/koppen/teksten gelijk) | 0 |
| 25 (`<dialog>`-lade) | 0, tests zoeken op tekst en rol |
| 26/29/31 (tabs) | 0, mits de inhoud van de niet-actieve tab pas na klik zichtbaar is én de tests eerst op de tab klikken → **let op:** tests die nu direct `Prepare frozen session export` of `Inspect paper run …` zoeken moeten dan eerst de tab openen. Dat is een kleine, mechanische aanpassing. |

Ga je bij 20–23 toch voor A, dan raakt dat 8+ testbestanden en `docs/redesign-handoff/*` moet mee.

---

## Samengevat: dit zou ik doen

Schil, tokens en indeling van v3 (2, 8, 9, 11–14, 18, 19, 24–32, 36 = A). Alles wat betekenis draagt van de app (1, 3, 5, 20–23 = B). Eén waarschuwingskleur houden (10 = B). Weglaten wat geen data heeft (4, 6 = B; 35).

Antwoordformulier (kopieer, vul in wat afwijkt):

```
1:  2:  3:  4:  5:  6:  8:  9:  10:  11:  12:  13:  14:  18:  19:
20:  21:  22:  23:  24:  25:  26:  27:  28:  29:  30:  31:  32:  36:
```
