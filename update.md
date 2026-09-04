# IRBR – átvizsgálási jegyzőkönyv és fejlesztési javaslatok

Dátum: 2026-07-12
Vizsgált állomány: `src/App.vue` (2910 sor), `src-tauri/src/lib.rs` (1966 sor), séma és keresési logika.
A kód nem módosult, ez a dokumentum kizárólag javaslatokat tartalmaz.

---

## 1. Összefoglaló

Az alkalmazás felépítése rendben van (Tauri + Vue + SQLite, auditnapló, hash-elt archiválás), ám a keresés körül több olyan hiba és tervezési korlát is akad, amely együttesen okozza a bejelentett tünetet: **az app nem ad találatot, miközben a kézi (natív) keresés talál**. A legfontosabb megértendő tény: az alkalmazás jelenleg nem kérdez le valódi keresőtalálatokat. A „találat" itt egy generált linkkártya (keresésindító URL, kurált forrás, AI-javaslat). Ha a kártyagenerálás bármelyik szűrőn elakad, üres lista jelenik meg, függetlenül attól, hogy a Google-ben lenne-e eredmény.

---

## 2. A „nincs találat" tünet konkrét okai (prioritási sorrendben)

### 2.1 KRITIKUS – Hibrid módban egy AI-hiba az összes natív kártyát is eldobja

`lib.rs:1750` (`execute_search`): a natív kártyák már bekerültek a `results` vektorba, ám az `add_ai_results(...)?` hibája (lejárt API-kulcs, hálózati gond, kvóta, timeout) az egész parancsot `Err`-rel zárja. A frontend ilyenkor csak hibaüzenetet lát, a már legenerált natív találatok elvesznek.

**Javaslat:** az AI-hívás hibáját ne `?`-tel propagáld, hanem kezeld lokálisan: a natív eredmények menjenek vissza, az AI-hiba pedig figyelmeztető mezőként (pl. `ai_error: Option<String>` a válaszban) jelenjen meg a felületen. Az auditnaplóba is kerüljön bejegyzés róla.

### 2.2 KRITIKUS – Az évszűrő rontott lekérdezést állít elő

`App.vue:180` (`buildYearClause`):

- Csak kezdőév esetén a klauzula `"2020" utáni`, csak záróév esetén `"2019" előtti`. Az „utáni" és „előtti" magyar szó literálisan bekerül a Google-lekérdezésbe, a kereső pedig olyan oldalt keres, amelyben ez a szó szerepel. Ez önmagában képes nullára vinni a recall-t, miközben a kézzel beírt, évszám nélküli keresés talál.
- Hat évnél nagyobb tartománynál a `"2020".."2024"` alak keletkezik. A Google számtartomány-operátora idézőjel nélkül működik (`2020..2024`); idézőjelben literál karakterláncként értelmeződik.

**Javaslat:** a klauzula legyen `2020..2024` (idézőjel nélkül), egyetlen év esetén pedig sima `2020` vagy `after:2020` / `before:2020` operátor (a Google ezeket dátumszűrőként ismeri). Az „utáni/előtti" szavas változatot mindenképp töröld.

### 2.3 MAGAS – Túl hosszú, túlszűrt lekérdezések a szűrt futamban

`App.vue:303` (`filteredQuery`): az alaplekérdezéshez hozzáfűződik az összes bekapcsolt forráscsomag `site:` OR-blokkja, plusz a kutatási lencse OR-blokkja. Több csomag esetén a lekérdezés könnyen túllépi a Google 32 szavas limitjét (a többletet a kereső szó nélkül levágja), és a sok kényszerfeltétel együtt gyakran ad nulla találatot, míg ugyanaz a kifejezés simán beírva találatos.

**Javaslat:**

- Egy futamon belül csomagonként külön lekérdezést generálj (pl. `alap + site:njt.hu`, `alap + site:ksh.hu`), ne egyetlen óriás OR-t.
- Mutass szó-/karakterszámlálót a lekérdezés-előnézetnél, és figyelmeztess 32 szó fölött.
- A lencse-OR-blokk és a site-blokk ne kerüljön ugyanabba a lekérdezésbe egyszerre.

### 2.4 MAGAS – A tier-szűrő pontos egyezést kér, ezért üresre szűrhet

