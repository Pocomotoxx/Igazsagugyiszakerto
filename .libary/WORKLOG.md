# IRBR Fejlesztési Napló

Ez a napló ritkított archiválással készül.

Szabály:
- csak minden 10. lépés kerül automatikusan archiválásra
- ezen felül minden jelentős architekturális vagy workflow-szintű változás külön bejegyzést kap
- apró UI, szöveg vagy formázási módosítások nem kerülnek külön archiválásra

## Archivált mérföldkövek

### M01 - Tauri-only átállás
- A Python / pywebview ág kivezetve.
- A Vue frontend közvetlen Tauri `invoke` hívásokra átállítva.
- Az alap workflow újraszervezve: ügykezelés, keresési futam, archivum, audit, riport.

### M02 - Keresési futam logika újrakeretezése
- A rendszer nem színlel élő találatlistát.
- Helyette priorizált keresési és megnyitási tervet generál több motorhoz és több query-variánshoz.
- A keresési workflow igazságügyi szakértői kutatástámogatásra lett hangolva.

### M03 - Archiválás és audit nyomvonal stabilizálása
- A forrásarchiválás hash-elt, fájlba mentett kivonattal működik.
- Az audit események külön nézetben és riportban is megjelennek.
- A Tauri konfiguráció kapott tisztább app-azonosítást és CSP-t.

### M04 - Query builder bővítés
- A keresőűrlap földrajzi fókuszt, évszűrést, forráscsomagokat és kutatási lencsét kapott.
- A felület javasolt keresési sorrendet is megjelenít.

### M05 - Keresési futamok perzisztálása
- A `search_runs` tábla bevezetve.
- Ügyenként mentődik a futam ideje, összefoglalója, alap query-je, motorai, tier-szűrője és találatszáma.
- A korábbi futamok visszatölthetők és újrafuttathatók.

### M06 - Futamok összevetése
- Két korábbi keresési futam kijelölhető összehasonlításra.
- A felületen megjelenik a két futam rövid egymás melletti összevetése.

### M07 - Források kapcsolása futamokhoz
- Az archivált források `search_run_id` mezővel kapcsolódnak a keresési futamokhoz.
- A forráslista és az archivum jelzi, melyik kutatási körből származik az adott mentés.

### M08 - Archivum dosszié nézet
- Az archivum futam szerint csoportosítva is megjeleníthető.
- Futam szerinti szűrés és nem kapcsolt mentések külön nézete elkészült.

### M09 - Futam-alapú riportstruktúra
- A riport export futamonként külön forrásblokkokat generál.
- A nem kapcsolt mentések külön szekcióba kerülnek.
- Minden futamblokkban van szakértői megjegyzés hely.

### M10 - Forrás-szintű szakértői értékelés
- Az archivált források külön értékelési mezőket kaptak: relevancia, bizonyító erő, hiányosságok.
- A forrásértékelés a felületen szerkeszthető és menthető.
- Az exportált riport ezeket a szakértői megjegyzéseket is beemeli a forrásblokkokhoz.

### M11 - Forrás triázs és használhatósági jelölések
- Az archivált források állapotmezőt kaptak (`nyers`, `ellenőrzés alatt`, `validált`) és prioritási szintet.
- Az archivum státusz szerint szűrhető, a kártyákon és a riportban is megjelenik a triázs állapot.
- Az ügy törlése már a kapcsolódó keresési futamokat is takarítja, így az adatszerkezet konzisztens marad.

### M12 - Bizonyítási kivonat és felhasználási döntés
- A forrásokhoz külön döntési mező került: `felhasználható`, `háttéranyag`, `kizárandó`.
- Minden archivált tételhez rövid bizonyítási kivonat rögzíthető arról, hogy pontosan mit támaszt alá.
- A riport külön "Kiemelt források" blokkot generál a felhasználhatónak minősített tételekből.

