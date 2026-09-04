# IRBR – Audit utókövetés

Dátum: 2026-07-03
Forrás: [service.md](C:/Bárkányi%20Edit/IBR/service.md)

## Állapotösszegzés

| # | Eredeti pont | Állapot | Megjegyzés |
|---|---|---|---|
| 1 | AI API-hívás PowerShell-en keresztül, kulccsal a lemezen | Javítva | Az AI-hívás natív Rust `reqwest` kliensre került, nincs temp fájlos PowerShell hálózati hívás. Lásd: [src-tauri/src/lib.rs](C:/Bárkányi%20Edit/IBR/src-tauri/src/lib.rs:675) |
| 2 | API-kulcs titkosítatlanul az SQLite-ban | Javítva | Az `api_key` DPAPI-védett formában mentődik és betöltéskor visszafejtődik. Lásd: [src-tauri/src/lib.rs](C:/Bárkányi%20Edit/IBR/src-tauri/src/lib.rs:345) |
| 3 | `.gitignore` nem zárja ki az adatbázisfájlt | Javítva | DB-kizárás és build output kizárás bekerült. Lásd: [.gitignore](C:/Bárkányi%20Edit/IBR/.gitignore:19) |
| 4 | `db_path` hardkódolt relatív útvonal | Javítva | A DB AppData alá kerül Tauri setup során. Lásd: [src-tauri/src/lib.rs](C:/Bárkányi%20Edit/IBR/src-tauri/src/lib.rs:1918) |
| 5 | Túl engedékeny Tauri CSP | Részben javítva | A CSP szűkebb lett, de jelenleg `connect-src 'self'`, ami később tovább finomhangolható, ha explicit domain-listát akarunk. Lásd: [src-tauri/tauri.conf.json](C:/Bárkányi%20Edit/IBR/src-tauri/tauri.conf.json:21) |
| 6 | Fájlnév-ütközés archiváláskor/riportmentésnél | Javítva | Az archivum milliszekundumos, a riport részletesebb időbélyeges fájlnevet használ. Lásd: [src-tauri/src/lib.rs](C:/Bárkányi%20Edit/IBR/src-tauri/src/lib.rs:1803) |
| 7 | `delete_source` elnyeli a fájltörlési hibát | Javítva | A hibát naplózza és hibával visszatér, nem törli némán csak a DB rekordot. Lásd: [src-tauri/src/lib.rs](C:/Bárkányi%20Edit/IBR/src-tauri/src/lib.rs:1428) |
| 8 | `search_query` mérete nem korlátozott | Javítva | 2000 karakteres limit került be. Lásd: [src-tauri/src/lib.rs](C:/Bárkányi%20Edit/IBR/src-tauri/src/lib.rs:12) és [src-tauri/src/lib.rs](C:/Bárkányi%20Edit/IBR/src-tauri/src/lib.rs:1703) |
| 9 | Vegyes ékezetes/ékezet nélküli magyar szöveg | Részben javítva | Több üzenet egységesítve lett, de a teljes szövegkészlet még nem teljesen konzisztens. Lásd például: [src-tauri/src/lib.rs](C:/Bárkányi%20Edit/IBR/src-tauri/src/lib.rs:1216), [src-tauri/src/lib.rs](C:/Bárkányi%20Edit/IBR/src-tauri/src/lib.rs:1242) |
| 10 | Nincs TODO/FIXME/HACK marker | Már nem releváns | Ez nem hiba volt, csak megjegyzés. |

## Rövid értékelés

- A kritikus pontok érdemben lezárva.
- A magas súlyosságú pontok lezárva, a CSP maradt részben további finomításra nyitva.
- A közepes pontok lezárva.
- Az alacsony súlyosságú pontokból csak kozmetikai egységesítés maradt.

## Javasolt maradék teendő

- Ha később fix AI szolgáltatókra szűkítjük a rendszert, a CSP `connect-src` szabálya finomhangolható explicit domain-listára.
