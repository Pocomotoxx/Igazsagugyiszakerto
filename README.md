# IRBR

Tauri + Vue alapú igazságügyi szakértői kereséstámogató alkalmazás.

## Termékcél

Az IRBR egy ügyalapú, igazságügyi szakértőt támogató asztali alkalmazás. A célja a kutatási és bizonyítási workflow rendezése, a források értékelhetőségének javítása és az auditálható következtetésképzés támogatása.

## Fókusz

- ügyalapú kutatási workflow
- keresési futamok összeállítása
- hivatalos, tudományos és piaci források priorizálása
- archivált forráskivonatok hash-elt mentése
- auditnyom és Markdown riport export

## Fejlesztés

```bash
npm install
npm run build
```

Tauri futtatáshoz Windows alatt szükséges a Rust toolchain és az MSVC build tools (`link.exe`).

## Portable build

Elokeszitesi ellenorzes:

```bash
npm run portable:check
```

Portable csomag build:

```bash
npm run portable:build
```

Sikeres build utan a csomag ide kerul:

- `release/portable/IRBR-portable`
- `release/portable/IRBR-portable.zip`

A portable csomag automatikusan tartalmazza:

- `IRBR.exe`
- `ELSO-INDITAS.txt`
- `RELEASE-MANIFEST.json`
- `README.md`
- `WORKLOG.md`

## Megjegyzés

A korábbi Python/pywebview előtanulmány kivezetésre került. A támogatott runtime most a `src-tauri` + `src` páros.

## Keresés a böngészőben

A keresés a **Böngésző** fülön történik, úgy, mint egy böngészőben (a Windows alatt beépített WebView2 / Chromium motorral):

- A fül tetején a **Keresés minden platformon** mezőbe írt kifejezés minden bekapcsolt platformhoz külön **böngészőfület** nyit, a platform szintaxisára igazított lekérdezéssel:

| Platform | Lekérdezés |
|---|---|
| Google, Google Hírek, Bing, DuckDuckGo | a saját keresőoldal, a Google az operátorokkal és az évhatárral |
| Google Scholar | évszűrő a `as_ylo`/`as_yhi` paraméterekkel |
| YouTube, Reddit | operátormentes keresőszöveg a platform saját keresőjében |
| Facebook, X | domainre szűkített Google-keresés (így bejelentkezés nélkül is látszik) |
| Magyar Közlöny, Nemzeti Jogszabálytár, KSH, Bíróságok, Közbeszerzési Értesítő | domainre szűkített Google-keresés |
| Anonimizált bírósági határozatok (e-akta), Kúria jogegységi határozatok | útvonalra szűkített Google-keresés (`site:domain/útvonal`) + a platform saját keresőoldala |

- A fülek **lustán töltődnek**: az első azonnal betölt, a többi akkor, amikor rákattintasz. A fül a platform nevét viseli.
- A **Platformok** menüben kapcsolhatók ki a platformok; a választás megmarad.
- Az **Archiválás** gomb az aktív fül *megjelenített* szövegét menti (azt, amit látsz, bejelentkezést kérő oldalon is) a hash-elt archívumba, az oldal által jelzett közzétételi és módosítási idővel. A mentés az aktuális keresési futamhoz kapcsolódik.
- Minden keresés futamként rögzítődik (a megnyitott címekkel), és az audit napló is tartalmazza.
- A Google hozzájárulási oldalát vagy captchát egyszer kell elfogadni/megoldani a böngészőfülön.

A találatok visszajuttatása (archiváláskor) IPC nélkül történik: az oldalba injektált szkript egy megszakított navigációval adja át a szöveget, ezért a betöltött idegen oldal nem fér hozzá az alkalmazás parancsaihoz.

## Egyszerű és Haladó mód

Az **Egyszerű** mód (alapértelmezett) a napi munka fülei: Ügyek, Böngésző, Archivum, Riport. A **Haladó** mód ezen felül a Bemutató, az Előzmények és az Audit fület mutatja.

## Előzmények és export (Haladó mód)

- **Szolgáltatói idő:** az archivált források külön tárolják a közzététel, a módosítás, a megfigyelés (Wayback snapshot) és a begyűjtés idejét. A közzétételi időt az oldal jelzése adja (`<meta>`, `<time>`); ha nincs, üres marad, a begyűjtés ideje nem helyettesíti. A frissesség (friss / elavult / ismeretlen) tier-függő küszöb szerint számolódik.
- **Rekordvizsgáló:** egy forrásra kattintva látszik az eredet, az időpontok, az integritás (az archivált fájl SHA-256 értéke a rögzítettel egyezik-e), a kapcsolódó rekordok (azonos tartalom, történet, domain, futam) és az auditesemények.
- **Előzmények:** szűrés szöveg (ékezet- és kisbetű-független), domain, tier, státusz, döntés, típus és dátum szerint, idővonallal és lapozással.
- **Export:** CSV (képletvédelemmel, Excel-barát), JSON, nyomtatható HTML és STIX 2.1; a szűrők az exportba is bekerülnek. A fájlok az adatmappa `exports/` könyvtárába kerülnek.
- **Futamok összehasonlítása:** minden futam elmenti a megnyitott címeit; az új, a kiesett és a tier-t váltó elemek az Előzmények fülön hasonlíthatók össze.

Az ötletek egy része a Crucix (AGPL-3.0) nyílt forráskódú projektből származik; kód nem került átvételre, a megoldások önállóan, Rustban készültek.

## Gyógyszer-ellenőrzés (gondozóknak és ellátottaknak)

A **Gyógyszer-ellenőrzés** fül ismeretlen gyógyszer gyors, internetes utánanézéséhez ad segítséget. **Nem gyógyszerazonosítás**, és nem helyettesíti a gyógyszerész vagy az orvos szakértelmét.

- **Figyelmeztetés minden indításnál:** a fül megnyitásakor és *minden* művelet előtt (kamera, kép kiválasztása, fotó küldése a Google Lens-nek, keresés névre) egy teljes képernyős figyelmeztetés jelenik meg, amelyet külön meg kell erősíteni. A fül tetején végig látható egy piros figyelmeztető sáv, és a megnyitott Lens-/keresőfülek fölött is ott marad.
- **Fotó:** kamerával vagy képfájlból; a program legfeljebb 1600 képpontosra kicsinyíti, majd egy átmeneti, azonnal törlődő átadással küldi a Google Lens-nek. Ha az automatikus átadás nem működik, a „Google Lens megnyitása (kézi feltöltés)" gomb a lens.google.com oldalt nyitja.
- **Név:** három keresés nyílik: betegtájékoztató, az OGYÉI hivatalos oldalai (`site:ogyei.gov.hu`) és képes találatok a doboz összevetéséhez.
- **Adatvédelem:** a program nem tárolja a fotót és a gyógyszernevet, nem írja az audit naplóba, és a fül oldalai nem archiválhatók. A kép vagy a név a Google-hoz kerül.
- A fül nem kapcsolódik ügyhöz, ezért ügy kiválasztása nélkül is használható.