### M13 - Forrás-ellenőrzési workflow
- A forrásokhoz ellenőrző személy és ellenőrzési időpont mező került.
- A validált tételek külön összesítő panelen jelennek meg az archivumban.
- Az audit napló már a forrásdöntés részleteit is rögzíti: státusz, prioritás, használhatóság, ellenőrző és időpont.

### M14 - Ügy-összegző döntéstámogatás
- Az archivum fölé ügy-szintű összegző panel került: fő megállapítások, ellen-megállapítások és hiányzó bizonyítási pontok.
- A riport külön "Ügyszintű döntéstámogatás" blokkot generál a felhasználható, kizárandó és gyenge források alapján.
- A rendszer külön számon tartja a kizárandó és gyenge bizonyító erejű forrásokat az összegzésben.

### M15 - Strukturált szakértői álláspont
- Az ügyek külön mezőt kaptak az előzetes és a végleges szakértői állásponthoz.
- Az archivumban külön szerkesztőpanel jelenik meg az ügy szintű konklúzióhoz.
- A riport export külön "Szakértői konklúzió" fejezetben bontja az előzetes és a végleges álláspontot.

### M16 - Állapotnapló a lezárási workflow előtt
- Az alkalmazás jelen állapotában már kezeli a keresési futamokat, forrás-triázst, használhatósági döntéseket, validálási metaadatokat és ügy-szintű szakértői konklúziót.
- A következő fejlesztési blokk célja a lezárási logika: riportkészültség, nyitott kérdések, lezárt pontok és végleges állásfoglalásra való alkalmasság jelzése.
- Ez a bejegyzés külön checkpointként rögzíti a K07 előtti funkcionalitási állapotot.

### M17 - Riport-minősítés és lezárás
- Az ügyek külön mezőt kaptak a riport státuszához, a végleges állásfoglalásra való alkalmassághoz, valamint a nyitott kérdésekhez és lezárt pontokhoz.
- Az ügylista és az archivum már vizuálisan is jelzi, hogy egy ügy tervezet, ellenőrzés alatt áll vagy véglegesített.
- A riport export most már tartalmazza a lezárási readiness állapotot és a nyitott/lezárt pontok bontását is.

### M18 - Riport-sablon és formai véglegesítés
- Az exportált riport szakértői átadásra alkalmasabb sablont kapott egységesebb fejezetstruktúrával.
- Külön vezetői kivonat került a riport elejére a readiness, forrásszámok és aktuális konklúzió gyors áttekintéséhez.
- A záró fejezet megnevezése és nyelve formálisabb szakértői átadási logikához igazodott.

### M19 - Strukturált export és mellékletlogika
- A riport külön mellékletjegyzéket kapott a futam- és forráscsoportok strukturált felsorolásához.
- A felhasználható forrásokból rövid bizonyítási mátrix készül az alátámasztott állításokkal és bizonyító erővel.
- Az export tagolása így jobban újrahasznosítható átadási és ellenőrzési célokra.

### M20 - Operatív használhatósági finomítás
- Az archivum szűrése kibővült használhatósági döntés és prioritás szerinti szűrőkkel.
- A forráskártyák gyors jelölőgombokat kaptak a tipikus státuszváltásokhoz.
- Rövid beépített súgó került az archivum fölé a napi munkafolyamat gyorsításához.

### M21 - Minőségbiztosítás és adatkonzisztencia
- Mentés előtti minimális ellenőrzések kerültek a forrásértékelésre és az ügyszintű álláspontra.
- A riport export blokkolódik, ha ismert konzisztenciahiba maradt a workflow-ban.
- Az archivum külön minőségellenőrzési panelen jelzi a hiányzó vagy ellentmondásos adatokat.

### M22 - UX tisztítás és teljesítmény
- A duplikált forrásértékelő UI külön komponensbe került, így egyszerűbb karbantartani.
- A minőségellenőrzési hibák egyszer számolódnak ki és újrahasznosulnak a sablonban.
- A refaktor funkcióváltozás nélkül csökkentette az ismétlődő frontend kódot.