`lib.rs:483` (`push_result`): `tier_filter == 0 || tier_filter == tier`. Ha a felhasználó „Tier 1 – Hivatalos"-t választ, a keresőmotor-indítók (tier 3), a lekérdezésvariánsok (tier 3) és a webarchív kártya (tier 2) mind kiesnek; tier 2-nél szinte csak a Scholar és a webarchív marad. Konkrét tier választása és szűk téma együtt könnyen üres listát ad.

**Javaslat:** a szűrő jelentése legyen „ez a tier és jobb" (`tier <= tier_filter`), vagy legalább a keresőmotor-indító kártyák mindig maradjanak benne védőhálóként. A felületen a címke is tükrözze a viselkedést (pl. „Tier 1–2").

### 2.5 KÖZEPES – Ékezetfüggő kulcsszóillesztés a variáns- és domainlogikában

`lib.rs:501` (`build_query_variants`) és `lib.rs:1038` (`add_domain_specific_sources`): a feltételek literál ékezetes alakra illesztenek („óradíj", „mezőgazdas", „ápolás"). Ha a felhasználó ékezet nélkül gépel (oradij, mezogazdasagi), vagy más szóalakot használ (díjazás, gépi bérmunka), a domain-specifikus kártyák és a variánsok el sem készülnek, a lista pedig soványabb vagy üres lesz.

**Javaslat:** normalizáld a lekérdezést illesztés előtt (kisbetűsítés + ékezettelenítés, pl. unidecode jellegű leképezés), és az illesztőmintákat is ékezettelen tőalakban tárold (`oradij`, `mezogazdas`). Hosszabb távon a kulcsszó–csomag hozzárendelést érdemes konfigurálhatóvá tenni (adatbázisban vagy JSON-ban), ne kódba égetve éljen.

### 2.6 KÖZEPES – A pontos kifejezés és a régió idézőjelezése recall-gyilkos

`App.vue:263` (`broadQuery`): a régió mező is idézőjeles pontos frázisként kerül be, a pontos kifejezéssel együtt. Két kötelező frázis + kizárások + site-szűrő együtt gyakran nulla találat.

**Javaslat:** a régiót ne idézőjelezd kötelezően (vagy tedd választhatóvá), és a felületen jelezd, hogy minden idézőjeles blokk kötelező egyezést jelent. Hasznos lenne egy „lazítás" gomb üres eredmény esetén: ugyanaz a futam idézőjelek és site-szűrők nélkül.

### 2.7 ALACSONY – A frontend védőhálója működik, ám némán elfedi az okot

`App.vue:1475`: üres lista esetén megjelennek a fallback keresésindító kártyák, ami jó. Ugyanakkor nem derül ki, melyik szűrő nyelte el az eredményeket.

**Javaslat:** a backend adja vissza diagnosztikaként, hány kártya készült szűrés előtt és hányat dobott el a tier-szűrő (pl. `generated: 14, filtered_out: 12`). A felület írja ki: „12 kártyát a Tier-szűrő rejtett el – szűrő lazítása?". Kerüljön az auditnaplóba is, mert szakértői környezetben a „miért nem volt találat" kérdésre később is válaszolni kell.

---

## 3. Architekturális javaslat: valódi találatlekérés

A mostani modell (linkgyűjtő + kézi megnyitás) auditszempontból védhető, ám a felhasználói elvárás láthatóan az, hogy az app maga hozzon találatokat. Két irány kínálkozik, akár beállításként egymás mellett:

1. **Keresési API integráció:** Brave Search API, SerpAPI vagy Google Programmable Search (CSE). Ezek valódi találatlistát (cím, URL, snippet) adnak, amelyeket a meglévő tier-besorolással és archiválással azonnal össze lehet kötni. A kulcskezelés infrastruktúrája (DPAPI-titkosítás) már megvan.
2. **AI-keresés webes grounding-gal:** a Gemini API `google_search` tool-lal, illetve az OpenAI/Anthropic search-képes modelljei valódi webes találatokra hivatkozó választ adnak; ez a mostani AI-ág természetes továbbfejlesztése lenne a „vak" JSON-javaslatok helyett.

Így a „natív keresés szerint volt találat, az app szerint nem" osztályú panasz megszűnne, mert az app ugyanazt a találathalmazt látná, mint a böngésző.

### 3.1 Appon belüli találatnéző ablak (beépített böngészőnézet)

Az alkalmazásban már most is teljes értékű böngészőmotor fut: a Tauri Windowson a rendszer WebView2-jét használja, ami ugyanaz a Chromium-alapú motor, mint az Edge-é. Külön motort (CEF, Servo, Ultralight) beágyazni ezért nem kell és nem is érdemes; a CEF például 100+ MB-tal hizlalná a portable buildet anélkül, hogy bármi többletet adna. Az Opera Mini szóba sem jöhet: zárt forráskódú, és proxy-alapú megoldás, beágyazható motorja nincs.

A jelenlegi viselkedés az, hogy a találatkártya a külső alapböngészőben nyílik meg (`openSearchResult`, `App.vue:1501`, `opener` plugin), az archiváláshoz pedig a felhasználónak kézzel kell tartalmat rögzítenie. Ehelyett a találatok nyíljanak meg egy appon belüli néző ablakban:

- **Megvalósítás:** Tauri `WebviewWindow` (második ablak) vagy a fő ablakba ágyazott webview panel. A Tauri 2-ben a `tauri::WebviewWindowBuilder` backend oldalról, a `@tauri-apps/api/webviewWindow` frontend oldalról is tudja ezt; új függőség nem kell.
- **Archiválás egy kattintással:** a néző ablak mellett legyen „Forrás archiválása" gomb, amely a betöltött oldal címét, URL-jét és szövegét adja át a meglévő `archive_source` parancsnak. Az oldal szövege a webview-ból JavaScript-kiértékeléssel nyerhető ki (pl. `document.title`, `document.body.innerText`); így az archivált kivonat a tényleges tartalomból készül, nem kézi másolásból, ami a hash-elt archívum bizonyító erejét is növeli.
- **Auditnapló:** a néző ablakban történt megnyitás és az onnan indított archiválás kapjon saját eseménytípust (pl. `source_viewed`, `source_archived_from_viewer`), így a kutatási workflow zárt marad.
- **Esési ág:** a „Megnyitás külső böngészőben" lehetőség maradjon meg a néző ablak menüjében, mert lesz oldal (pl. bejelentkezést kívánó vagy webview-t blokkoló), amely csak rendes böngészőben használható.
- **Korlát, amit előre tudni kell:** néhány oldal (köztük a Google kereső is időnként) másképp viselkedik beágyazott webview-ban, CAPTCHA-t vagy consent-falat mutathat. Ez megjelenítésre nem akadály, automatikus kaparásra viszont nem szabad építeni rá; a találatlekérésre a fenti API-s út a stabil megoldás.

---

## 4. Backend (lib.rs) további javításai

- **Aszinkron keresés:** az `execute_search` blokkoló `reqwest::blocking` klienst használ 45 másodperces timeouttal. Tedd a parancsot `async`-má (async reqwest vagy `spawn_blocking`), különben AI-módban hosszan fagyottnak tűnhet a felület. A frontend `Promise.all`-ja most motoronként × lekérdezésenként külön AI-hívást is indít, ami hibrid módban ugyanazt az AI-kérdést akár 4–6-szor futtatja le: pénz- és időpazarlás, érdemes az AI-hívást futamonként egyszer végezni.
- **AI-válasz megbízhatósága:** OpenAI-nál kérd a `response_format: {"type":"json_object"}` paramétert, Gemininél a `responseMimeType: "application/json"`-t; így az `extract_json_object` kézi vágása ritkábban hasal el.
- **DPAPI PowerShell nélkül:** az `encrypt_secret_for_storage` minden mentésnél PowerShell-folyamatot indít (a `runSearch` ráadásul minden keresés előtt ment, tehát minden keresés PowerShell-spawnnal jár). Használd a `windows` crate `CryptProtectData`/`CryptUnprotectData` hívását vagy a `keyring` crate-et; gyorsabb, csendesebb, és nem törik el, ha a PowerShell tiltott vagy más profillal fut.
- **API-kulcs ne utazzon vissza a frontendre:** a `get_app_settings` a visszafejtett kulcsot adja a webview-nak. Elég egy `has_api_key: bool`; a kulcsot csak a backend használja.
- **Sémamigráció rendezetten:** a `CREATE TABLE` + tucatnyi lenyelt hibájú `ALTER TABLE` (`let _ = ...`) minta valódi hibákat is elrejt. Vezess `PRAGMA user_version` alapú, sorszámozott migrációkat; a hibát pedig csak akkor nyeld le, ha tényleg „duplicate column" jellegű.
- **Törlés következetesen:** a `delete_case` törli a forrásrekordokat, ám az `archive/` mappában lévő fájljaikat nem, így árva fájlok maradnak. Törlés előtt gyűjtsd ki a `file_path`-okat és töröld a fájlokat is (a `delete_source` mintájára), vagy legalább naplózd az elmaradást. Megfontolandó a tranzakcióba foglalás is (most négy külön DELETE fut).
- **SQLite beállítások:** kapcsold be a `PRAGMA foreign_keys = ON`-t (és vedd fel a hiányzó idegen kulcsokat), tegyél indexet a `sources.case_id`, `audit_logs.case_id`, `search_runs.case_id` oszlopokra, és fontold meg a WAL módot. Az `execute_search` most három külön kapcsolatot nyit egy futáson belül; egy kapcsolat újrahasznosítása tisztább.
- **Archivált tartalom kereshetősége:** a mentett forráskivonatokra érdemes SQLite FTS5 virtuális táblát tenni, hogy az ügyön belül teljes szövegű keresés is legyen – szakértői anyagnál ez sokat ér.

## 5. Frontend (App.vue) további javításai

- **Komponensbontás:** a 2910 soros `App.vue` tarthatatlanná válik. Bontsd legalább Keresés / Találatok / Archívum / Ügy / Beállítások komponensekre, az állapotot pedig vidd Pinia store-ba (a `SourceReviewPanel` már jó példa a leválasztásra).
- **Beállításmentés csak változáskor:** a `runSearch` minden futáskor meghívja a `saveAppSettings`-et (lásd a PowerShell-spawn problémát is). Mentés csak akkor kelljen, ha tényleg módosult valami.
- **Hibaüzenetek szétválasztása:** minden hiba ugyanabba az `appError` mezőbe ömlik, és a következő művelet felülírja. Legyen külön keresési, mentési és archiválási hibacsatorna, a fontos hibák pedig maradjanak látva, amíg a felhasználó el nem tünteti őket.
- **Üres eredmény UX:** a védőháló-üzenet mellett ajánlj konkrét lazítási lépéseket (tier-szűrő nullázása, idézőjelek elhagyása, évszűrő törlése) egy-egy kattintható gombbal.
- **Duplikátumszűrés kulcsa:** a `url|query_used` kulcs miatt ugyanaz az URL több lekérdezésváltozattal többször is megjelenik. Fontold meg az URL-alapú összevonást, a variánsok felsorolásával egy kártyán.

## 6. Minőségbiztosítás és üzemeltetés

- **Tesztek:** jelenleg nincs teszt. A legérzékenyebb, tisztán tesztelhető pontok: `buildYearClause`, `broadQuery`/`filteredQuery` összeállítás (JS), illetve `build_query_variants`, `detect_archive_metadata`, `extract_site_targets`, tier-szűrés (Rust, `#[cfg(test)]`). Pont ezekben lapultak a mostani hibák is.
- **Zéró találat naplózása:** kerüljön az auditnaplóba külön eseménytípus (`search_zero_results`) a lekérdezéssel és a szűrőállapottal; így a mostanihoz hasonló panasz visszakereshető lesz.
- **Repo-higiénia:** az `irbr_database.db`, a `build_output/`, a `dist/` és a 900 kB-os PDF ne legyen a repóban (vagy kerüljön `.gitignore`-ba); a git most detached HEAD állapotban áll, érdemes rendbe tenni. A projektmappa neve `IBR`, az app mindenhol `IRBR` néven fut, ezt az eltérést is érdemes egyszer eldönteni.

---

## 7. Megvalósítási állapot (2026-07-12)

A javaslatok nagy része beépült a kódba. Tételes állapot:

**Elkészült:**

- **2.1** Hibrid módban az AI-hiba már nem dobja el a natív kártyákat: a hiba `ai_error` mezőként megy vissza, a felület sárga figyelmeztetésben mutatja, az auditnapló `ai_search_error` eseményt kap.
- **2.2** Évszűrő javítva: nincs többé literál „utáni/előtti" szó; rövid tartománynál OR-lista, hosszabbnál `2020..2024`, fél-nyitott tartománynál `after:`/`before:` operátor.
- **2.3** A szűrt futam forráscsomagonként külön lekérdezést generál egyetlen óriás OR-blokk helyett; a lekérdezés-előnézet figyelmeztet a 32 szavas Google-limit átlépésére.
- **2.4** A Tier-szűrő jelentése mostantól „ez a tier és jobb" (`tier <= szűrő`), a legördülő címkéi ezt tükrözik (pl. „Tier 1-2").
- **2.5** A kulcsszóillesztés ékezettelenített, kisbetűs alakon fut (`normalize_for_match`), így az ékezet nélküli gépelés is aktiválja a variánsokat és a domain-specifikus kártyákat.
- **2.6** A régió mező már nem kerül kényszer-idézőjelbe.
- **2.7** A backend diagnosztikát ad vissza (generált / megjelenített / szűrő által elrejtett kártyák), a felület kiírja, és üresre szűrt futamnál egykattintásos „Tier-szűrő nullázása és újrafuttatás" gombot ajánl. Zéró találatnál `search_zero_results` auditesemény készül.
- **3.1** Találatnéző ablak: minden találatkártyán „Appon belül" gomb, amely a meglévő WebView2-vel külön ablakban nyitja az oldalt (`open_viewer_window` parancs, `source_viewed` auditesemény). A külső böngészős megnyitás megmaradt.
- **Archiválás valódi tartalommal:** archiváláskor az app letölti az oldalt (`fetch_page_text`), kinyeri a címét és szövegét (script/style nélkül, max 20 000 karakter), és ez kerül a hash-elt archívumba; keresőmotor-URL-eknél és sikertelen letöltésnél a kártyakivonat marad.
- **4. fejezetből:** `execute_search` aszinkron (nem blokkolja a felületet); AI-hívás futamonként egyszer fut, nem motoronként; OpenAI `response_format: json_object`, Gemini `responseMimeType: application/json`; `delete_case` tranzakcióban töröl és az archivált fájlokat is eltakarítja; indexek a `case_id` oszlopokra, `foreign_keys` és `busy_timeout` pragma; az API-kulcs nem utazik vissza a frontendre (`api_key_set` jelző, üres mentés megőrzi a régi kulcsot); változatlan kulcs mentésekor nincs felesleges DPAPI-kör.
- **5. fejezetből:** a keresés már nem menti feleslegesen a beállításokat minden futamnál; duplikátumszűrés URL alapján; üres eredménynél konkrét lazítási gomb; külön figyelmeztető sáv (sárga) a nem blokkoló hibáknak.
- **6. fejezetből:** 8 Rust egységteszt (`cargo test --lib`) az érzékeny függvényekre; a tesztelés közben kiderült és javítva lett egy további hiba: a `site:` felismerés nem működött zárójeles tokennél (`(site:ksh.hu`).

