# Arbitrage Research: overdracht en uitvoeringsplan

**Versie:** 1.0  
**Datum:** 18 september 2026  
**Samengesteld:** 18-09-2026 10:35 CEST  
**Voor:** Yasin Boelhouwer, opvolgende ontwikkelaar, technisch beheerder of coding-agent  
**Repository:** https://github.com/makafeli/arbitrage-research  
**Gecontroleerde `main`:** `52b3cb93441d02a73bd3d0c1f5105d9266c89f11`  
**Gecontroleerde bronboom:** `550805a7a0309bd257aa4cb5fee0fb3235b43971`

> **Begin hier:** de infrastructuur, login, Base-configuratie en workerstartcode bestaan. De databasecertificaatreparatie is gebouwd en getest, maar niet op productie toegepast. De gehoste worker voert nog alleen een gereedheidscontrole uit. Er is geen actieve marktfeed en geen complete automatische paper-tradingketen aangetoond. Echte handel is een afzonderlijke, nog te bouwen en goed te keuren fase.
>
> **Eerste werkpakket:** bestaande #58 voortzetten voor gecontroleerd databaseonderhoud, daarna de bestaande Base-sessie gebruiken om gegevensverzameling en bediening te bewijzen met #30, #32, #51 en #53. Bouw de eerder gemergede launcher of certificaattool niet opnieuw.

Dit document is een overdracht, geen uitvoerbaar installatiescript en geen toestemming om wallets te financieren, sleutels te gebruiken of echte transacties te verzenden. Tijdens het maken van dit bestand zijn GitHub en Railway alleen gelezen; er zijn geen nieuwe deployments, codewijzigingen of issuewijzigingen uitgevoerd.

## Inhoud

