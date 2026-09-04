# IRBR – Biztonsági és kódminőségi audit jelentés

## Friss állapot 2026-07-03

Ez a fájl az eredeti 2026-07-01-es auditjelentést tartalmazza.
Az audit utókövetése és a jelenlegi javítási állapot külön fájlban érhető el:

- [service-followup.md](C:/Bárkányi%20Edit/IBR/service-followup.md)

Rövid összegzés:

- A kritikus pontok javítva.
- A magas súlyosságú pontok döntő többsége javítva.
- A közepes súlyosságú pontok javítva.
- Nyitott maradvány főként kozmetikai és további finomhangolási jellegű.

Dátum: 2026-07-01
Módszer: read-only kódaudit (Rust/Tauri backend, Vue frontend, adatbázis-réteg, build szkriptek)

## Áttekintés

Az IRBR egy Tauri v2 + Vue 3 + Rust (rusqlite) asztali alkalmazás, igazságügyi szakértői kutatástámogatásra, SQLite adatbázissal és auditnaplóval.

## Kritikus problémák

1. **AI API-hívás PowerShell-en keresztül, kulccsal a lemezen**
   `src-tauri/src/lib.rs:684-714` (`request_ai_search_response`)
   Minden AI-keresésnél egy PowerShell szkript íródik ki `%TEMP%`-be, benne az `Authorization: Bearer {api_key}` fejléccel plaintext-ben, majd `-ExecutionPolicy Bypass`-szal fut le. A fájlt utólag törli a kód (712-714. sor), de crash vagy race condition esetén a kulcsot tartalmazó fájl a lemezen maradhat.
   **Javaslat:** natív Rust HTTP klienssel (pl. `reqwest`) kiváltani a PowerShell-alapú megoldást.

2. **API-kulcs titkosítatlanul az SQLite-ban**
   `src-tauri/src/lib.rs:342-362` (`save_app_settings_internal`), `1176-1195` (`save_app_settings`)
   Az `api_key` mező egyszerű `TEXT` oszlopként kerül az `app_settings` táblába, OS-szintű secure storage (Windows Credential Manager / DPAPI) nélkül.
   **Javaslat:** a kulcsot ne az adatbázisban, hanem OS-szintű titkosított tárolóban kezelni.

## Magas súlyosságú problémák

3. **`.gitignore` nem zárja ki az adatbázisfájlt**
   A gyökér `.gitignore` nem tartalmaz `*.db` mintát. Ha a projekt git alá kerül, ügyadatok és a plaintext API-kulcs bekerülhetnek a repóba.
   **Javaslat:** `*.db`, `/irbr_database.db`, `/build_output/` hozzáadása a `.gitignore`-hoz.

4. **`db_path` hardkódolt relatív útvonal**
   `src-tauri/src/lib.rs:1862`: `let db_path = "irbr_database.db".to_string();`
   Az útvonal a folyamat aktuális munkakönyvtárához relatív, nem az OS alkalmazásadat-mappájából (`tauri::path::BaseDirectory::AppData`) származik. Portable buildnél írásvédett mappából indítva hibázhat, több példány esetén adatszóródást okozhat.
   **Javaslat:** az adatbázis és az archívum/riport könyvtárak elérési útját az OS alkalmazásadat-mappájából vezetni le.

5. **Túl engedékeny Tauri CSP**
   `src-tauri/tauri.conf.json:21`: `connect-src 'self' https: http:; img-src 'self' asset: data: https: http:; ...`
   A `connect-src`/`img-src` gyakorlatilag bármely HTTP/HTTPS célhoz enged kapcsolódást.
   **Javaslat:** a szabályt konkrét, ténylegesen szükséges domainekre szűkíteni.

## Közepes súlyosságú problémák

6. **Fájlnév-ütközés archiváláskor/riportmentésnél**
   `src-tauri/src/lib.rs:1747` (`format!("src_{case_id}_{}.txt", Local::now().timestamp())`) és `1841-1844` — másodperc-pontosságú időbélyeg, gyors egymásutáni műveletnél csendes felülírás/adatvesztés lehetséges bizonyítékfájloknál.
   **Javaslat:** UUID vagy nanoszekundum-pontosságú egyedi azonosító a fájlnevekbe.

7. **`delete_source` elnyeli a fájltörlési hibát**
   `src-tauri/src/lib.rs:1397-1399`: `let _ = fs::remove_file(&file_path);` — sikertelen törlés esetén a DB-rekord törlődik, a fájl árván marad, naplózás nélkül.
   **Javaslat:** a hibát naplózni, és/vagy a DB-törlést a fájltörlés sikerességéhez kötni.

8. **`search_query` mérete nem korlátozott**
   `src-tauri/src/lib.rs:1640-1706` — nincs hossz-limit a keresési szövegre, ami korlátlan méretben kerül be az `audit_logs` táblába is.
   **Javaslat:** ésszerű hossz-limit bevezetése.

## Alacsony súlyosságú megjegyzések

9. Vegyes ékezetes/ékezet nélküli magyar szöveg a felhasználói üzenetekben (pl. `lib.rs:311` vs. `1194`) — kozmetikai inkonzisztencia.
10. Nincs TODO/FIXME/HACK marker a kódban — nem hiba, csak megjegyzés.

## Pozitív megállapítások

- **SQL-injekció ellen mintaszerűen védett**: minden adatbázis-hozzáférés parametrizált (`rusqlite` `params![]`), sehol nincs string-konkatenáció SQL-lekérdezésben.
- **URL-encoding helyesen implementálva** (`url_encode`, `lib.rs:383-397`).
- **Tauri capabilities minimalisták**: `src-tauri/capabilities/default.json` csak `core:default` és `opener:default` jogot ad, nincs tágra nyitott fs/shell engedély.
- **Nincs `v-html`/`innerHTML` XSS-kockázat** a Vue kódban.
- **Nincs forráskódba égetett titok/API-kulcs.**
- **Konzisztens hibakezelés**: minden `#[tauri::command]` `Result<T, String>`-et ad vissza, `.map_err(|e| e.to_string())?` mintával.
- **Auditnapló funkció ténylegesen működik** a README-ben ígért módon.
- **Build szkriptek** (`scripts/*.ps1`) nem tartalmaznak veszélyes távoli kód letöltést/futtatást.

## Teendők összefoglalása (prioritás szerint)

| # | Prioritás | Feladat |
|---|-----------|---------|
| 1 | Kritikus | AI API-hívás natív HTTP klienssel (pl. `reqwest`), PowerShell-kitérő megszüntetése |
| 2 | Kritikus | API-kulcs OS-szintű secure storage-ban, ne plaintext SQLite-ban |
| 3 | Magas | `.gitignore` kiegészítése (`*.db`, `/build_output/`) |
| 4 | Magas | `db_path` és kapcsolódó útvonalak az OS AppData mappájából |
| 5 | Magas | CSP `connect-src`/`img-src` szűkítése |
| 6 | Közepes | Egyedi azonosítók az archív/riport fájlnevekben |
| 7 | Közepes | Fájltörlési hiba naplózása a `delete_source`-nál |
| 8 | Alacsony | Hossz-limit a `search_query`-re |
| 9 | Alacsony | Ékezethasználat egységesítése |