- **3. Google CSE integráció (2026-07-13):** valódi Google találatlekérés a Programmable Search Engine API-val. A három keresési mód logikája: natív mód = keresési terv + CSE valódi találatok (ha be van állítva), hibrid mód = ugyanez + AI-javaslatok, tisztán AI mód = változatlan, CSE nélkül. A CSE-kulcs ugyanazzal a DPAPI-titkosítással tárolódik, mint az AI-kulcs, és nem utazik vissza a frontendre. A találatok domain alapján automatikus tier-besorolást kapnak (njt/közlöny/KSH/gov.hu → T1, scholar/archive/edu → T2, reddit/facebook/jófogás → T4, egyéb → T3). CSE-hiba nem dobja el a futamot: sárga figyelmeztetés + `cse_search_error` auditesemény, sikeres lekérdezésnél `cse_search` esemény. A CSE lekérdezésenként egyszer fut, nem motoronként. Beállítás a keresési panel „Google CSE – valódi találatok" kártyáján (cx + API-kulcs); ingyenes keret napi 100 lekérdezés.

- **3/b. Brave Search API integráció (2026-07-14):** mivel a Google az új CSE-knél kivezette a teljes webes keresést (csak domainlistás kereső hozható létre), a teljes webes valódi találatokat a Brave Search API adja. A Brave a CSE mellett vagy helyett is futhat; mindkettő ugyanabba a kártyaformátumba, tier-besorolásba (`classify_result_tier`) és hibatűrésbe csatlakozik. A módlogika változatlan: natív és hibrid módban fut, tisztán AI módban nem. A Brave-kulcs DPAPI-val titkosítva tárolódik, lekérdezésenként egyszer fut (`brave_search` / `brave_search_error` auditesemények), hibája sárga figyelmeztetés, nem futamhiba. Beállítás a „Brave Search – teljes webes találatok" kártyán; kulcs a brave.com/search/api oldalról, az ingyenes csomag kb. havi 2000 lekérdezés.