### M23 - Portable kiadási előkészítés
- Elkészült a portable build pipeline: előellenőrző script, csomagoló script és release célmappa-logika.
- A projekt most már külön `portable:check` és `portable:build` npm parancsokat kínál.
- A tényleges portable buildet jelenleg a hiányzó `link.exe` blokkolja, ezt az előellenőrzés egyértelműen jelzi.

### M24 - Portable build stabilizálás
- A Rust függőségi ütközés helyi crate patchel oldva lett, így a release exe újra lefordul.
- A portable script már automatikusan a ténylegesen elkészült release exe-t csomagolja, nem fix fájlnevet vár.
- A csomagolás levált a Tauri MSI bundle folyamatról, így WiX letöltés nélkül is elkészül a hordozható kiadás.

### M25 - Portable átadási csomag finomítás
- A portable build most már első indítási útmutatót és géppel olvasható release manifestet is generál.
- A csomag tartalma egyértelműbb lett átadásra és archiválásra is.
- A projektmetaadatok pontosítva lettek, hogy a desktop build jobban tükrözze a valódi termékcélt.

### M26 - Valodi standalone portable exe
- A portable folyamat mar nem nyers `cargo build` kimenetet csomagol, hanem `tauri build --no-bundle` release buildet.
- Ezzel az exe mar beagyazott frontenddel keszul, nem var helyi `localhost` dev szervert futaskor.
- A portable terjesztes igy tovabbra is egyszeru zip marad, de a futtathato allomany valodi desktop build lett.

### M27 - Harommodos keresesi modell visszaepitese
- Visszakerult a valaszthato keresesi mod: natív, hybrid, illetve csak AI API alapú futam.
- Helyi alkalmazasbeallitasok koze bekerult az AI provider, modell, API kulcs es rendszerutasitas tarolasa.
- Az AI modok Tauri oldalon, Windows portable kornyezetben is futtathato PowerShell HTTP hivassal dolgoznak, kulon telepites nelkul.

### M28 - Webarchiv es Wayback metaadatok
- Az archivum kulon kezeli az elo URL-t, az archivalt URL-t, a snapshot azonositojat es a forras archiv tipusát.
- A Wayback keresesi javaslatok mar domain- es pontos URL-szinten is kulon nyithatok.
- A riport es az archivum felulete mar megjeleniti az archivalt/eredeti forras kapcsolatot, ami szakertoi dokumentalasnal fontos.

### M29 - Ketutas archiválás Wayback talalatokhoz
- A talalati listaban a Wayback elemek mar kulon badge-et kapnak.
- Wayback talalatnal kulon archiválhato az aktualis archivalt nezet es az eredeti elo URL is.
- A keresesi talalatmodellek archiv metaadatot is hordoznak, igy az archiválas elott sem vesznek el a forrastipus-informaciok.

### M30 - Reddit kontrollforras reteg
- A keresoben kulon kapcsolokent megjelent a Reddit mint kozossegi kontrollforras.
- A backend kulon Reddit keresesi javaslatokat ad, de alacsonyabb bizonyito ereju kategoriaban.
- Archiválaskor a Reddit forrasok automatikusan ovatosabb, kontrolljellegu alapmetaadatot kapnak.

### M31 - Google lekérdezés javítás és Közbeszerzési Értesítő visszaépítés
- A kereső lekérdezésépítése javítva lett: a forráscsomagok már egy közös OR-blokkban szerepelnek, így nem oltják ki egymást.
- A Közbeszerzési Értesítő újra külön, explicit kurált forrásként megjelent.
- Külön közbeszerzési ár- és szerződéskontroll keresési javaslat is bekerült a backendbe.

