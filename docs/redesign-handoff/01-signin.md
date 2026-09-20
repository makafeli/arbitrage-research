# 01 · Sign in / Set your password

Bron: `apps/web/src/components/AccountAccess.tsx` → `AccountAccess`. Styles: `.account-screen`, `.account-top`, `.account-aside`, `.account-card` in `styles.css`.

Screenshots:
- `screenshots/00-signin-desktop.png` · `screenshots/00-signin-mobile.png`
- `screenshots/00-signin-help-desktop.png` · `screenshots/00-signin-help-mobile.png` (help-notice open)

## Doel

Privé toegang. Geen publieke registratie. Geen wallet- of exchange-gegevens. Twee varianten op hetzelfde scherm:
1. **Sign in** (standaard).
2. **Set your password** — alleen als de URL een fragment `#activate=<64 hex tekens>` heeft. Het fragment wordt direct uit de adresbalk verwijderd en nooit opgeslagen.

## Wanneer zichtbaar

Zolang `authenticated === false`. De app-schil is dan `hidden`. Na inloggen verdwijnt dit scherm en verschijnt de schil (Overview).

## Opbouw

Nieuw in v3: het scherm is een **split** in plaats van één gecentreerde kaart — links een `aside` met merk en drie
vertrouwensregels, rechts het formulier. Vanaf ≥1120px staan beide naast elkaar; daaronder staat de aside boven
de kaart.

```
main.account-screen              flex kolom, gap 48px, padding 24px
├─ div.account-top                flex, ruimte tussen
│  ├─ a.brand[href="/"][aria-label="Arbitrage home"]   alleen span.brandmark(aria-hidden,"↗") in de zwarte tegel — geen zichtbare tekst
│  └─ button[aria-label="Switch to dark theme"|"Switch to light theme"]   tekst "Light theme" | "Dark theme"
├─ aside.account-aside            ≥1120px: naast de kaart · <1120px: erboven, volle breedte
│  ├─ div.brand                   span.brandmark(aria-hidden,"↗") + "Arbitrage."  (mét punt)
│  └─ ul.account-facts            drie regels, elk een losse `<li>`:
│     ├─ li  "Private account access."
│     ├─ li  "No wallet or exchange credentials are requested here."
│     └─ li  "There is no public registration."
└─ section.panel.account-card[aria-labelledby=login-title]    ≥1120px: rechterkolom · <1120px: volle breedte, onder de aside
   ├─ p.eyebrow                  "Your trading workspace"
   ├─ h1#login-title             "Sign in"  |  "Set your password"
   ├─ p.muted                    zie teksten
   ├─ p.notice.error-notice[role=alert]   (conditioneel)
   ├─ p.notice[role=status]                (conditioneel)
   ├─ form.connected-form
   │  ├─ label+input#account-email        "Email address"  type=email, max 254
   │  ├─ label+input#account-password     "Password" | "New password"  type=password, max 128 (min 15 bij activatie)
   │  ├─ [activatie] label+input#account-confirm  "Confirm password" + p.tiny
   │  └─ button.primary[type=submit]
   ├─ button.space-top            "First time here or forgot your password?"  |  "Back to sign in"
   ├─ p.notice[role=status]       help-tekst (na klik)
   └─ p.tiny.space-top            "Private account access. No wallet or exchange credentials are requested here."
```

De drie zinnen in `aside.account-aside` zijn dezelfde tekst als de voetregel onderaan de kaart
(`p.tiny.space-top`) — bewust dubbel: de aside toont ze als losse vertrouwenspunten, de kaart herhaalt de
kernzin vlak bij de knop.

## Alle teksten

| Plek | Sign in | Set your password |
|---|---|---|
| eyebrow | `Your trading workspace` | idem |
| h1 | `Sign in` | `Set your password` |
| intro | `Use your email address and password to access Paper trading and Real trading.` | `Activate or recover your private account with your one-time access link.` |
| wachtwoord-label | `Password` | `New password` |
| extra veld | — | `Confirm password` + tiny `Use 15 to 128 characters. A long passphrase is welcome.` |
| primaire knop | `Sign in` | `Save password` |
| knop tijdens sessiecheck | `Checking session…` | idem |
| knop tijdens verzenden | `Please wait…` | idem |
| secundaire knop | `First time here or forgot your password?` | `Back to sign in` |
| help-notice | `Use the private account setup or recovery link supplied by the workspace owner. There is no public registration. Recovery links expire after 30 minutes and work once; no reset email has been sent.` | — |
| succes-notice | — | `Your password is saved. Sign in to continue.` (daarna schakelt het scherm naar Sign in) |
| fout mismatch | — | `The passwords do not match.` |
| fout + hint | API-fout | API-fout + ` If a previous request was interrupted, try signing in with the password you chose.` |
| voet | `Private account access. No wallet or exchange credentials are requested here.` | idem |

## States

| State | Gedrag |
|---|---|
| `checking` (sessie wordt gecontroleerd bij laden) | alle velden disabled, knop `Checking session…` |
| `busy` (verzenden) | velden disabled, knop `Please wait…`. Wachtwoordvelden worden direct geleegd. |
| knop disabled | ook als e-mail of wachtwoord leeg is |
| fout | `.notice.error-notice` boven het formulier, `role=alert` |
| help open | `.notice` onder de secundaire knop |

## Interacties

- Submit → `api.signIn(email, password)` of `api.activateAccount(email, password, token)`.
- Thema-knop werkt hier ook (zelfde `body.dark`-toggle als de topbar; zie [00-shell.md](00-shell.md) §10).
- Brand-link gaat naar `/`.

## Toegankelijkheid

- `section` heeft `aria-labelledby="login-title"`.
- Alle inputs hebben een `label[for]`. `autoComplete`: `username`, `current-password` / `new-password`.
- Fouten `role=alert`, status `role=status`.

## Mobiel / smal (<1120px)

`aside.account-aside` en `section.account-card` stapelen (aside eerst, volle breedte). ≤600px: padding 16 px, gap 24 px, kaart padding 24/16 px. Knop volle breedte (≤540px via `.connected-form button`).

## Regels bij redesign

- Geen registratie-link, geen "wachtwoord vergeten"-mailflow tonen. De help-tekst is de enige route.
- Het activatie-token blijft alleen in geheugen. Nooit in URL-query of storage.
- De home-link linksboven (`a.brand[aria-label="Arbitrage home"]`) is icoon-only: alleen de `↗`-tegel, geen zichtbare tekst. De enige zichtbare wordmark `↗ Arbitrage.` (mét punt) staat in `aside.account-aside`.
- Tests die dit scherm vastleggen: `apps/web/tests/browser/dashboard.spec.ts` (regels 5, 19, 37) en `shell-acceptance.spec.ts` (320/375/414/768 px breedtes, light + dark).
