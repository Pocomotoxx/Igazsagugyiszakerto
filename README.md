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

## Előzmények, export és forrás-állapot (Haladó mód)

- **Szolgáltatói idő:** a találatok és az archivált források külön tárolják a közzététel, a módosítás, a megfigyelés (Wayback snapshot) és a begyűjtés idejét. A közzétételi időt a keresőszolgáltató, az oldal `<meta>`/`<time>` jelölése vagy a `Last-Modified` fejléc adja; ha egyik sincs, üres marad, a begyűjtés ideje nem helyettesíti. A frissesség (friss / elavult / ismeretlen) tier-függő küszöb szerint számolódik.
- **Rekordvizsgáló:** az Előzmények fülön egy forrásra kattintva látszik az eredet, az időpontok, az integritás (az archivált fájl SHA-256 értéke a rögzítettel egyezik-e), a kapcsolódó rekordok (azonos tartalom, történet, domain, futam) és az auditesemények.
- **Előzmények:** szűrés szöveg (ékezet- és kisbetű-független), domain, tier, státusz, döntés, típus és dátum szerint, idővonallal és lapozással.
- **Export:** CSV (képletvédelemmel, Excel-barát), JSON, nyomtatható HTML és STIX 2.1; a szűrők az exportba is bekerülnek. A fájlok az adatmappa `exports/` könyvtárába kerülnek.
- **Azonos történet:** a valódi webes találatok közül az egy témáról szólók egy szálba kerülnek (IDF-súlyozott, ékezet- és tőcsonkoló hasonlóság).
- **Futamok összehasonlítása:** minden futam elmenti a találatlistáját; az új, a kiesett és a tier-t váltó találatok megjelennek az ismételt keresés után és az Előzmények fülön.
- **Forrás-állapot mátrix:** futamonként látszik, hogy a natív terv, a Tavily, a Brave, a Google CSE és az AI lefutott-e, hány találatot adott és mennyi ideig tartott.
- **Korlátos lekérés:** átmeneti hibánál (időtúllépés, 408/429/5xx) legfeljebb két újrapróbálás, a `Retry-After` tiszteletben tartásával; a méretkorlát miatt csonka letöltés jelölve van. Az AI-válasz sémaellenőrzött, hibás válasznál szabályalapú tartalék készül.

Az ötletek a Crucix (AGPL-3.0) nyílt forráskódú projektből származnak; kód nem került átvételre, a megoldások önállóan, Rustban készültek.

## Platformok: egy kérés, minden platformon

A keresés nem keresőmotor-választással működik. Minden kérés minden bekapcsolt platformon lefut, és mindenhol a platform saját szintaxisára igazodik:

| Platform | Mit csinál |
|---|---|
| Google, Bing, DuckDuckGo | keresőkártya a platform operátoraival (Bingnél és DuckDuckGónál évhatár nélkül) |
| Google Scholar | évszűrő a `as_ylo`/`as_yhi` paraméterekkel |
| X (Twitter) | az évhatár `since:`/`until:` dátumhatárrá alakul |
| YouTube | operátormentes keresőszöveg |
| Facebook, Reddit | valódi webes keresés a domainre szűkítve + a platform saját keresője (operátormentesen) |
| Magyar Közlöny, Nemzeti Jogszabálytár, KSH, Bíróságok (birosag.hu), Közbeszerzési Értesítő | valódi webes keresés a hivatalos domain(ek)re szűkítve + domainre szűkített Google-kártya |
| Anonimizált bírósági határozatok (eakta.birosag.hu), Kúria jogegységi határozatok | valódi webes keresés a platform saját útvonalára szűkítve (`site:domain/útvonal`) + domainre szűkített Google-kártya + a platform saját keresőoldala kártyaként (a kifejezést ott kézzel kell megadni) |
| Magyar hírcsatornák (RSS) | 10 hírcsatorna közvetlen lekérése és szűrése a kifejezésre (ékezetfüggetlen, évtartománnyal), a csatorna saját közzétételi idejével |

A domainre szűkített valódi keresés platformonként egy hívás az első beállított szolgáltatóval (Tavily, ennek hiányában Brave, majd Google CSE). Ha egyik sincs beállítva, a platformokhoz keresőkártya készül, a forrás-állapot mátrixban pedig „kikapcsolva” szerepel. A platformok a Haladó módban, a keresési panel „Platformok” kártyáján kapcsolhatók ki; a választás megmarad.

### Kulcs nélküli működés és a találatok pontossága

- Ha egyik webes keresőszolgáltatás (Tavily, Brave, Google CSE) sincs beállítva, a valódi találatokat a DuckDuckGo HTML-oldala adja kulcs nélkül (ha az hibázik vagy üres, a Bing HTML-oldala), szándékosan lassan (időközzel, a korlátozás jelzésével). Ez tartalék: ingadozhat, ezért a felület jelzi, honnan jönnek a találatok. Megbízható működéshez érdemes egy ingyenes Tavily-kulcs.
- A keresési eredmények tetején a valódi találatok állnak, utánuk a keresőkártyák; a lista fölött látszik a kettő darabszáma.
- Az RSS-szűrés szóhatáron kezdődő egyezést keres (ragozott alak igen, a szó közepe nem): a „takarítás” nem találja meg az „időmegtakarítás” szót.