### M32 - Szeles es szurt natív keresés szétválasztása
- A natív alaplekérdezés most mar nem kap automatikusan domain- es PDF-korlatozast.
- A szurt szakertoi keresés külön, atlathato tervlepeskent jelenik meg az alap webes futam mellett.
- A felhasznalo a feluleten mar kulon latja a szeles alaplekérdezést és a szurt szakertoi valtozatot.

### M33 - Natív futtatási profilok és build stabilizálás
- A natív modban kulon valaszthato lett a szeles, szurt vagy kombinált futtatási profil.
- A tenyleges keresesi futas mar a valasztott natív profil szerint indit szeles vagy szakitott lekérdezéseket.
- A frontend build explicit Vite root/outDir beallitast kapott a stabilabb csomagolashoz.

### M34 - Facebook csoport kontrollreteg
- Külön kapcsoló került a keresőbe a Google által indexelt Facebook csoportok és oldalak kontrollkereséséhez.
- A backend külön Facebook kontrolltalálatokat is javasol, de továbbra is alacsony bizonyító erejű közösségi forrásként kezeli őket.
- Archiváláskor a Facebook-források ugyanúgy óvatos, kontrolljellegű alapmetaadatot kapnak, mint a Reddit találatok.

### M35 - Kozossegi kontrollreteg szetbontasa
- A keresőben kulon kapcsoló lett a Reddit, a Facebook csoportok és az általános közösségi kontroll számára.
- A keresési tervben ezek mar elkulonult lepesekként jelennek meg, nem egyetlen homalyos kontrollblokkban.
- A felhasznalo igy finomabban tudja szabalyzni, melyik kozossegi csatornat akarja bevonni az ugykutatasba.

## Következő cél

### K03 - Bizonyítási kivonat és felhasználási döntés
- külön mező: felhasználható / háttéranyag / kizárandó
- rövid "mit bizonyít ez a forrás?" összefoglaló mező
- riportban automatikus "kiemelt források" blokk

### K04 - Forrás-ellenőrzési workflow
- ellenőrzési dátum és ellenőrző személy mező
- validált források külön összesítő panelje
- auditban részletesebb forrásdöntési események

### K05 - Ügy-összegző döntéstámogatás
- ügy szintű megállapítások és ellen-megállapítások blokk
- automatikus lista a kizárandó és gyenge bizonyító erejű forrásokról
- gyors "mi hiányzik még az állásfoglaláshoz?" összegzés

### K06 - Strukturált szakértői álláspont
- külön mező az ügy végső álláspontjához
- előzetes és végleges következtetés szétválasztása
- exportban külön "szakértői konklúzió" fejezet

### K07 - Riport-minősítés és lezárás
- ügy lezárási státusz és riportkészültségi jelzés
- exportban "nyitott kérdések" és "lezárt pontok" blokk
- gyors jelzés, hogy az ügy alkalmas-e végleges szakértői állásfoglalásra

### K08 - Riport-sablon és formai véglegesítés
- exportált szöveg sablonosítása bírósági / szakértői átadásra
- egységes megnevezések és fejezetcímek finomítása
- gyors "vezetői kivonat" blokk a riport elejére

### K09 - Strukturált export és mellékletlogika
- külön mellékletjegyzék a futamokhoz és forráscsoportokhoz
- exportált tartalom gépileg jobban újrahasznosítható tagolása
- rövid "bizonyítási mátrix" blokk a kiemelt forrásokhoz

### K10 - Operatív használhatósági finomítás
- gyors jelölőgombok gyakori státuszváltásokhoz
- archivum szűrés bővítése döntés és prioritás szerint
- rövid felhasználói súgó a munkafolyamathoz

### K11 - Minőségbiztosítás és adatkonzisztencia
- alap ellenőrzések hiányzó kulcsmezőkre mentés előtt
- tömör hibaüzenetek és figyelmeztetések a workflow kritikus pontjain
- riport export előtti minimális konzisztenciaellenőrzés