- **3/c. Tavily integráció (2026-07-14):** mivel a Brave Search API-nak nincs ingyenes csomagja, a bankkártya nélküli, ingyenes teljes webes találatlekérést a Tavily adja (havi 1000 lekérdezés, e-mailes regisztráció a tavily.com oldalon, `tvly-...` kulcs). Ugyanaz a minta, mint a CSE-nél és a Brave-nél: DPAPI-titkosított kulcs, natív + hibrid módban fut, tisztán AI-ban nem, lekérdezésenként egyszer, tier-besorolás domain alapján, hibája sárga figyelmeztetés (`tavily_search` / `tavily_search_error` auditesemények). A javasolt felállás: CSE (kurált domainlista) + Tavily (teljes web); a Brave-kártya megmaradt annak, akinek van előfizetése.

- **3/d. Facebook csoportkeresés javítása (2026-07-14):** a Facebook nyilvános csoportokban keresés kulcsfontosságú adatforrás, ám három ok miatt nem adott találatot: a „Facebook csoportok" forráscsomag alapból ki volt kapcsolva; a Tavily nem érti a `site:facebook.com` operátort; a domainlistás CSE-ben nem szerepelt a facebook.com. Javítások: a Facebook-csomag mostantól alapból bekapcsolt, így a szűrt/kombinált futamok mindig készítenek Facebookra célzott lekérdezést; a Tavily-hívás a `site:` szűrős lekérdezéseket automatikusan `include_domains` paraméterre fordítja le (a `site:` tokenek és az árva OR-ok kikerülnek a szövegből, a domain a szűrőbe kerül) – ez minden `site:` alapú forráscsomagnak jót tesz, nem csak a Facebooknak. Felhasználói teendő: a CSE domainlistájába fel kell venni a `*.facebook.com` mintát, mert a Facebookot a Google indexe fedi le a legjobban. Korlát: a bejelentkezés mögé zárt csoporttartalom egyik keresőben sem jelenik meg, oda csak a kézi Facebook-keresés lát be.

