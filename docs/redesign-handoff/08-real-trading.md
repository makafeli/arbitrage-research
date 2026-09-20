# 08 · Real trading

Bron: `apps/web/src/components/RealTradingPanel.tsx` (eigen component sinds v3, was eerder inline in `ConnectedApp.tsx`), gewired via `<RealTradingPanel hidden={tradingMode !== 'real'} />` in `ConnectedApp.tsx`.

Screenshots: `screenshots/07-real-trading-desktop.png` · `screenshots/07-real-trading-mobile.png`.

Schil: zie [00-shell.md](00-shell.md). Deze pagina wijkt af van de schil: er is geen `h1`-paginakop uit de nav, geen chain-filter en geen paper-content.

## Doel

Eén boodschap: echt handelen kan niet. Er is geen wallet, geen signing, geen order-submission in deze release. De pagina bestaat om die grens zichtbaar te maken, niet om iets te doen.

## Opbouw

```
(schil, in real-modus — zie hieronder)
section.panel.real-trading-panel   (aria-labelledby real-title; max-width 800px; padding 32px)
├─ p.eyebrow             "Real trading"
├─ h1#real-title         "Real trading is not available"
├─ p                     "Live execution has not been enabled. No real orders can be sent from this workspace."
├─ p.muted.space-top     "This release has no wallet connection, signing or live order submission. Switching workspaces never changes the mode of an existing session."
└─ button.primary.space-top  disabled   "Start real trading"
```

## Wat de schil doet in real-modus

| Element | Paper-modus | Real-modus |
|---|---|---|
| `nav.trading-modes` | knop `Paper` actief (`aria-pressed=true`) | knop `Real` actief |
| `.navlabel` | `Paper workspace` | `Real workspace` |
| `nav.nav` (paginalinks) | zichtbaar | **verborgen** |
| Topbar pill | `PAPER TRADING` | `REAL TRADING` |
| Footer | `Arbitrage · Paper trading` | `Arbitrage · Real trading` |
| Paper-content div | zichtbaar | `hidden` (blijft gemount; state blijft bewaard) |
| Geopende dialoog | — | wordt gesloten bij de switch (`setInspected(null)`) |

Mode-knoppen zijn disabled zolang `unresolvedRequest` waar is (onzekere levering van een create-request; zie 04 en 05). Dan staat de schil-notice `#unresolved-mode-note`.

## States

- Alleen deze ene state. Geen laden, geen fout, geen data.
- De knop is altijd disabled. Er hangt geen handler aan.

## Data

Geen. Geen API-call.

## Mobiel

Paneel volle breedte, padding blijft 32px → op 390px wordt dat krap maar leesbaar. `.trading-modes` staat in de top-nav (≤800px).

## Regels bij redesign

- De knop `Start real trading` blijft disabled én zichtbaar. Weghalen zou suggereren dat de functie later "vanzelf" komt; disabled tonen maakt de grens expliciet.
- Beide zinnen blijven letterlijk. Ze zijn de handoff-boundary uit `AGENTS.md` (no signing, broadcasting, wallet funding, live activation).
- Geen "coming soon", geen waitlist, geen formulier.
- Paginalinks mogen verborgen blijven; er is niets om naartoe te gaan in real-modus.