### K12 - UX tisztítás és teljesítmény
- ismétlődő UI blokkok részleges komponensre bontása
- számított ellenőrzések egyszeri kiértékelése a sablonban
- kisebb interakciós és olvashatósági tisztítások

### K13 - Csomagolás és desktop készültség
- asztali build és terjesztési folyamat áttekintése
- release előtti ellenőrzőlista
- alap demo / átadási csomag összeállítása

### K14 - Valódi release build és átadási csomag
- portable build lefuttatása teljes toolchainnel
- kimeneti fájlok ellenőrzése és átadási struktúra validálása
- rövid üzembe helyezési leírás a csomag mellé

### M36 - Archivum eredet szerinti szűrés
- az archivum külön szűrhető hivatalos, webarchív, Reddit, Facebook és egyéb eredetre
- a forráseredet besorolása URL és archiválási típus alapján történik
- az archivált találatok kártyáin már látszik az eredeti forrásközeg is, így gyorsabb az audit

### M37 - Dokumentumtípus-korlát kivezetése
- a keresőűrlapból kikerült a fájltípus mező és a hozzá kötött automatikus filetype-szűkítés
- a natív keresési lekérdezések így szélesebb webes találati halmazt tudnak visszaadni
- a célzott forrásjavaslatoknál a PDF-központú szűkítés enyhítve lett, hogy ne essen ki sok releváns oldal

### M38 - Riport eredet szerinti forrásbontással
- a riport export és előnézet már külön blokkban mutatja a hivatalos, webarchív, Reddit, Facebook és egyéb eredetű forrásokat
- a kiemelt felhasználható források is eredet szerint csoportosítva jelennek meg, nem egyetlen vegyes listában
- az archivum gyorsértékelése már eredet szerinti megoszlást is tartalmaz a szakértői áttekintéshez

### M39 - Riport óvatossági jelzések forráseredet szerint
- a Reddit és Facebook eredetű források a riportban automatikus kontrollforrás-megjegyzést kapnak
- a webarchív eredetű forrásoknál külön jelzés látszik, hogy az időállapot és a közzétételi környezet ellenőrzendő
- az óvatossági logika a kiemelt és az összesített eredet szerinti riportblokkokban is egységes

### M40 - Portable kiadás frissítése az aktuális fejlesztésekkel
- a portable csomag újraépült a legutóbbi keresési, archivum- és riportmódosításokkal
- frissült az `IRBR.exe`, a `RELEASE-MANIFEST.json` és a zip csomag build ideje is
- az átadható hordozható kiadás a `release/portable/IRBR-portable` mappában és zipben elérhető

### M41 - Találati és futamkártyák tömörítése
- a keresési találatoknál a részletes keresési paraméterek külön lenyíló blokkba kerültek
- a korábbi futamok kártyái is rövidebbek lettek, a teljes lekérdezés és a jegyzetmező csak kérésre nyílik le
- a portable kiadás újraépült ezzel a tömörebb, kezelhetőbb felülettel

### M42 - Wayback modul összevonása
- a Wayback ellenőrzés a keresési tervben már egyetlen összefogott modulként jelenik meg
- a backend sem gyárt külön URL-, domain- és kezdőlap-szintű Wayback kártyahalmazt, csak egy tömörített modult
- a portable kiadás újraépült a rövidebb, kevésbé görgetésigényes Wayback megjelenítéssel

### M43 - Közös Webarchív réteg
- az Internet Archive és a Wayback külön blokkjai közös `Webarchív` réteg alá kerültek
- a találati listában már nem jelenik meg külön archivumos és külön waybackes szomszédos kártya
- a portable kiadás újraépült az egységesebb, rövidebb webarchív megjelenítéssel

### M44 - Összecsukható keresési terv
- a `Javasolt keresési sorrend` blokk alapból összecsukott állapotban jelenik meg
- a fejlécben megmarad a lépésszám, de a teljes terv csak külön megnyitásra látszik
- a portable kiadás újraépült a rövidebb, kevésbé görgetésigényes kezdőnézettel