**Későbbre maradt:**

- További találatlekérők (SerpAPI) vagy grounding-os AI-keresés igény esetén.
- DPAPI natív hívása a `windows` crate-tel a PowerShell-spawn helyett (a spawn gyakorisága már minimalizálva).
- `App.vue` komponensbontás és Pinia; SQLite FTS5 az archivált tartalmakra; WAL mód; JS egységtesztek (vitest); repo-higiénia (adatbázis és build-mappák kivezetése a repóból).

## 8. Javasolt sorrend

1. **2.1** AI-hiba ne dobja el a natív találatokat (pár soros javítás, azonnali hatás).
2. **2.2** évszűrő javítása (`utáni/előtti` törlése, `2020..2024` alak).
3. **2.4** tier-szűrő viselkedésének javítása + **2.7** diagnosztika (hány kártya esett ki és miért).
4. **2.3** és **2.6** lekérdezés-összeállítás lazítása, szólimit-figyelmeztetés.
5. **2.5** ékezettelenített kulcsszóillesztés.
6. **3.1** appon belüli találatnéző ablak a meglévő WebView2-re építve, egykattintásos archiválással.
7. **3.** valódi találatlekérés (Search API vagy grounding-os AI) mint nagyobb fejlesztés.
8. A 4–6. fejezet tételei folyamatos karbantartásként.