1. [Doel, scope en drie verschillende opleveringen](#1-doel-scope-en-drie-verschillende-opleveringen)
2. [Actueel gecontroleerd checkpoint](#2-actueel-gecontroleerd-checkpoint)
3. [Wat al is afgerond of gebouwd](#3-wat-al-is-afgerond-of-gebouwd)
4. [Architectuur en bestaande koppelingen](#4-architectuur-en-bestaande-koppelingen)
5. [Tickets en voortgang](#5-tickets-en-voortgang)
6. [Blokkades en operationele aandachtspunten](#6-blokkades-en-operationele-aandachtspunten)
7. [Uitvoeringsvolgorde en acceptatie per werkpakket](#7-uitvoeringsvolgorde-en-acceptatie-per-werkpakket)
8. [Volledige lijst resterende oorspronkelijke taken](#8-volledige-lijst-resterende-oorspronkelijke-taken)
9. [Testen, review, sluiten en overdracht bij onderbreking](#9-testen-review-sluiten-en-overdracht-bij-onderbreking)
10. [Besluiten en bevoegdheden die nog nodig zijn](#10-besluiten-en-bevoegdheden-die-nog-nodig-zijn)
11. [Starttekst voor de volgende ontwikkelaar of agent](#11-starttekst-voor-de-volgende-ontwikkelaar-of-agent)
12. [Bronnen en herleidbaarheid](#12-bronnen-en-herleidbaarheid)

## 1. Doel, scope en drie verschillende opleveringen

### 1.1 Productdoel

Een Rust-gebaseerd arbitrageplatform dat echte blockchainwaarnemingen verzamelt, mogelijke routes onderzoekt, beslissingen en kosten reproduceerbaar vastlegt, volledige transacties met virtueel kapitaal simuleert en resultaten in een geauthenticeerd dashboard toont. Daarna kan een afzonderlijk beoordeelde, begrensde live-strategie worden ontwikkeld en geactiveerd. Het oorspronkelijke onderzoeksproduct omvat **Base en Solana**. [S02] [S03] [S11]

De gebruiker wil voortgang naar een werkend product, niet alleen meer gemergede technische deelwijzigingen. Bestaande tickets, repository, Railway-project, login en frontend moeten worden hergebruikt. De gebruikersinterface heeft normale accounttoegang en de werkruimten **Paper trading** en **Real trading**. Herintroduceer geen publieke Demo/Connected-keuze. De naam Real trading is geen bewijs dat de uitvoeringslaag al werkt. [S04] [S14]

### 1.2 Gebruik het woord 'live' niet zonder specificatie

| Oplevering | Concreet resultaat | Echte marktgegevens? | Echt geld verplaatsen? |
|---|---|---:|---:|
| **A. Online onderzoeksflow** | Gehoste worker verzamelt gegevens; het dashboard toont waarnemingen, fouten en werkende bediening. | Ja | Nee |
| **B. Volledige paper-tradingflow** | Exacte plannen, complete simulatie, kosten, virtuele reserveringen en automatische virtuele afwikkeling werken samen. | Waar beschikbaar, met expliciete test- en scenarioherkomst | Nee |
| **C. Begrensde livepilot** | Een goedgekeurde strategie ondertekent, verstuurt en reconcilieert echte transacties onder harde limieten. | Ja | Ja, uitsluitend na de afzonderlijke goedkeuringsstappen |

Een container met status SUCCESS is niet automatisch oplevering A. Twee berekende swapprijzen zijn niet automatisch oplevering B. Een positief paper-resultaat is geen livegoedkeuring of gerealiseerde winst. [S07] [S11]

### 1.3 Base eerst: prioriteit, niet automatisch een nieuwe scope

De eerdere aanbeveling was om eerst één beperkte Base-keten bruikbaar te maken. **Er is in de zichtbare overdracht geen expliciete goedkeuring vastgelegd om Solana uit de oorspronkelijke onderzoeksrelease te verwijderen.** Daarom geldt:

- Base mag de eerste operationele demonstratie en ontwikkelprioriteit zijn.
- De oorspronkelijke Base/Solana-afhankelijkheden en acceptatiecriteria blijven geldig.
- Een formele Base-only release vereist een vastgelegde scopebeslissing en aanpassing van gedeelde tickets, tests en releaseclaims.
- Verplaats Solana, vergelijking of kwalificatie niet stilzwijgend naar 'later' om tickets te kunnen sluiten.

Bij goedgekeurde versmalling zijn #31, #34, #43 en #45 de expliciet Solana-specifieke kandidaten voor een volgende release. Gezamenlijke tickets zoals #32, #35, #38, #41, #46, #47 en #49 moeten dan inhoudelijk worden afgebakend. Basisboekhouding, monitoring, herstel en transactieveiligheid blijven noodzakelijk. [S03] [S11]

## 2. Actueel gecontroleerd checkpoint

### 2.1 GitHub

| Controle | Waargenomen resultaat |
|---|---|
| `main` | `52b3cb93441d02a73bd3d0c1f5105d9266c89f11` |
| Bronboom | `550805a7a0309bd257aa4cb5fee0fb3235b43971` |
| Branches | Alleen `main` in de opnieuw opgevraagde branchlijst |
| Open pull requests | Geen in de opnieuw opgevraagde lijst |
| Laatste relevante merge | PR #149, certificaatonderhoud en ticketrapportage |
| Voorafgaande relevante merge | PR #148, starten van de bestaande Base-worker met database-identiteitscontrole |
| Open issues, exclusief pull requests | **61**, opnieuw gecontroleerd via native GitHub-zoekopdracht; `incomplete_results=false` |
| Hoofdteststraat op deze `main` | Run **35320749453**; Rust, containers, web en specifications allemaal succesvol |

Bronnen: [S01] [S05] [S06] [S10] [S21] [S22]. De details van de testjobs zijn bij deze overdracht opnieuw gelezen. Dit vervangt de oudere melding dat de hoofdteststraat nog liep.

### 2.2 Railway-deployments

De native statuscontrole rapporteerde voor alle onderstaande nieuwste deployments **SUCCESS**. Dat beschrijft de deploymentstatus, niet een afzonderlijk uitgevoerde gebruikersacceptatie of doorlopende marktfeedtest.

| Service | Nieuwste gecontroleerde deployment | Status |
|---|---|---|
| `postgres` | `c75b3574-976e-4118-a4b4-ef8bf5fe498f` | SUCCESS |
| `web` | `bf84caa7-466d-4221-9e7b-86e07c90516d` | SUCCESS |
| `control-api` | `494c6457-050f-4446-a442-c8651510bf5e` | SUCCESS |
| `base-research-worker` | `ca4da5bd-3460-4878-a314-3cf1d0050bb8` | SUCCESS, met alleen gereedheidscontrole als ingestelde opdracht |

De oudere WAITING/BUILDING-meldingen voor deze deployments zijn hiermee achterhaald. De bestaande databasedeployment is niet door de twee laatste PR's vervangen. Native Railway-status en serviceconfiguratie zijn tijdens deze overdracht gelezen; zie de bronregistratie in hoofdstuk 12.

### 2.3 Werkelijke worker- en sessiestatus

De actuele ingestelde opdracht is:

```text
worker-entrypoint worker-readiness-check
```

De laatste opgehaalde gestructureerde inspectie is van **18 september 2026 om 08:03:26 UTC**, oftewel **10:03:26 Europe/Amsterdam**:

| Veld | Waarde |
|---|---|
| Inspectieresultaat | `WORKER_READINESS_INSPECTED` |
| Netwerk en modus | Base, `OBSERVE` |
| Oorspronkelijke sessie | `f8dfbc28-2b64-4346-93a9-207dd38393fe` |
| Waargenomen toestand | `RECOVERING` |
| Gewenste / toegepaste revisie | `0` / `0` |
| Actieve lease | `false` |
| Ingestion streams | `0` |
| Gestarte researchworker | `false` |
| Runtime gekwalificeerd | `false` |
| Providerrequests van deze inspectie | `0` |
| Databasecontrole | Alleen-lezen |

**Betekenis:** de registratie bestaat, maar een draaiende researchworker en gegevensstroom zijn nog niet aangetoond. De inspectie is een momentopname; gebruik haar niet als onbeperkt actuele database- of providerstatus.

De gemelde ontbrekende runtimevelden `ARB_WORKER_CONFIG`, `ARB_POOL_REGISTRY`, `ARB_SESSION_ID`, `ARB_BASE_INGESTION_STREAM` en `ARB_BASE_MANAGED_INGESTION` zijn geen nieuwe insteltaken voor de eigenaar. De launcher stelt deze afgeleid in op basis van het bestaande profiel, de oorspronkelijke sessie en de vaste streambinding. [S07]

### 2.4 Nog te verklaren CI-gategebeurtenis

In de opgehaalde logs van dezelfde deployment staat om **07:48:10 UTC** ook:

```text
CI_GATE_BLOCKED
reason=CI_WAIT_EXHAUSTED
execution_authorized=false
```

Daarna verschijnt om 08:03:26 UTC de gereedheidsinspectie. In de geretourneerde logselectie staat geen bijbehorend `CI_GATE_PASSED`-record. De hoofdteststraat is inmiddels wel afzonderlijk als succesvol gecontroleerd.

**Vervolgactie binnen #58:** reconcilieer de pre-deploypoging, exacte deploymentrevisie en latere inspectie voordat de launcher wordt geactiveerd. Een groene hoofdteststraat bewijst niet op zichzelf dat elke eerdere deploymentpoging de eigen gate heeft doorlopen. Uit deze beperkte logselectie volgt evenmin dat de gate is omzeild. Behoud de foutregistratie en zoek de feitelijke verklaring; schakel de controle niet uit om het verschil weg te werken.

## 3. Wat al is afgerond of gebouwd

### 3.1 Formeel geaccepteerde oorspronkelijke taken

**15 van 68 oorspronkelijke taken zijn geaccepteerd:** ARB-001 t/m ARB-014 en ARB-036. Hun native issues zijn #14 t/m #20, #22 t/m #28 en #50. Let op: ARB-nummers zijn niet hetzelfde als GitHub-issuenummers. EPIC-01 is afgerond; EPIC-02 heeft acht van negen kinderen afgerond. [S02] [S09]

Dit omvat de oorspronkelijke scope- en basisafspraken, repository/CI-basis, exacte hoeveelheden en identiteiten, onveranderlijke configuratie, opslag en commandologboek, workerlevenscyclus, geauthenticeerde API, scheduling/telemetrie, capturecontract en dashboardshell. Acceptatie van die onderdelen is geen brede kwalificatie van alle huidige providers, pools of live-uitvoering.

### 3.2 Belangrijke gemergede leveringen die niet opnieuw gebouwd moeten worden

| Levering | Relevante PR's / bron | Wat aanwezig is | Wat daarmee niet automatisch af is |
|---|---|---|---|
| Railway-basis | #95 | Web, API, PostgreSQL en deploymentbasis | Actieve gegevensstroom en operationele releaseacceptatie |
| Back-up/herstelbasis | #104; afgerond deelticket #103 | Gecontroleerde database-/captureback-up, integriteitscontrole en geïsoleerde herstelproef | Actuele productieback-up, beschermde bewaartermijnen en volledig applicatiebewust herstel |
| Echte Base-demonstratie | #129, #130 | Opgeslagen echte Base-observatie met beslissingen, bediening en replay; ook foutbewijs is bewaard | Continu draaiende gehoste feed of marktbrede kwalificatie |
| Base-ingestion | #131 t/m #136 | Eventdecodering, begrensde recovery, duurzame checkpoints, signaalafhandeling, filters en begrensde reconnects | Alle providersemantiek, tickdekking en volledige oorspronkelijke acceptatie |
| Account en navigatie | #135 | Normale accounttoegang; Paper trading / Real trading | Beschikbare echte handelsuitvoering |
| Broncontinuïteit | #139 t/m #143 | Duurzame invalidatie, bron-/capturebinding, beschermde publicatie, dashboardstatus en beheerde Base-inhaalstappen | Volledige versheidskwalificatie, alle Solana-rollbackregels of paper settlement |
| Workercontainer en controle | #144, #145 | Container, private volumecontrole, CI-gate en alleen-lezen gereedheidsinspectie | Gestarte researchworker |
| Base-profiel | #146 | Twee-poolprofiel voorbereid en gekoppeld aan gecontroleerde identiteit | Volledige quote- of simulatiekwalificatie |
| Sessie en API-profiel | #147 | Oorspronkelijke idempotente OBSERVE-registratie en overeenkomend API-profiel | Providerfeed of uitgevoerde START |
| Launcher | #148 | Bestaande sessie hergebruiken, expliciete eerste initialisatie, normale herstart zonder initialisatie, strikte database-TLS | Opgelost productiecertificaat of uitgevoerde gehoste start/stop/herstartproef |
| Certificaatonderhoud en backlograpport | #149 | Gecontroleerde prepare/apply/rollback-tool; automatische JSON/Markdown-ticketrapportage | Toegepaste productiereparatie, automatische certificaatvernieuwing of gesloten release-issue |

Historische test- en bronverwijzingen staan in #58 en de genoemde PR's. De historische echte Base-proef blijft geldig bewijs binnen haar oorspronkelijke scope en moet niet als 'nog nooit uitgevoerd' worden behandeld. Zij is geen bewijs voor de huidige gehoste worker. [S04] [S05] [S06] [S15] [S16]

### 3.3 Laatste twee PR's en testbewijs

**PR #148** is gemerged als `a5aac07ed9a4904859727f11525ac5c2c276f49c`. De launcher gebruikt de oorspronkelijke sessie. Normaal starten initialiseert niets; eerste broninitialisatie is een afzonderlijke expliciete handeling. Beide vormen versturen geen onderzoeks-START. De gevonden tekortkoming in database-identiteitscontrole is opgelost met `verify-full`, hostnaamcontrole en publieke CA-trust. De PR vermeldt 26 gerichte launchertests en geslaagde volledige CI. [S05] [S07]

**PR #149** is gemerged als `52b3cb93441d02a73bd3d0c1f5105d9266c89f11`, op **18 september 2026 om 07:42:45 UTC / 09:42:45 CEST**. De PR vermeldt 18 reparatietests, 10 rapportagetests en overige regressies. De echte geïsoleerde PostgreSQL-proef toont weigering van het ongeschikte certificaat, ongewijzigde actieve verbinding tijdens voorbereiding, toepassen plus herladen, succesvolle Rust-databasecontrole en gecontroleerd terugzetten. [S06]

De nieuwe hoofdteststraat **35320749453** is bij deze overdracht opnieuw gecontroleerd:

| Job | Job-ID | Resultaat |
|---|---:|---|
| Containers, inclusief certificaatonderhoudsproef | 105522475057 | Geslaagd |
| Specifications en scripts | 105522475301 | Geslaagd |
| Web en browsertests | 105522475326 | Geslaagd |
| Rust, PostgreSQL en HTTP-integratie | 105522475387 | Geslaagd |

De certificaatproef gebruikt geïsoleerde **PostgreSQL 17.11** en synthetische sleutels. Productie gebruikt **PostgreSQL 18**. Dit blijft een afzonderlijke compatibiliteits- en productiecontrole, niet een reden om productie te downgraden. De eerdere reviews zijn AI-ondersteund; zij gelden niet als de onafhankelijke uitvoeringsreview voor echte handel. In deze handoff-ronde zijn geen softwaretests opnieuw lokaal uitgevoerd: bestaande CI is gelezen en bronintegriteit is gecontroleerd. [S06] [S08] [S10]

## 4. Architectuur en bestaande koppelingen

### 4.1 Processen en broncode

```text
Browser: bestaande accountlogin en Paper/Real-werkruimten
                         |
                         v
                web -> control-api
                         |
                         v
                     PostgreSQL
                         ^
                         |
            research-worker / Base-ingestion
                         |
                         v
             bestaande Base-RPC en poolprofiel

Captures en profiel: afzonderlijk workervolume op /data
Replay: bestaande offline replay-applicatie
Solana: bestaande adapterbroncode, oorspronkelijke kwalificatie nog open
Live signer / verzending / live-reconciliatie: nog geen geleverde uitvoeringsketen
```

Dit is de bedoelde verbinding tussen bestaande componenten. De researchworker is in de gehoste configuratie nog niet actief. De bronboom bevat onder meer:

| Pad | Doel |
|---|---|
| `apps/control-api/` | Geauthenticeerde API en bediening |
| `apps/research-worker/` | Onderzoeksworker, levenscyclus en verzameling |
| `apps/evm-worker/` | Base/EVM-ingestion, waaronder `base-ingest` |
| `apps/solana-worker/` | Solana-workeronderdelen |
| `apps/replay/` | Replay |
| `apps/web/` | Bestaande React/TypeScript-frontend |
| `crates/arb-domain`, `arb-config`, `arb-control`, `arb-storage` | Exacte domeintypen, configuratie, bediening en opslag |
| `crates/arb-evm`, `arb-solana`, `arb-engine`, `arb-paper` | Adapters, routeberekening en virtuele boekhoudonderdelen |
| `crates/arb-capture`, `arb-registry`, `arb-scheduler`, `arb-adapter-api` | Herkomst, toegestane identiteiten, begrenzing en adaptercontract |
| `deploy/`, `.railway/railway.ts` | Container- en infrastructuurconfiguratie |
| `planning/backlog.json`, `planning/implementation-progress.json` | Oorspronkelijke criteria/afhankelijkheden en implementatiestatus |

De gecontroleerde Rust-toolchain is **1.90.0** uit `rust-toolchain.toml`. Gebruik de vastgelegde toolchain en lockfiles, niet de nieuwste compiler of pakketversies zonder wijzigingsreview. Geen frontendherbouw of overstap naar een nieuwe stack nodig. [S02] [S03] [S18] [S24]

### 4.2 Exacte Railway-resources

| Resource | Bestaande waarde |
|---|---|
| Project | `arbitrage-research` |
| Project-ID | `c7cea88d-24b8-45dc-9602-7c0c272f1da7` |
| Environment | `production` |
| Environment-ID | `5ff4044a-b0ac-4b02-b9cd-888cd91a3fee` |
| Regio | `europe-west4-drams3a` |
| PostgreSQL-service | `ff012117-92dc-433f-a2a3-ed61f3f33ce9` |
| Web-service | `1546cbbd-22a8-42af-9a1b-aeaa64ed57e7` |
| API-service | `3b576798-60e3-474b-abe9-a82779c219ab` |
| Base-workerservice | `6b546a9d-ebd1-4443-9526-fbd61acf96e6` |
| Databasevolume | `744e7772-2b2c-4998-92f4-dabc529b0d9c` |
| Databasemount | `/var/lib/postgresql/data`, geconfigureerd op 50.000 MB |
| Workervolume | `8e809a27-c9f5-4658-bbea-3f2611d7b11a` |
| Workermount | `/data`, geconfigureerd op 1.024 MB |
| Bestaand database-image | `ghcr.io/railwayapp-templates/postgres-ssl:18` |
| Databasehost | `postgres.railway.internal` |
| Certificaatdirectory | `/var/lib/postgresql/data/certs` |
| Gedocumenteerde PGDATA | `/var/lib/postgresql/data/pgdata`; verifieer vóór onderhoud in de doelcontainer |

**Actuele workersettings, opnieuw uit Railway gelezen:**

```text
source.repo=makafeli/arbitrage-research
source.branch=main
source.checkSuites=false
build.dockerfilePath=deploy/Dockerfile.worker
startCommand=worker-entrypoint worker-readiness-check
preDeployCommand=python3 -I /usr/local/lib/arb/worker_ci_gate.py
restartPolicyType=NEVER
configuredReplicasInRegion=1
```

De native GitHub-wachtschakelaar is dus niet ingeschakeld; de afzonderlijke pre-deploycontrole is wel geconfigureerd. De laatste gatepoging moet nog worden gereconcilieerd zoals beschreven in 2.4. Een geconfigureerde replica betekent niet dat het eindige readiness-proces als doorlopende researchworker actief is. Behoud deze instellingen totdat een expliciete, gecontroleerde deploymentwijziging plaatsvindt.

Volumegroottes zijn configuratie, geen gemeten gebruik. Een onveranderlijke digest van het werkelijk gebruikte database-image is niet in deze overdracht vastgesteld; wijzig het image niet tijdens de certificaatreparatie. [S04] [S08]

### 4.3 Profiel, sessie en grenzen

| Onderdeel | Bestaande waarde |
|---|---|
| Profieldirectory | `/data/runtime/base-v1` |
| Configuratiebestand | `/data/runtime/base-v1/configuration.toml` |
| Workerregistry | `/data/runtime/base-v1/registry.json` |
| Ingestionregistry | `/data/runtime/base-v1/ingestion-registry.json` |
| Sessiemodus | `OBSERVE`, netwerk `base-mainnet` |
| Sessienummer | `f8dfbc28-2b64-4346-93a9-207dd38393fe` |
| Configuratiedigest | `sha256:9792c6abb2e19a5b1c8cd1ee76157acf99d5be7d744bc80f150d7d34d98bec11` |
| Vaste bron-ID voor de launcher | `railway-base-profile-v1` (sinds #193: `.g<N>` per generatie) |
| Aantal voorbereide pools | 2 |
| Voorbereide kandidaatgrootte | 1 USDC, onderzoeksinvoer; geen gefinancierd handelsbedrag |
| Capturequota van het profiel | 128 MiB, afzonderlijk van de totale volumesize |
| Gedocumenteerde RPC-spacing | 75 ms; bestaande request-, tijd- en bytebudgetten blijven gelden |
| Begrensde Base-inhaalstap | Maximaal 16 blocks binnen het bestaande contract (sinds #187: 32 blokken, zie docs/BASE-LOG-RECOVERY.md) |

Controleer exacte pooladressen, ABI- en contractidentiteiten in de originele profielbestanden en registry. Reconstrueer ze niet uit tickers of een willekeurige actuele poollijst. [S04] [S07] [S16]

### 4.4 Instellingen die niet opnieuw bij de eigenaar hoeven te worden opgevraagd

De actuele workerconfiguratie bevat de volgende namen. Geheimen zijn opzettelijk niet opgenomen:

| Naam | Behandeling |
|---|---|
| `ARB_BASE_RPC_URL` | Bestaand geheim. Behouden; niet uitschrijven in chat, logs, repository of artefacten. |
| `ARB_DATABASE_URL`, `ARB_INGEST_DATABASE_URL` | Bestaande databaseverbindingen. Geen nieuwe database of account maken. |
| `ARB_DATABASE_CA_PEM` | Bestaande gedeelde publieke CA met workerreferentie. In PR149 ingesteld en naam opnieuw teruggelezen. |
| `ARB_BASE_PROFILE_DIGEST` | Bestaande profielverankering behouden. |
| `ARB_OPERATOR_ID`, `ARB_INGEST_OPERATOR_ID` | Bestaande operatorscope behouden. |
| `ARB_RPC_MIN_INTERVAL_MS` | Bestaande begrenzing behouden. |
| `RAILWAY_DOCKERFILE_PATH` | Bestaande workerbuild behouden. |

De launcher vertaalt publieke CA-trust naar `PGSSLROOTCERT` voor de gebruikte SQLx-processen. Een losse `worker-session`-aanroep moet dezelfde geverifieerde trust ontvangen. Pas de omgevingsconventies van SQLx niet blind toe op andere clients. De private CA- of serversleutel hoort nooit in servicevariabelen. [S07] [S08]

De normale accountlogin is eerder door de eigenaar bevestigd als werkend. Er is tijdens deze overdracht geen nieuwe inlogproef of credentialreset uitgevoerd.

## 5. Tickets en voortgang

### 5.1 Betrouwbare telling

| Categorie | Open |
|---|---:|
| Oorspronkelijke ontwikkeltaken | 53 |
| Parent epics | 7 |
| Apart toolingticket #107 | 1 |
| **Totaal native issues, exclusief PR's** | **61** |

| Resterende oorspronkelijke taken | Aantal |
|---|---:|
| Initiële onderzoekssoftware Base en Solana, M1-M4 | 29 |
| Meetcampagne, beoordeling en besluit, M5 | 6 |
| Livevoorbereiding, pilot, uitbreiding en onderhoud, M6-M7 | 18 |
| **Totaal** | **53** |

| Implementatiestatus over alle 68 oorspronkelijke taken | Aantal |
|---|---:|
| Geaccepteerd / afgerond | 15 |
| Implementatie aanwezig, oorspronkelijke acceptatie open | 13 |
| In uitvoering | 8 |
| Gepland | 32 |

De 29 taken voor de initiële software bestaan uit 13 met implementatie, 8 in uitvoering en 8 geplande taken. 'Implementatie aanwezig' kan nog inhoudelijke integratie, foutafhandeling of kwalificatie vereisen. Deze aantallen zijn geen inspannings- of voltooiingspercentages. Een epic telt zijn kinderen samen en is geen extra los bouwproject. [S02] [S04] [S09]

De detailvergelijking tussen native issues en register is afkomstig uit de volledige, niet-atomaire snapshot van **07:30:43-07:30:44 UTC** in Delivery review **35319756677**. Die had nul statusverschillen. Tijdens het maken van deze handoff is het totaal van 61 opnieuw native gecontroleerd; de bronboom en het register zijn ongewijzigd. Er wordt geen nieuwe atomaire controle van alle individuele issues geclaimd. [S06] [S21]

### 5.2 Waarom veel PR's geen gesloten tickets opleveren

De laatste PR's leveren deelgedrag binnen grotere oorspronkelijke taken. #58 omvat bijvoorbeeld meer dan een launcher: ook deploymentgedrag, herstel, backups en afhankelijkheden van de onderzoeksrelease. Code-integratie is daarom niet automatisch volledige issueacceptatie.

De belangrijke maatstaf voor de volgende ronde is een **aantoonbaar bruikbare keten**: gegevens komen binnen, bediening werkt en resultaten zijn herleidbaar. Niet het aantal gewijzigde bestanden of uitsluitend de totale issue-teller.

### 5.3 Telling richting echte handel volgens het huidige plan

De oorspronkelijke afhankelijkheidsgraaf bevat **46 nog open voorafgaande taken voor #76**, plus **#76 zelf** voor de uitvoering en afronding van de eerste livepilot: **47 open taken tot en met die pilot**. Dit is rechtstreeks nagerekend uit `backlog.json` en het huidige register, niet uit alleen de issuevolgorde.

De overige zes oorspronkelijke taken zijn #77 t/m #82, voor evaluatie, uitbreiding en onderhoud. #77 is relevant voor doorgaan ná de pilot; #82 bevat onderhouds- en afsluitverantwoordelijkheden. Hun plaats buiten de directe afhankelijkheidsketen betekent niet dat incidentafhandeling of beheer vóór de pilot genegeerd mag worden.

Een goedgekeurde Base-only release kan deze scope verkleinen. Noem geen nieuw exact minimum voordat de gedeelde afhankelijkheden en acceptatiecriteria formeel zijn herschreven. [S02] [S03]

## 6. Blokkades en operationele aandachtspunten

| Onderwerp | Huidige grens | Concrete vervolgactie | Eigenaar / ticket |
|---|---|---|---|
| Productiecertificaat PostgreSQL | Laatst gecontroleerde servercertificaat heeft `CA:TRUE`; huidige worker vereist strikte serveridentiteitscontrole. | Bestaande reparatie gecontroleerd toepassen, herladen en echte Rust-databaseverbinding bewijzen. | Beheerder + integrator, #58 |
| Productieherstelpunt en uitvoering | In eerdere native capabilitycontrole geen bruikbare container-exec, SQL/reload of backupfunctie via de ChatGPT-koppeling vastgesteld. | Bestaande bevoegde Railway-SSH/CLI-beheersessie en verifieerbare herstelroute gebruiken; mogelijkheden opnieuw vaststellen in de uitvoerende omgeving. | Beheerder, #58 |
| CI-gatebewijs | Laatste logselectie bevat `CI_WAIT_EXHAUSTED`, daarna een inspectie; hoofd-CI is geslaagd. | Pogingen en exacte revisie reconcilieren vóór activatie. Geen bypass. | Integrator, #58 |
| Bron en worker | Originele sessie bestaat; laatste inspectie heeft nul streams en geen lease. (sinds #193: na een gemelde HALT selecteert `ARB_BASE_GENERATION` een nieuwe stream- en sessiegeneratie zonder de HALTED bron te resetten, zie docs/BASE-WORKER-LAUNCH.md §Generation) | Alleen een werkelijk ontbrekende passende bron initialiseren; bij generatie ≥ 2 registreert de launcher een nieuwe sessie in plaats van op de bestaande te starten. | Base / operations, #30, #32, #58 |
| Broncontinuïteit bij herstart | 32-blokgrens (sinds #187, eerder 16; zie docs/BASE-LOG-RECOVERY.md) en terminale HALT-regels blijven actief. (sinds #197: een providerfout tijdens de begrensde inhaalstap is geen terminale HALT meer, het ongewijzigde checkpoint wordt bij de volgende poging opnieuw geprobeerd, zie docs/BASE-LOG-RECOVERY.md) (sinds #202: de ranged `eth_getLogs`-call per stap wordt in maximaal 10-blok-stukken opgeknipt omdat de provider's free tier bredere spans afwees met `RPC HTTP error: bad request`, zie docs/BASE-LOG-RECOVERY.md) | Oud of HALTED sourcecheckpoint niet resetten. Bewijs het gedocumenteerde herstelpad of rapporteer een echte beperking. | Base / platform, #30, #32 |
| Volledige paperketen | Er zijn boekhoudprimitieven en quotes, maar geen complete automatische keten. | Exacte plannen, complete simulatie, scenarios, kosten en idempotente settlement integreren. | Engine / chain, #39-#49 |
| Live-uitvoering | Nog geplande aparte laag. | Scope, signer, limieten, journaling, verzending, reconciliatie en onafhankelijke review uitvoeren. | #64-#76 |
| Certificaatlevenscyclus | Reparatie is eenmalig; de originele image-template is niet gerepareerd. | Heruitgifte en behoud van trust vóór de beschreven vernieuwingstermijn regelen en testen. | Operations, #58 / later onderhoud |

Het verschil tussen een platformfunctie en een aangesloten tool is essentieel. Railway documenteert SSH en volumeback-ups; dat bewijst niet dat een specifieke agent die mag uitvoeren of dat er al een backup bestaat. Een pre-deploycontainer heeft geen gemounte volumes en kan deze certificaatbestanden dus niet repareren. PostgreSQL ondersteunt certificaatherladen zonder volledige serverherstart. Controleer de daadwerkelijk aangeboden servercertificaatketen, niet alleen een bestandswijziging. [S08] [S25] [S26] [S27] [S28]

**Bekende publieke DER SHA256-referenties**, gedateerd op de eerdere geauthenticeerde certificaatcontrole:

```text
ROOT_CA_DER_SHA256=5385eb665eebc8edbd62aeb609319501ee1f788c5dd527b0b1f495e5f948e3f5
ORIGINAL_SERVER_DER_SHA256=998b112f74c3a11b86cad451a2b7a757901f14c0b1a5b473b5b03977be3881a7
```

Een afwijkende actuele fingerprint is een reden voor onderzoek, geen toestemming om de nieuwe waarde zonder verificatie als vertrouwd aan te nemen. De server- en rootcertificaten zijn voor deze handoff niet opnieuw uit productie uitgelezen; hun eerdere eigenschappen staan in #58 en de onderhoudshandleiding. Private sleutelbytes zijn niet in dit bestand opgenomen. [S04] [S08]

## 7. Uitvoeringsvolgorde en acceptatie per werkpakket

Deze werkpakketten zijn een uitvoeringsindeling van bestaande taken, geen nieuwe GitHub-issues. Ze geven de aanbevolen volgorde en het bewijs dat bij elke stap hoort. Binnen de oorspronkelijke scope kan afgebakende ontwikkeling tegen bestaande contracten parallel doorgaan, ook wanneer een andere acceptatiepoort nog openstaat. Dat is geen toestemming om die poort bij releasesluiting over te slaan. [S12]

### 7.0 Begin elke nieuwe uitvoeringssessie met reconciliatie

**Ticket:** bestaande #58 voor de huidige operationele voortzetting.

Lees `AGENTS.md`, `GOAL.md`, `CONTRIBUTING.md`, oorspronkelijke issuecriteria, het voortgangsregister, de laatste PR's, reviewbevindingen en CI. Controleer actuele `main`, Railway-configuratie en logs. Gebruik hoofdstuk 2 als uitgangspunt, niet als eeuwig actuele waarheid.

Controleer actieve werkclaims voordat code wordt gewijzigd. Leg exact ticket, branch, basis-SHA, letterlijke bestandspaden, testplan en deliverable vast. Behoud het bestaande authenticatie- en resourcepad. Ga niet eerst een nieuw account, nieuw profiel of nieuwe repository bouwen.

**Klaar wanneer:** de uitvoerder de werkelijke basis, bevoegdheden, niet-overlappende claim en eerstvolgende concrete levering heeft vastgelegd.

### 7.1 Databaseonderhoud afronden en verbinding bewijzen

**Essentieel:** #58. **Doel:** een werkelijk geverifieerde databaseverbinding voor de bestaande worker.

1. Reconcilieer de CI-gategebeurtenis en controleer de exacte test-/deploymentrevisie.
2. Verkrijg een bevoegde bestaande onderhoudsverbinding. Leg vast welke uitvoermogelijkheid werkelijk beschikbaar is; vraag geen private keys of databasewachtwoord in chat.
3. Verifieer een actueel herstelpunt en terugvalroute voor de juiste database en het volume. Certificaatrollback vervangt geen databasebackup. Registreer identiteit, tijdstip, scope en herstelbeperking van de backup.
4. Inspecteer doelservice, PGDATA, image, file owner, openbare certificaatpins en bestaande onderhoudsstaging. Houd de researchlauncher inactief. Toets eventuele verschillen tussen de PostgreSQL 17.11-test en productie 18 in een passende geïsoleerde omgeving.
5. Gebruik exact de gereviewde bytes van `scripts/postgres_leaf_repair.sh`. Het bestaande onderhoud bestaat uit `--prepare`, afzonderlijke inspectie, `--apply` en eventueel `--rollback`. Geen remote download direct naar een shell pipen.
6. Laat PostgreSQL via de bevoegde beheersessie herladen. De reparatietool zelf verstuurt bewust geen reload, SQL, restart of workercommando.
7. Vergelijk de werkelijk aangeboden peerfingerprint met de voorbereide kandidaat en voer vanuit de worker de bestaande, alleen-lezen `worker-session --status /data/runtime/base-v1` uit met de juiste CA-trust. De oorspronkelijke sessie en digest moeten terugkomen.
8. Leg ongewijzigde databaseprocesstart, applicatiestatus en behoud van sleutels/CA vast zonder sleutelbytes te exporteren. Bij mislukking de gedocumenteerde terugzetroute gebruiken, de oorspronkelijke peer en servicegezondheid verifiëren en de worker inactief houden.

**Definitie van klaar:** geverifieerd herstelpunt; passende certificaatketen en hostnaam; succesvolle verbinding met de verscheepte Rust-verifier; oorspronkelijke sessie behouden; productiegegevens niet gemigreerd of gereset; bewijs in #58.

De exacte commando-argumenten en paden staan in [S08]. De nieuwe leaf heeft volgens de tool een looptijd van 90 dagen; de automatische vernieuwingsroute is niet gerepareerd. Neem het vervolgbeheer mee voordat dit als duurzame productie-inrichting wordt geaccepteerd. Een rollback herstelt het oude, ongeschikte certificaat: dat is herstel van de uitgangssituatie, geen succesvolle hardening.

**Bij geblokkeerde beheerstoegang:** leg de exacte beperking en uitvoerder vast, lever de reeds bestaande onderhoudsinstructie over en ga verder met inhoudelijke, geïsoleerd testbare ontwikkeltaken. Maak niet opnieuw een certificaatscript of alleen extra statusdocumentatie.

### 7.2 De bestaande Base-worker aan de gegevensbron koppelen

**Essentieel:** #58, #30, #32. **Voorwaarde:** 7.1 en de toepasselijke CI-controle geslaagd.

Controleer vlak vóór initialisatie opnieuw of de passende bron werkelijk ontbreekt. Een oudere telling van nul streams is hiervoor geen blijvend bewijs. Controleer de oorspronkelijke profielbestanden en volledige registrybinding. Behoud de oorspronkelijke sessie, operator, digest, capturequota en RPC-budgetten.

De bestaande, eenmalige launcheropdracht uit het runbook is:

```text
worker-entrypoint worker-launch-base --initialize-and-start
```

**Deze opdracht is alleen voor de expliciete eerste broninitialisatie.** Een onderbroken initialisatie heeft mogelijk al geschreven. Inspecteer dan de opgeslagen toestand; herhaal niet blind en reset/rearm geen bron.

Na bevestigde initialisatie is de normale opdracht voor latere starts:

```text
worker-entrypoint worker-launch-base --start
```

Beide opdrachten starten het proces, maar sturen **geen onderzoeks-START**. Het bestaande workercontract herstelt naar `STOPPED`, verzamelt begrensde gereedheidsinvoer en wacht op de normale geauthenticeerde bediening. De huidige Railway-opdracht is nog de readiness-check; pas alleen de bedoelde service aan. De IaC bevat al de normale launcheropdracht. Een volledige ongerichte IaC-apply kan dus onbedoeld andere wijzigingen uitvoeren. [S07]

**Te bewijzen:** werkelijke lease/heartbeat, oorspronkelijke sessie, passend broncheckpoint, opgeslagen echte captures met block/hash/herkomst en meer dan één opeenvolgende geslaagde verzamelstap binnen een vastgelegd meetvenster. Een rustige markt, afwijzing of negatieve route blijft een geldig resultaat; verzin geen winst om activiteit te tonen.

**Definitie van klaar:** de Base-flow levert herleidbare actuele onderzoekswaarnemingen, providerfouten zijn zichtbaar en de bestaande begrenzingen blijven gelden. Een actieve stream alleen bewijst nog geen volledige quote- of simulatiekwalificatie.

**Herstartrisico:** een verouderd checkpoint kan de 16-blockgrens overschrijden. (sinds #187: 32 blokken, zie docs/BASE-LOG-RECOVERY.md; sinds #197: een providerfout tijdens de inhaalstap is geen terminale HALT meer, het checkpoint blijft staan en wordt bij de volgende poging opnieuw geprobeerd; sinds #204: `worker-entrypoint worker-launch-base --rotate-and-start` laat een ACTIEVE stream doorlopen in de volgende generatie vóór de retentiegrens van 4096 batches, zie docs/BASE-WORKER-LAUNCH.md) Dat is geen reden voor een automatische reset of het verhogen van de grens. Test het toegestane herstelpad en maak een echte ontbrekende herstelmogelijkheid expliciet. [S16]

### 7.3 Gehoste bediening en dashboard bewijzen

**Essentieel:** #51, #53, #58; bestaande contracten hergebruiken.

Voer één aantoonbare ketentest uit: oorspronkelijke sessie lezen, geauthenticeerde START indienen, `PENDING` onderscheiden van worker-`APPLIED`, echte waarnemingen tonen, STOP toepassen en herstart naar `STOPPED` controleren. Test ook vertraagde acknowledgements, disconnect en veilige idempotente herhaling. Neem PAUSE/RESUME en gedeeltelijke stop-all-uitkomsten mee volgens de oorspronkelijke criteria.

Het dashboard moet een ontbrekende feed, verouderde gegevens en nul kansen onderscheiden. Een demoactie of infrastructuurstop mag niet worden voorgesteld als een door de worker toegepaste stopbarrière.

**Definitie van klaar voor oplevering A:** de gebruiker kan de echte onderzoeksflow zien en bedienen, en de vastgelegde herstartproef behoudt sessie-/bronidentiteit zonder automatisch hervatten. Registreer trace-ID's, commandorevisies, observaties en daadwerkelijk waargenomen resultaten in de bestaande tickets.

### 7.4 Adapter-, snapshot-, reken- en routekwalificatie afronden

**Essentieel:** #30-#38 en #29, volgens de oorspronkelijke afhankelijkheden.

De inhoudelijke volgorde is: gevalideerde pool/accountinvoer, consistente snapshots, exacte quote-aritmetiek, aantoonbaar toegestane capabilities, routevorming, duurzame beslissporen en de geïntegreerde kwalificatietest. Veel implementatie bestaat al; richt tests en fixes op de expliciete resterende criteria.

Voor Base: werkelijke contract-/ABI-identiteiten, voldoende ticks voor het ondersteunde bereik, exact block/hashverband, ontbrekende logs, reconnects, rollback en rekenkundige grensgevallen. Voor Solana: eigenaarschap/programidentiteit, coherente accountcontext, tick arrays, actuele programma-equivalentie en ondersteund token-/feegedrag.

**Definitie van klaar:** echte representatieve invoer en protocolreferenties ondersteunen de gedeclareerde mogelijkheden; stale/incomplete/inconsistente toestand wordt afgewezen; alle oorspronkelijke dependency- en reviewcriteria zijn per ticket afgehandeld. `NO_KNOWN_INVALIDATION` is alleen continuïteitsinformatie, geen bewijs van versheid of uitvoerbaarheid. [S03] [S16] [S17]

De Base-demonstratie #29 kan niet automatisch EPIC-02 afsluiten zolang haar oorspronkelijke gezamenlijke afhankelijkheden openstaan. Een eventuele Base-only acceptatie moet eerst formeel zijn afgesproken.

### 7.5 Volledige paper-tradingketen bouwen en integreren

**Essentieel:** #39-#49, met geldige quote-/routeinvoer uit het vorige werkpakket.

| Stap | Tickets | Vereiste levering |
|---|---|---|
| Kosten en waardering | #39 | Exacte, herleidbare kosten; ontbrekende kosten blijven onbekend. Poolfees niet dubbel aftrekken. |
| Virtueel kapitaal | #40 | Hoofdsom en native fees apart reserveren; concurrentie, release en journalreplay correct. |
| Replay | #41 | Oorspronkelijke captures, tijd en versies; geen actuele RPC-data gebruiken om historische hiaten te vullen. |
| Compleet Base-plan en simulatie | #42, #44 | Beide swaps en guards in één exact plan; complete execution testen met expliciet virtuele funding en allowances. |
| Compleet Solana-plan en simulatie | #43, #45 | Volledige instructiereeks/accountset, guards, compute/fees en expliciete simulatieaannames. |
| Vertraging en uitkomsten | #46 | Benoemde delay-/inclusion-/failure-scenarios; ontbrekende toekomstige toestand apart registreren. |
| Uitvoerbaarheid en settlementintegratie | #47, met #39, #40, #44, #45, #46 | Exact-planbewijs en alle predicates controleren; de virtuele boekhouding idempotent aan scenario-uitkomsten koppelen. |
| Vergelijking en releasepoort | #48, #49 | Vergelijkbare datasets, correcte claims, volledige ketentest en oorspronkelijke financiële review. |

**Automatische virtuele afwikkeling is nog geen afgeronde functie.** Koppel reservering, simulatie, scenario-uitkomst en terminale boeking zo dat een herstart of retry geen dubbele fill, fee of vrijgave veroorzaakt. Bewaar negatieve, afgewezen en onzekere uitkomsten. De bestaande interne boekhoudmethoden zijn geen compleet paperhandelsproduct.

**Definitie van klaar voor oplevering B:** één complete ondersteunde route per chain simuleert volgens het oorspronkelijke releasecontract; exacte kosten en virtuele reserveringen werken; falende en gewijzigde plannen worden geweigerd; dezelfde invoer reproduceert dezelfde uitkomst; dashboard en export tonen uitsluitend de bijbehorende bewijsklasse.

De oorspronkelijke bewijsklassen blijven behouden: `CANDIDATE` voor lokale quote-/replayberekening, `SIMULATED` pas na complete geslaagde plansimulatie, `ESTIMATED_EXECUTABLE` pas na alle extra voorwaarden en nooit `REALIZED` vanuit PAPER of REPLAY. Een hypothetische winst is geen echte opbrengst. [S03] [S11]

### 7.6 Gebruiks-, monitoring- en herstelcriteria afronden

**Essentieel voor de oorspronkelijke onderzoeksrelease:** #52-#58, naast #51 en de paper-releasepoort #49.

Bevestig onveranderlijke experimentinstellingen, correcte saldi/historie, exacte en geredigeerde exports, zichtbare capturebeschikbaarheid, bruikbare alarmen en behoud van bediening onder belasting. Test toetsenbord/nauw scherm, fouten en focusgedrag daadwerkelijk; statische markup of screenshots alleen sluiten de volledige toegankelijkheidstaak niet.

Voer applicatiebewust herstel uit in een geïsoleerde omgeving: configuraties, sessies, commando's, ledger en capture-/replayverwijzingen moeten consistent terugkomen. Leg bewaartermijnen, verlopen ruwe data, backupbeveiliging en operationele eigenaren vast.

**Definitie van klaar:** de originele M4-criteria zijn onderbouwd en de onderzoeksrelease kan gebruikt en hersteld worden zonder verborgen signer- of verzendmogelijkheden. Monitoring en herstel mogen parallel worden ontwikkeld, niet pas na de livepilot worden toegevoegd. [S03] [S04] [S11]

### 7.7 Onderzoeksresultaat en go/no-go vastleggen

**Essentieel volgens het huidige plan:** #59 t/m #64.

Leg vóór de primaire evaluatiedataset vast welke pools, bedragen, virtueel kapitaal, kosten, scenarios, uitsluitingen, dekking en stopvoorwaarden gelden. Houd verkennend werk en evaluatie gescheiden. Verzamel werkelijke waarnemingen, registreer uitval, controleer replay-/boekhoudsteekproeven en beoordeel de resultaten en operationele gereedheid.

**Definitie van klaar:** reproduceerbare onderzoeksresultaten en een expliciet eigenaarsbesluit in #64 over stoppen, verder onderzoeken of één begrensde live-strategie voorbereiden. Geslaagde synthetische tests kunnen echte meetdekking niet vervangen. Dit is geen automatisch verstrijkende kalenderpoort en geen gegarandeerde winsttest. [S03] [S11]

### 7.8 Live-uitvoeringslaag ontwikkelen en laten beoordelen

**Essentieel voor echte handel:** #65 t/m #75, na de toepasselijke besluiten en binnen de vastgelegde scope.

| Tickets | Vereiste functie |
|---|---|
| #65 | Exacte chain/strategie, bevoegdheden, custody, kapitaal-, fee-, verlies- en pendinglimieten vastleggen. |
| #66, #67 | Afzonderlijke begrensde signer en geharde uitvoeringsartefacten; geen willekeurige transacties ondertekenen. |
| #68 | Werkelijke saldo-/allowancecontrole, aparte gasreserves en concurrentieveilige risicoreserveringen. |
| #69, #70 | Duurzame intentie, beschermd ondertekend payloadrecord en dispatch-startstatus vóór elke verzending; chain-specifieke transportregels. |
| #71, #72 | Pending/unknown/failure/finality, balans- en kostenreconciliatie, fencing en herstel zonder dubbele verzending. |
| #73 | Expliciet activeren, stoppen en intrekken van signbevoegdheid; gedeeltelijke of vertraagde acknowledgements zichtbaar. |
| #74 | Werkelijke onafhankelijke uitvoeringstechnische review, remediatie en regressiebewijs op de exacte doelversie. |
| #75 | Controleerbaar pilotpakket, geïsoleerde oefening en definitieve operatorgoedkeuring voor exacte versie en limieten. |

De huidige researchbuild mag niet stilzwijgend een signer of verzendpad krijgen. Ontwikkeling van de latere uitvoeringslaag moet expliciet worden afgebakend; gebruik van echte sleutels, productie-uitvoeringsartefacten, financiering en activering wachten op hun eigen goedkeuring. Agentzelfreview, CodeRabbit of een groene CI-run vervangen #74 niet.

**Definitie van klaar:** de daadwerkelijk te gebruiken keten, code, sleutellocatie, limieten en recovery zijn getest, onafhankelijk beoordeeld en door de eigenaar goedgekeurd. Nog geen onbeperkte handel of automatische uitbreiding. [S03] [S11]

### 7.9 Begrensde pilot, evaluatie en onderhoud

**#76** voert de expliciet goedgekeurde pilot uit. Werkelijke chaintransacties, saldomutaties en alle unknown/pending-uitkomsten moeten worden vergeleken met het duurzame logboek. Handhaaf de afgesproken kapitaal- en kostenlimieten. Financiering en activering zijn afzonderlijke concrete operatorhandelingen.

**#77** beoordeelt daarna doorgaan, aanpassen, verkleinen of stoppen. **#78-#81** zijn vervolgscope voor extra venues, tokens/routes, chains en fundingadapters. **#82** borgt onderhoud en veilige afsluiting. Geen automatische opschaling na één geslaagde transactie. [S03]

### 7.10 Praktische parallelisering

Zolang toegang voor #58 blokkeert, kan het team bestaande restcriteria in Base/Solana, rekentests, complete plan-/simulatieontwikkeling en dashboardintegratie aanpakken. Kies afgebakende paden en bestaande contracten; accepteer geen complete downstream-release voordat de oorspronkelijke voorwaarden zijn vervuld.

Gebruik echte parallelle ontwikkelaars of daadwerkelijk beschikbare agents, met aparte branches/worktrees en één integrator. Planningcapaciteit, een CI-matrix of de configuratie uit #107 is niet hetzelfde als draaiende agents. Laat een toolingprobleem de bruikbare Base-flow niet opnieuw blokkeren. [S12] [S23]

## 8. Volledige lijst resterende oorspronkelijke taken

De volgende tabellen zijn afgeleid uit de oorspronkelijke backlog en het register van de gecontroleerde bronboom. Titels zijn Nederlands samengevat; de originele issuecriteria zijn leidend. **Open voorafgaande taken** toont alleen directe oorspronkelijke afhankelijkheden die volgens dat register nog niet zijn geaccepteerd. Een lege kolom betekent niet dat alle eigen criteria al zijn bewezen. [S02] [S03]

### M1: resterende platformintegratie

| Issue | Taak-ID | Onderwerp | Registerstatus | Open voorafgaande taken |
|---|---|---|---|---|
| [#29](https://github.com/makafeli/arbitrage-research/issues/29) | ARB-015 | Eerste geïntegreerde observatie- en bedieningsproef | In uitvoering | [#30](https://github.com/makafeli/arbitrage-research/issues/30), [#32](https://github.com/makafeli/arbitrage-research/issues/32), [#36](https://github.com/makafeli/arbitrage-research/issues/36), [#37](https://github.com/makafeli/arbitrage-research/issues/37) |

### M2: adapters, snapshots en routes

| Issue | Taak-ID | Onderwerp | Registerstatus | Open voorafgaande taken |
|---|---|---|---|---|
| [#30](https://github.com/makafeli/arbitrage-research/issues/30) | ARB-016 | Base Uniswap V3-invoer | Implementatie aanwezig | Geen open directe afhankelijkheden |
| [#31](https://github.com/makafeli/arbitrage-research/issues/31) | ARB-017 | Solana Orca-invoer | Implementatie aanwezig | Geen open directe afhankelijkheden |
| [#32](https://github.com/makafeli/arbitrage-research/issues/32) | ARB-018 | Consistente snapshots, versheid en rollback | Implementatie aanwezig | [#30](https://github.com/makafeli/arbitrage-research/issues/30), [#31](https://github.com/makafeli/arbitrage-research/issues/31) |
| [#33](https://github.com/makafeli/arbitrage-research/issues/33) | ARB-019 | Exacte Uniswap V3-quotekwalificatie | Implementatie aanwezig | [#30](https://github.com/makafeli/arbitrage-research/issues/30), [#32](https://github.com/makafeli/arbitrage-research/issues/32) |
| [#34](https://github.com/makafeli/arbitrage-research/issues/34) | ARB-020 | Exacte Orca-quotekwalificatie | Implementatie aanwezig | [#31](https://github.com/makafeli/arbitrage-research/issues/31), [#32](https://github.com/makafeli/arbitrage-research/issues/32) |
| [#35](https://github.com/makafeli/arbitrage-research/issues/35) | ARB-021 | Gekwalificeerde adaptermogelijkheden en activering | In uitvoering | [#33](https://github.com/makafeli/arbitrage-research/issues/33), [#34](https://github.com/makafeli/arbitrage-research/issues/34) |
| [#36](https://github.com/makafeli/arbitrage-research/issues/36) | ARB-022 | Begrensde routes door verschillende pools | Implementatie aanwezig | [#33](https://github.com/makafeli/arbitrage-research/issues/33), [#34](https://github.com/makafeli/arbitrage-research/issues/34), [#35](https://github.com/makafeli/arbitrage-research/issues/35) |
| [#37](https://github.com/makafeli/arbitrage-research/issues/37) | ARB-023 | Beslissporen, afwijzingen en deduplicatie | Implementatie aanwezig | [#32](https://github.com/makafeli/arbitrage-research/issues/32), [#36](https://github.com/makafeli/arbitrage-research/issues/36) |
| [#38](https://github.com/makafeli/arbitrage-research/issues/38) | ARB-024 | Kwalificatie en foutproeven op beide chains | Gepland | [#29](https://github.com/makafeli/arbitrage-research/issues/29), [#33](https://github.com/makafeli/arbitrage-research/issues/33), [#34](https://github.com/makafeli/arbitrage-research/issues/34), [#35](https://github.com/makafeli/arbitrage-research/issues/35), [#37](https://github.com/makafeli/arbitrage-research/issues/37) |

### M3: volledige paper- en replayketen

| Issue | Taak-ID | Onderwerp | Registerstatus | Open voorafgaande taken |
|---|---|---|---|---|
| [#39](https://github.com/makafeli/arbitrage-research/issues/39) | ARB-025 | Volledige kosten en reproduceerbare waardering | Implementatie aanwezig | [#37](https://github.com/makafeli/arbitrage-research/issues/37) |
| [#40](https://github.com/makafeli/arbitrage-research/issues/40) | ARB-026 | Virtuele portefeuilles en reserveringen | Implementatie aanwezig | [#39](https://github.com/makafeli/arbitrage-research/issues/39) |
| [#41](https://github.com/makafeli/arbitrage-research/issues/41) | ARB-027 | Deterministische offline replay | Implementatie aanwezig | [#33](https://github.com/makafeli/arbitrage-research/issues/33), [#34](https://github.com/makafeli/arbitrage-research/issues/34), [#37](https://github.com/makafeli/arbitrage-research/issues/37) |
| [#42](https://github.com/makafeli/arbitrage-research/issues/42) | ARB-028 | Compleet Base-transactieplan en guards | Gepland | [#33](https://github.com/makafeli/arbitrage-research/issues/33), [#35](https://github.com/makafeli/arbitrage-research/issues/35), [#39](https://github.com/makafeli/arbitrage-research/issues/39) |
| [#43](https://github.com/makafeli/arbitrage-research/issues/43) | ARB-029 | Compleet Solana-transactieplan en guards | Gepland | [#34](https://github.com/makafeli/arbitrage-research/issues/34), [#35](https://github.com/makafeli/arbitrage-research/issues/35), [#39](https://github.com/makafeli/arbitrage-research/issues/39) |
| [#44](https://github.com/makafeli/arbitrage-research/issues/44) | ARB-030 | Complete Base-simulatie en exact-planbewijs | Gepland | [#32](https://github.com/makafeli/arbitrage-research/issues/32), [#41](https://github.com/makafeli/arbitrage-research/issues/41), [#42](https://github.com/makafeli/arbitrage-research/issues/42) |
| [#45](https://github.com/makafeli/arbitrage-research/issues/45) | ARB-031 | Complete Solana-simulatie en exact-planbewijs | Gepland | [#32](https://github.com/makafeli/arbitrage-research/issues/32), [#41](https://github.com/makafeli/arbitrage-research/issues/41), [#43](https://github.com/makafeli/arbitrage-research/issues/43) |
| [#46](https://github.com/makafeli/arbitrage-research/issues/46) | ARB-032 | Vertraging, inclusion en failure-scenarios | Gepland | [#39](https://github.com/makafeli/arbitrage-research/issues/39), [#40](https://github.com/makafeli/arbitrage-research/issues/40), [#41](https://github.com/makafeli/arbitrage-research/issues/41), [#44](https://github.com/makafeli/arbitrage-research/issues/44), [#45](https://github.com/makafeli/arbitrage-research/issues/45) |
| [#47](https://github.com/makafeli/arbitrage-research/issues/47) | ARB-033 | Voorwaarden voor geschatte uitvoerbaarheid | Gepland | [#38](https://github.com/makafeli/arbitrage-research/issues/38), [#39](https://github.com/makafeli/arbitrage-research/issues/39), [#40](https://github.com/makafeli/arbitrage-research/issues/40), [#44](https://github.com/makafeli/arbitrage-research/issues/44), [#45](https://github.com/makafeli/arbitrage-research/issues/45), [#46](https://github.com/makafeli/arbitrage-research/issues/46) |
| [#48](https://github.com/makafeli/arbitrage-research/issues/48) | ARB-034 | Eerlijke vergelijkingen en holdoutanalyse | In uitvoering | [#37](https://github.com/makafeli/arbitrage-research/issues/37), [#39](https://github.com/makafeli/arbitrage-research/issues/39), [#40](https://github.com/makafeli/arbitrage-research/issues/40), [#46](https://github.com/makafeli/arbitrage-research/issues/46), [#47](https://github.com/makafeli/arbitrage-research/issues/47) |
| [#49](https://github.com/makafeli/arbitrage-research/issues/49) | ARB-035 | Paper-releasepoort en financiële juistheid | Gepland | [#38](https://github.com/makafeli/arbitrage-research/issues/38), [#40](https://github.com/makafeli/arbitrage-research/issues/40), [#41](https://github.com/makafeli/arbitrage-research/issues/41), [#44](https://github.com/makafeli/arbitrage-research/issues/44), [#45](https://github.com/makafeli/arbitrage-research/issues/45), [#47](https://github.com/makafeli/arbitrage-research/issues/47), [#48](https://github.com/makafeli/arbitrage-research/issues/48) |

### M4: dashboard en operations

| Issue | Taak-ID | Onderwerp | Registerstatus | Open voorafgaande taken |
|---|---|---|---|---|
| [#51](https://github.com/makafeli/arbitrage-research/issues/51) | ARB-037 | Dashboarddata, herkomst en datakwaliteit | Implementatie aanwezig | [#37](https://github.com/makafeli/arbitrage-research/issues/37) |
| [#52](https://github.com/makafeli/arbitrage-research/issues/52) | ARB-038 | Experimentaanmaak en onveranderlijke instellingen | Implementatie aanwezig | [#35](https://github.com/makafeli/arbitrage-research/issues/35) |
| [#53](https://github.com/makafeli/arbitrage-research/issues/53) | ARB-039 | START/PAUSE/RESUME/STOP en acknowledgements | Implementatie aanwezig | [#51](https://github.com/makafeli/arbitrage-research/issues/51) |
| [#54](https://github.com/makafeli/arbitrage-research/issues/54) | ARB-040 | Runhistorie, ledger en vergelijkingsweergave | In uitvoering | [#40](https://github.com/makafeli/arbitrage-research/issues/40), [#48](https://github.com/makafeli/arbitrage-research/issues/48), [#51](https://github.com/makafeli/arbitrage-research/issues/51), [#52](https://github.com/makafeli/arbitrage-research/issues/52) |
| [#55](https://github.com/makafeli/arbitrage-research/issues/55) | ARB-041 | Reproduceerbare JSON/CSV-export en redactie | In uitvoering | [#37](https://github.com/makafeli/arbitrage-research/issues/37), [#41](https://github.com/makafeli/arbitrage-research/issues/41), [#48](https://github.com/makafeli/arbitrage-research/issues/48), [#54](https://github.com/makafeli/arbitrage-research/issues/54) |
| [#56](https://github.com/makafeli/arbitrage-research/issues/56) | ARB-042 | Monitoring, alarmen en bruikbare diagnostiek | In uitvoering | [#38](https://github.com/makafeli/arbitrage-research/issues/38), [#51](https://github.com/makafeli/arbitrage-research/issues/51) |
| [#57](https://github.com/makafeli/arbitrage-research/issues/57) | ARB-043 | Toegankelijkheid, responsive gedrag en foutweergave | In uitvoering | [#51](https://github.com/makafeli/arbitrage-research/issues/51), [#52](https://github.com/makafeli/arbitrage-research/issues/52), [#53](https://github.com/makafeli/arbitrage-research/issues/53), [#54](https://github.com/makafeli/arbitrage-research/issues/54), [#55](https://github.com/makafeli/arbitrage-research/issues/55), [#56](https://github.com/makafeli/arbitrage-research/issues/56) |
| [#58](https://github.com/makafeli/arbitrage-research/issues/58) | ARB-044 | Deployment, backups en herstelbewijs | In uitvoering | [#49](https://github.com/makafeli/arbitrage-research/issues/49), [#53](https://github.com/makafeli/arbitrage-research/issues/53), [#55](https://github.com/makafeli/arbitrage-research/issues/55), [#56](https://github.com/makafeli/arbitrage-research/issues/56), [#57](https://github.com/makafeli/arbitrage-research/issues/57) |

### M5: meting, beoordeling en besluit

| Issue | Taak-ID | Onderwerp | Registerstatus | Open voorafgaande taken |
|---|---|---|---|---|
| [#59](https://github.com/makafeli/arbitrage-research/issues/59) | ARB-045 | Meetprotocol vooraf vastleggen | Gepland | [#48](https://github.com/makafeli/arbitrage-research/issues/48), [#58](https://github.com/makafeli/arbitrage-research/issues/58) |
| [#60](https://github.com/makafeli/arbitrage-research/issues/60) | ARB-046 | Meetcampagne en datakwaliteitsincidenten | Gepland | [#59](https://github.com/makafeli/arbitrage-research/issues/59) |
| [#61](https://github.com/makafeli/arbitrage-research/issues/61) | ARB-047 | Replaysteekproeven en boekhoudcontrole | Gepland | [#60](https://github.com/makafeli/arbitrage-research/issues/60) |
| [#62](https://github.com/makafeli/arbitrage-research/issues/62) | ARB-048 | Economische evaluatie en haalbaarheidsrapport | Gepland | [#60](https://github.com/makafeli/arbitrage-research/issues/60), [#61](https://github.com/makafeli/arbitrage-research/issues/61) |
| [#63](https://github.com/makafeli/arbitrage-research/issues/63) | ARB-049 | Operationele en onderzoeksbeveiligingsreview | Gepland | [#58](https://github.com/makafeli/arbitrage-research/issues/58), [#61](https://github.com/makafeli/arbitrage-research/issues/61) |
| [#64](https://github.com/makafeli/arbitrage-research/issues/64) | ARB-050 | Expliciete go/revise/stop-beslissing | Gepland | [#62](https://github.com/makafeli/arbitrage-research/issues/62), [#63](https://github.com/makafeli/arbitrage-research/issues/63) |

### M6: livevoorbereiding en review

| Issue | Taak-ID | Onderwerp | Registerstatus | Open voorafgaande taken |
|---|---|---|---|---|
| [#65](https://github.com/makafeli/arbitrage-research/issues/65) | ARB-051 | Concrete live-scope en risicolimieten | Gepland | [#64](https://github.com/makafeli/arbitrage-research/issues/64) |
| [#66](https://github.com/makafeli/arbitrage-research/issues/66) | ARB-052 | Afzonderlijke signer en duurzame intrekking | Gepland | [#65](https://github.com/makafeli/arbitrage-research/issues/65) |
| [#67](https://github.com/makafeli/arbitrage-research/issues/67) | ARB-053 | Geharde live-uitvoeringsartefacten en atomiciteit | Gepland | [#42](https://github.com/makafeli/arbitrage-research/issues/42), [#43](https://github.com/makafeli/arbitrage-research/issues/43), [#65](https://github.com/makafeli/arbitrage-research/issues/65) |
| [#68](https://github.com/makafeli/arbitrage-research/issues/68) | ARB-054 | Werkelijke saldi, reserveringen en risicocontrole | Gepland | [#40](https://github.com/makafeli/arbitrage-research/issues/40), [#65](https://github.com/makafeli/arbitrage-research/issues/65) |
| [#69](https://github.com/makafeli/arbitrage-research/issues/69) | ARB-055 | Duurzame intentie-, payload- en verzendregistratie | Gepland | [#66](https://github.com/makafeli/arbitrage-research/issues/66), [#68](https://github.com/makafeli/arbitrage-research/issues/68) |
| [#70](https://github.com/makafeli/arbitrage-research/issues/70) | ARB-056 | Goedgekeurd chain-specifiek verzendtransport | Gepland | [#67](https://github.com/makafeli/arbitrage-research/issues/67), [#69](https://github.com/makafeli/arbitrage-research/issues/69) |
| [#71](https://github.com/makafeli/arbitrage-research/issues/71) | ARB-057 | Pending/unknown/failure/finality-reconciliatie | Gepland | [#69](https://github.com/makafeli/arbitrage-research/issues/69), [#70](https://github.com/makafeli/arbitrage-research/issues/70) |
| [#72](https://github.com/makafeli/arbitrage-research/issues/72) | ARB-058 | Fencing, crashherstel en gecontroleerde overname | Gepland | [#66](https://github.com/makafeli/arbitrage-research/issues/66), [#69](https://github.com/makafeli/arbitrage-research/issues/69), [#71](https://github.com/makafeli/arbitrage-research/issues/71) |
| [#73](https://github.com/makafeli/arbitrage-research/issues/73) | ARB-059 | Livegereedheid, activering en incidentbediening | Gepland | [#53](https://github.com/makafeli/arbitrage-research/issues/53), [#65](https://github.com/makafeli/arbitrage-research/issues/65), [#66](https://github.com/makafeli/arbitrage-research/issues/66), [#68](https://github.com/makafeli/arbitrage-research/issues/68), [#71](https://github.com/makafeli/arbitrage-research/issues/71), [#72](https://github.com/makafeli/arbitrage-research/issues/72) |
| [#74](https://github.com/makafeli/arbitrage-research/issues/74) | ARB-060 | Onafhankelijke uitvoeringsreview en remediatie | Gepland | [#66](https://github.com/makafeli/arbitrage-research/issues/66), [#67](https://github.com/makafeli/arbitrage-research/issues/67), [#68](https://github.com/makafeli/arbitrage-research/issues/68), [#69](https://github.com/makafeli/arbitrage-research/issues/69), [#70](https://github.com/makafeli/arbitrage-research/issues/70), [#71](https://github.com/makafeli/arbitrage-research/issues/71), [#72](https://github.com/makafeli/arbitrage-research/issues/72), [#73](https://github.com/makafeli/arbitrage-research/issues/73) |
| [#75](https://github.com/makafeli/arbitrage-research/issues/75) | ARB-061 | Definitief pilotpakket en goedkeuring | Gepland | [#64](https://github.com/makafeli/arbitrage-research/issues/64), [#74](https://github.com/makafeli/arbitrage-research/issues/74) |

### M7: pilot, uitbreiding en onderhoud

| Issue | Taak-ID | Onderwerp | Registerstatus | Open voorafgaande taken |
|---|---|---|---|---|
| [#76](https://github.com/makafeli/arbitrage-research/issues/76) | ARB-062 | Goedgekeurde begrensde livepilot uitvoeren | Gepland | [#75](https://github.com/makafeli/arbitrage-research/issues/75) |
| [#77](https://github.com/makafeli/arbitrage-research/issues/77) | ARB-063 | Pilot evalueren en vervolg besluiten | Gepland | [#76](https://github.com/makafeli/arbitrage-research/issues/76) |
| [#78](https://github.com/makafeli/arbitrage-research/issues/78) | ARB-064 | Tweede venue per chain kwalificeren | Gepland | [#49](https://github.com/makafeli/arbitrage-research/issues/49), [#62](https://github.com/makafeli/arbitrage-research/issues/62), [#64](https://github.com/makafeli/arbitrage-research/issues/64) |
| [#79](https://github.com/makafeli/arbitrage-research/issues/79) | ARB-065 | Meer tokens en eventuele drieledige routes | Gepland | [#78](https://github.com/makafeli/arbitrage-research/issues/78) |
| [#80](https://github.com/makafeli/arbitrage-research/issues/80) | ARB-066 | Aanvullende chain kwalificeren | Gepland | [#62](https://github.com/makafeli/arbitrage-research/issues/62), [#64](https://github.com/makafeli/arbitrage-research/issues/64), [#78](https://github.com/makafeli/arbitrage-research/issues/78) |
| [#81](https://github.com/makafeli/arbitrage-research/issues/81) | ARB-067 | Optionele atomaire fundingadapter | Gepland | [#65](https://github.com/makafeli/arbitrage-research/issues/65), [#67](https://github.com/makafeli/arbitrage-research/issues/67), [#74](https://github.com/makafeli/arbitrage-research/issues/74), [#77](https://github.com/makafeli/arbitrage-research/issues/77) |
| [#82](https://github.com/makafeli/arbitrage-research/issues/82) | ARB-068 | Onderhoud, change control en veilige afsluiting | Gepland | [#58](https://github.com/makafeli/arbitrage-research/issues/58), [#64](https://github.com/makafeli/arbitrage-research/issues/64) |


### Buiten deze oorspronkelijke takenlijst

De zeven open parent epics zijn #5, #6, #7, #8, #10, #12 en #13. #107 is een afzonderlijke toolingtaak voor de native ontwikkelagent-start. Deze items zijn geen extra handelsfeatures boven op de 53 taken. De historische vijf-agentwerkronde is afgerond; er is geen door deze handoff gestarte autonome runner. [S09] [S23]

## 9. Testen, review, sluiten en overdracht bij onderbreking

### 9.1 Verplichte werkwijze

Gebruik het bestaande proces uit `AGENTS.md` en `CONTRIBUTING.md`: actuele toestand lezen, exclusieve afgebakende claim, klein samenhangend resultaat, passende tests, bronreview, exact-head/base-integratie en pas daarna criteriumgewijze acceptatie. Een gewijzigde basis vereist gecombineerde validatie. [S12] [S13]

Sluit een issue uitsluitend wanneer alle oorspronkelijke criteria specifiek bewijs hebben, relevante afhankelijkheden zijn geaccepteerd, blokkerende bevindingen zijn opgelost en de native checklist klopt. Gebruik de bestaande closeout-preflight uit `scripts/delivery.py` volgens `docs/DELIVERY-COORDINATION.md`. Zo'n offline controle voert zelf geen sluiting uit en bewijst geen actuele remote toestand. Synchroniseer pas daarna native issue, register en parent epic.

De gebruiker wil geen nieuwe duplicerende taken voor werk dat al onder #58 of een ander bestaand issue hoort. Voeg concrete resterende criteria en bewijs toe aan bestaande issues. Verander scope alleen via een expliciete beslissing, niet door criteria te verwijderen.

### 9.2 Gericht testen zonder productie te muteren

De volgende commando's zijn bestaande lokale controlevoorbeelden voor een uitgecheckte bronversie met de vereiste testafhankelijkheden. Ze zijn geen claim dat ze in deze handoff-ronde opnieuw zijn uitgevoerd:

```bash
python scripts/validate_specs.py
python scripts/validate_project.py
python scripts/test_worker_launch_base.py -v
python scripts/test_postgres_leaf_repair.py -v
python scripts/test_backlog_summary.py -v
python scripts/test_pipeline_workflow.py
```

Voor Rust/HTTP/PostgreSQL- en containerwerk is de volledige omgeving uit `.github/workflows/ci.yml` leidend. Gebruik een **geïsoleerde testdatabase**, de vastgelegde toolchain en de echte vereiste API-binary. Laat `TEST_DATABASE_URL` nooit naar productie wijzen. Tests overslaan wegens ontbrekende PostgreSQL/Docker is geen geslaagde test. De feitelijke shipped-containerproeven blijven nodig voor wijzigingen aan launcher of certificaatonderhoud. [S18]

Voor een reeds opgehaalde volledige native issue-snapshot:

```bash
python scripts/backlog_summary.py --snapshot github-issues.json --format markdown
```

De rapporteur leest en telt; hij voert geen GitHub-sluitingen, deployments of agentstarts uit. [S09]

### 9.3 Bewijspakket per afgerond werkpakket

Bewaar bij het bestaande ticket: exacte head/base/merge-SHA, gewijzigde paden, uitgevoerde tests met job-URL's, reviewer en beperkingen, deployment-ID/revisie, niet-geheime runtimebewijzen, onvervulde criteria en concrete volgende actie. Maak onderscheid tussen synthetische tests, historische echte captures en actuele gehoste waarnemingen.

Voor de eerstvolgende operationele ronde hoort het bewijs ten minste de backupidentiteit, certificaatcontrole, echte Rust-handshake, oorspronkelijke sessie, broncheckpoint, heartbeat, commandorevisies en start/stop/herstartuitkomsten te bevatten. Een ontbrekend deel blijft expliciet open.

### 9.4 Niet opnieuw doen

Herhaal geen accountactivatie, providerregistratie, Base-secretsetup, profielvoorbereiding of oorspronkelijke sessieregistratie zolang de bestaande gegevens kloppen. Herbouw PR148/149 niet als ongepubliceerde patches. Presenteer de historische recorded-slice niet als verloren of nog nooit uitgevoerd. Herbouw de frontend niet en voeg geen nieuwe Demo-keuze toe.

Schakel TLS-/CI-/broncontroles niet uit. Reset geen HALTED bron, migreer geen OBSERVE-sessie in-place naar PAPER/LIVE en verhoog geen providerbudgetten om een test te laten slagen. Een nieuwe PAPER-run hoort bij de daarvoor bedoelde onveranderlijke experimentaanmaak; dat staat los van het behoud van de bestaande OBSERVE-sessie. [S03] [S04] [S07]

### 9.5 Sjabloon bij onderbreking

```text
Datum/tijd en uitvoerder:
Bestaand issue / afgebakend doel:
Basis-SHA / branch / actuele head:
Gewijzigde bestanden:
Werkelijk uitgevoerd en geslaagd:
Werkelijk uitgevoerd en mislukt:
Niet uitgevoerd:
Reviewbevindingen en beperkingen:
Productietoestand vóór/na, inclusief deployment-ID:
Bron-, sessie- en commandostaat:
Open criteria en exacte blokkade:
Eerstvolgende concrete actie en bevoegde uitvoerder:
Bewijslinks:
Werkclaim vrijgegeven of expliciet nog actief:
```

Noem geen onbewaakte achtergrondontwikkeling of actieve subagents zonder daadwerkelijk teruggeleverde runtime-identiteiten en actuele status.

## 10. Besluiten en bevoegdheden die nog nodig zijn

| Onderwerp | Wat vastligt / wat nog nodig is |
|---|---|
| Account, Base-RPC en bestaande sessie | Ingericht. Niet opnieuw bij de eigenaar opvragen. |
| Databaseonderhoud | Een bevoegde uitvoering en aantoonbare terugvalroute zijn nodig; de eerdere ChatGPT-koppeling leverde die niet. |
| Base-only versus oorspronkelijke release | Base eerst is voorgestelde prioriteit. Een kleinere formele release is nog niet als goedgekeurde wijziging vastgelegd. |
| Meting en beoordeling | Protocol, dekking, kosten en uitsluitingen onder #59-#64 vastleggen; geen fictieve resultaten of impliciet gekozen meetvenster. |
| Extra provideruitgaven / infrastructuur | Niet stilzwijgend aanschaffen of verhogen. Behoud bestaande scope en budgetten. |
| Live custody, kapitaal en risico | Exacte keuzes en limieten onder #64/#65; private sleutelmaterialen niet in deze overdracht. |
| Onafhankelijke uitvoeringstechnische review | Onder #74 daadwerkelijk organiseren en aantonen; niet door agentzelfreview vervangen. |
| Financiering en liveactivering | Alleen via de concrete versie-/limietgoedkeuring en de gecontroleerde pilotprocedure. |

Deze besluiten hoeven niet allemaal te wachten voordat afgebakende onderzoeksontwikkeling verdergaat. Scheid ontwikkelwerk, productieonderhoud, kwalificatie en geldbewegingen expliciet. [S03] [S11]

## 11. Starttekst voor de volgende ontwikkelaar of agent

Onderstaande tekst kan samen met dit bestand aan een opvolger worden gegeven:

```text
Neem makafeli/arbitrage-research over op basis van dit handoff-bestand.
Controleer eerst actuele main, issues, PR's, reviews, CI en Railway-configuratie.
De laatst gecontroleerde main is 52b3cb93441d02a73bd3d0c1f5105d9266c89f11.
PR148 en PR149 zijn al gemerged. Geen oude patch opnieuw publiceren.

Lees AGENTS.md, GOAL.md, CONTRIBUTING.md, planning/backlog.json,
planning/implementation-progress.json, issue #58, docs/BASE-WORKER-LAUNCH.md
en docs/POSTGRES-LEAF-REPAIR.md. De bestaande login, Base-RPC, twee-poolprofiel,
publieke CA-configuratie en oorspronkelijke OBSERVE-sessie blijven behouden.

De eerste levering is een aantoonbaar werkende Base-onderzoeksflow.
Begin met de operationele voortzetting van #58: CI-gate/logs reconcilieren,
bevoegde onderhoudsverbinding en gecontroleerd herstelpunt vaststellen,
bestaande certificaatreparatie toepassen/herladen en de echte Rust-
databaseverbinding met de oorspronkelijke sessie bewijzen.
Daarna #30/#32 integreren en #51/#53 voor echte dashboardbediening toetsen.

De laatste opgehaalde workerinspectie van 18 september 2026 08:03:26 UTC
had één RECOVERING-sessie, nul streams en geen actieve lease. Railway staat
nog op worker-entrypoint worker-readiness-check. SUCCESS is geen marktfeed.
De laatste logselectie had ook CI_WAIT_EXHAUSTED; reconcilieer die poging
vóór activatie en schakel geen gate uit.

Initialiseer alleen een werkelijk ontbrekende passende bron. Normale starts
hergebruiken de bestaande bron. Een launcherstart is geen onderzoeks-START.
Geen automatische source reset/rearm, geen nieuwe OBSERVE-sessie en geen
omzeiling van de 16-blockgrens. (sinds #187: 32 blokken, zie docs/BASE-LOG-RECOVERY.md)
Gebruik de bestaande geauthenticeerde bediening.

Als beheeruitvoering blokkeert, registreer de exacte ontbrekende mogelijkheid
en vervolg inhoudelijke ontwikkeling/testen tegen bestaande contracten.
Herhaal niet alleen voorbereidende scripts of voortgangsdocumentatie.
Werk vervolgens aan quote-/routekwalificatie, volledige transactiesimulatie,
kosten en automatische virtuele afwikkeling volgens de oorspronkelijke taken.

Base-first is een prioriteitsvoorstel; Base-only is nog geen goedgekeurde
vervanging van het oorspronkelijke Base+Solana-releasecontract. Behoud de
huidige acceptatiecriteria tenzij de eigenaar een scopewijziging vastlegt.

Gebruik bestaande tickets, afgebakende branches en exacte bron-/testbewijzen.
Rapporteer afzonderlijk wat gebouwd, getest, gemerged, uitgerold en werkelijk
bruikbaar is. Sluit alleen volledig bewezen oorspronkelijke criteria en
geaccepteerde afhankelijkheden. Geen fictieve percentages of actieve agents.

Echte handel is een aparte fase: #64/#65, de uitvoeringslaag #66-#73,
onafhankelijke review #74, goedkeuring #75 en pilot #76. Geen funding,
productiesigning, broadcast of automatische liveactivering vanuit onderzoek.
```

## 12. Bronnen en herleidbaarheid

### 12.1 Bewijssoorten en actualiteit

De GitHub-branchlijst, open PR-lijst, totale open-issuetelling, #58 en de hoofd-CI-jobs zijn tijdens het maken van deze overdracht rechtstreeks gelezen. De Railway-projectstatus, workerconfiguratie en nieuwste workerlogs zijn eveneens rechtstreeks gelezen. Deze reads vormen geen gezamenlijke atomaire snapshot.

De detailtelling gebruikt daarnaast het volledige Delivery review-artefact van run **35319756677**, artefact **10536248319**, met SHA256:

```text
f2374a3cda0a8f1e68ab47d3230cf9b29b4be01a64b3c31b71fca84a45ea6484
```

De bronboom van de daarin aanwezige 474 bronbestanden is bij deze handoff opnieuw berekend en gelijk aan `550805a7a0309bd257aa4cb5fee0fb3235b43971`, de gecontroleerde `main`-boom. Het artefact vermeldt de PR-testmerge `81d7799a7e6e25d34a4bbf6df0fe701b65810c93`; die commit wordt niet verward met de uiteindelijke main-commit. Dezelfde bronboom maakt het bruikbaar voor bestandsinventarisatie en dependencyberekening.

Voor Railway-readback zijn de volgende exacte verzoeken gebruikt:

```text
get-status(projectId=c7cea88d-24b8-45dc-9602-7c0c272f1da7,
           environmentId=5ff4044a-b0ac-4b02-b9cd-888cd91a3fee)
get-service-config(projectId=c7cea88d-24b8-45dc-9602-7c0c272f1da7,
                   serviceId=6b546a9d-ebd1-4443-9526-fbd61acf96e6,
                   environmentId=5ff4044a-b0ac-4b02-b9cd-888cd91a3fee)
get-logs(projectId=c7cea88d-24b8-45dc-9602-7c0c272f1da7,
         deploymentId=ca4da5bd-3460-4878-a314-3cf1d0050bb8,
         types=[deploy], limit=60)
```

Dat zijn herhaalbare bronverwijzingen voor de gekoppelde omgeving, geen shellcommando's. Deploymentstatus is niet vertaald naar onbewezen gebruikers-, provider- of handelskwalificatie. Openbare platformdocumentatie is alleen gebruikt voor algemene SSH-, backup-, pre-deploy- en certificaatherlaadmechanismen, niet als bewijs van de toestand van deze specifieke database.

### 12.2 Bronnenlijst

| Bron | Inhoud |
|---|---|
| [S01] | Actuele branchlijst |
| [S02] | Voortgangsregister op gecontroleerde bron |
| [S03] | Oorspronkelijke backlog, criteria en dependencygraaf |
| [S04] | Bestaand operationeel issue #58; actuele updates en historische bewijsindex |
| [S05] | PR148: bestaande Base-sessie starten en database-identiteit controleren |
| [S06] | PR149: gecontroleerd certificaatonderhoud en backlograpportage |
| [S07] | Runbook voor de bestaande Base-launcher |
| [S08] | Runbook voor certificaatreparatie, productiegrenzen en exacte pins |
| [S09] | Uitsplitsing van voortgang; historische runtimeparagraaf niet verwarren met huidige logs |
| [S10] | Hoofdteststraat op gecontroleerde main, run35320749453 |
| [S11] | Oorspronkelijke delivery- en releaseafspraken |
| [S12] | Werkclaims, review en oorspronkelijke acceptatieprocedure |
| [S13] | Bijdrage- en reviewafspraken |
| [S14] | Bestaande accounttoegang en behoud van login |
| [S15] | Historische echte Base-observatie en beperkingen |
| [S16] | Beheerde Base-inhaalstappen en behoud van bron-/commandogrenzen |
| [S17] | Broncontinuïteit in de bestaande decision inspector |
| [S18] | Werkelijke CI-omgeving en tests |
| [S19] | PR149 detailartefact met native snapshot en bronarchief |
| [S20] | Dependency-aware ontwikkelcoördinatie en closeout |
| [S21] | Opnieuw gelezen telling van open issues zonder PRs |
| [S22] | Opnieuw gelezen open PR-lijst |
| [S23] | Losse native agent-starttaak en historische uitvoering |
| [S24] | Vastgelegde Rust-toolchain |
| [S25] | Officiële Railway-documentatie: pre-deploy heeft geen volumes |
| [S26] | Officiële Railway-documentatie: SSH |
| [S27] | Officiële PostgreSQL18-documentatie: TLS en herladen |
| [S28] | Officiële Railway-documentatie: volumeback-ups |

[S01]: https://api.github.com/repos/makafeli/arbitrage-research/branches
[S02]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/planning/implementation-progress.json
[S03]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/planning/backlog.json
[S04]: https://github.com/makafeli/arbitrage-research/issues/58
[S05]: https://github.com/makafeli/arbitrage-research/pull/148
[S06]: https://github.com/makafeli/arbitrage-research/pull/149
[S07]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/docs/BASE-WORKER-LAUNCH.md
[S08]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/docs/POSTGRES-LEAF-REPAIR.md
[S09]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/docs/DELIVERY-STATUS.md
[S10]: https://github.com/makafeli/arbitrage-research/actions/runs/35320749453
[S11]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/docs/07-DELIVERY-PLAN.md
[S12]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/AGENTS.md
[S13]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/CONTRIBUTING.md
[S14]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/docs/ACCOUNT-ACCESS.md
[S15]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/docs/RECORDED-BASE-SLICE-RESULT.md
[S16]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/docs/MANAGED-BASE-WORKER.md
[S17]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/docs/DECISION-CONTINUITY-UI.md
[S18]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/.github/workflows/ci.yml
[S19]: https://github.com/makafeli/arbitrage-research/actions/runs/35319756677
[S20]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/docs/DELIVERY-COORDINATION.md
[S21]: https://api.github.com/search/issues?q=repo%3Amakafeli%2Farbitrage-research%20is%3Aissue%20is%3Aopen&per_page=1
[S22]: https://api.github.com/repos/makafeli/arbitrage-research/pulls?state=open&per_page=100
[S23]: https://github.com/makafeli/arbitrage-research/issues/107
[S24]: https://github.com/makafeli/arbitrage-research/blob/52b3cb93441d02a73bd3d0c1f5105d9266c89f11/rust-toolchain.toml
[S25]: https://docs.railway.com/deployments/pre-deploy-command
[S26]: https://docs.railway.com/cli/ssh
[S27]: https://www.postgresql.org/docs/18/ssl-tcp.html
[S28]: https://docs.railway.com/volumes/backups

---

**Overdrachtsconclusie:** hervat vanaf de geïntegreerde code op `52b3cb93`. Maak eerst de bestaande Base-flow aantoonbaar bruikbaar. Rond daarna de oorspronkelijke kwalificatie-, simulatie-, boekhoud- en operationele criteria af. Behandel scopeversmalling en echte handel als afzonderlijke expliciete besluiten. De bestaande inrichting is het vertrekpunt, niet iets dat opnieuw moet worden opgezet.