### M45 - Biztonsági és adattárolási javítások
- az AI-hívás már nem temp fájlokból fut, hanem stdin-alapú PowerShell átadással, így a kulcs és a payload nem marad lemezen
- az API-kulcs DPAPI-védett formában kerül mentésre, az adatbázis AppData alá költözött, a CSP szűkebb lett
- egyedibb archivum- és riportfájlnevek, keresési hosszlimit és szigorúbb forrástörlés került be, majd a portable kiadás újraépült

### M46 - Natív Rust AI HTTP kliens
- az AI-hívás teljesen kikerült a PowerShell-ből, a backend már natív `reqwest` HTTP klienssel beszél az OpenAI, OpenRouter és Gemini API-kkal
- ezzel a `service.md` legfontosabb kritikus pontja megszűnt: nincs többé PowerShell-alapú AI hálózati kitérő
- a portable kiadás újraépült a natív Rust HTTP-alapú AI integrációval

### M47 - Audit maradványpontok lezárása
- az alkalmazás- és portable szövegezés igazítva lett az AppData-alapú tényleges adattároláshoz
- a vegyes magyar rendszerüzenetek egy része egységesítve lett, így az audit alacsony súlyosságú inkonzisztenciája csökkent
- a portable kiadás újraépült a pontosított működési leírással és az aktuális biztonsági javításokkal

### M48 - Bemutató menü és hover súgó
- külön `Bemutató` menü készült az alkalmazás fő részeinek rövid ismertetésére
- egységes kérdőjeles hover súgó került a fő blokkok és fontos funkciók mellé
- a portable kiadás újraépült a beépített bemutató- és súgórendszerrel

### M49 - Hibatűrő keresésvezérlés
- a hibrid mód már nem áll le csak azért, mert nincs AI kulcs; ilyenkor a natív és valódi webes találati ág tovább futhat
- a frontend keresésvezérlése részleges futamhibák esetén sem dobja el az összes sikeres találatot, hanem a sikeres részhalmazt megőrzi
- a keresési figyelmeztetések most már külön jelzik a részfutamhibákat, így az operátori diagnózis gyorsabb

### M50 - Valódi forráscsomag-korlátozás
- a forráscsomagok már nem csak javasolt plusz futamokat adnak, hanem ténylegesen korlátozzák a natív és a valódi webes keresést
- ha például csak a Facebook csoportok van bepipálva, akkor a query-builder és a backend megnyitási kártyák is csak ehhez a csomaghoz igazodnak
- a felületen minden forráscsomag mellett látszik, pontosan mely domaineken keres az adott választás

### M-Crucix - Előzmények, export, szolgáltatói idő, állapot-mátrix
- Séma v2: `sources.published_at/modified_at/time_source`, `search_runs.health`, új `run_results` tábla.
- Új modulok: `provenance`, `story`, `export`, `history`, `fetchx` (mind egységtesztelt).
- Új parancsok: `search_history`, `inspect_source`, `export_sources`, `compare_search_runs`, `previous_run_delta`, `get_source_health`, `cluster_stories`.
- Felület: Előzmények fül (Haladó mód), frissességi és történet-jelvények a találatokon, futam-különbség sáv, forrás-állapot chipek.


### M-Platformok - egy kérés minden platformon
- A keresőmotor- és forráscsomag-választás kikerült a keresési űrlapból; a kérés minden bekapcsolt platformon lefut.
- Új modulok: `platforms` (lekérdezés-elemzés és platformonkénti igazítás), `rss` (10 magyar hírcsatorna lekérése és szűrése).
- Új parancsok: `list_platforms`, `platform_links`; az `execute_search` kapott `enabled_platforms` paramétert.
- Platformonkénti forrás-állapot sorok a mátrixban.

