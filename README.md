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
