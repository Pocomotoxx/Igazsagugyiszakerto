# IRBR — Fourier-fraktáldimenzió + Lévy-keresés beépítési tervezet

**Státusz:** TERVEZET · CANDIDATE_ONLY · `rfc: irbr-fractal-levy` PROPOSED / RECOMMEND_HOLD
**Cél:** a két jel/keresés-módszert az **IRBR (Igazságügyiszakértő) adat-rétegébe** építeni — nem a KRISTA/FRR-be —, a KRISTA/FRR-rel mint **episztemikus kormányzóval**. A cél a te célod: *a kész adatokból visszanézni (retrodikció)*, forenzikus kontextusban.
**Evidencia:** az architektúra-illesztés `DECLARED` (az IRBR e sessionben megismert felépítésére építve: Tauri 2 + Vue 3 + Rust, rusqlite, a PR #1-ben bevezetett `evidence_level` = CONSTRUCTED/RETRIEVED/SUGGESTED). Az empirikus haszon `UNVERIFIABLE`, amíg nem mérjük (8.).
**Ez nem:** repó-módosítás; nem ígéret, hogy működik; a fraktál/Lévy neurálisháló-csomagolása (azt elvetettük — itt csak a *jel/keresés* magjuk kell).

---

## 0. Vezérelv (a forenzikus/jogi kontextus miatt kötelező)

> **Egy jel-alapú anomália vagy egy Lévy-találat NYOM (lead), SOSEM bizonyíték.**

Igazságügyi környezetben egy statisztikai jel legfeljebb *irányítja* a szakértő figyelmét — nem bizonyít, nem vádol, nem következtet okra. Ezért minden kimenet a KRISTA episztemikus fegyelme alá kerül (4.). Ez nem dísz: ez teszi a módszert egyáltalán **használhatóvá** jogi kontextusban.

## 1. Hova illik az IRBR-ben

Az IRBR már kezel adatot és **már van kimenet-címkéző rendszere** (`evidence_level`, PR #1). A két új modul ebbe illeszkedik:
- **A modul (Fourier/fraktál):** *idővonal-/sorozat-elemzés* — regime-váltás, anomália, retrodikció. Feltétel: az IRBR-nek legyen (vagy tudjon fogadni) **szekvenciális/temporális adatot** (esemény-, kommunikáció-, tranzakció-, aktivitás-sorozat). Ha ilyen ma nincs, a modul **feltételes** (lásd 10.).
- **B modul (Lévy-keresés):** a *bizonyíték-/dokumentumtér feltárása* — a meglévő keresés kiegészítése egy „néha ugorj távolra" operátorral.

## 2. A modul — Fourier-fraktáldimenzió / spektrális regime-detektor

**Mit eszik:** egy numerikus idősor (pl. napi eseményszám, kommunikációs gyakoriság, tranzakció-volumen egy ügyben).
**Mit ad:** egy **skalár szerkezet-ujjlenyomat** csúszóablakonként, és annak **változása** = regime-váltás / anomália jelzése. Retrodikció: a *kész* sorozatból visszamutat, **mikor** és **hol** tért el a mögöttes folyamat.

**Matek — őszinte pontosítás:**
- A prototípus `d_F = (5−β)/2` képlete **2D képekre** való. Egy **1D idősorra** a spektrális exponens a Hurst-paraméterhez (H) kötődik; a fraktáldimenzió D ≈ 2−H egy 1D nyomvonalra. A képletet a **dimenzióhoz kell igazítani** — a prototípus ezt nem teszi.
- **Robusztusabb választás nemstacionárius forenzikus adatra: DFA (Detrended Fluctuation Analysis)** a nyers FFT-meredekség helyett — kevésbé érzékeny a trendre. Javaslat: **DFA-alapú skálázási exponens + csúszóablakos változás-detektálás**, nem a nyers spektrum-slope.
- Ez tiszta **numerikus rutin** — nincs szükség neurális hálóra, torch-ra, tréningre.

**Hol számol:** **pure Rust**, lokálisan (Tauri backend). FFT: `rustfft` crate vagy kézi DFA; nincs ML-függőség, nincs hálózat — illeszkedik a desktop-app és a SEC-fixek szelleméhez.

## 3. B modul — Lévy-keresés (bizonyítéktér-feltárás)

**Mit csinál:** a meglévő keresést/retrievalt egészíti ki egy **Lévy-flight feltárási operátorral** — főleg lokális finomítás (kapcsolódó dokumentumok), de **ritka nagy ugrások** távoli, mégis potenciálisan releváns régiókba (nem-nyilvánvaló kapcsolatok). A Lévy-mintavétel a prototípus CMS-algoritmusával, pure Rustban.
**Mit ad:** **jelölt** dokumentumok/kapcsolatok — coverage, nem igazság.
**Miért hasznos:** a forenzikus kutatás pont a nem-nyilvánvaló összefüggéseket keresi; a kulcsszó-alapú keresés lokális zsákutcákba ragad. A Lévy-ugrás ezt oldja.

## 4. Episztemikus kormányzás (KRISTA/FRR) — a plumbing már részben megvan

Minden kimenet a fegyelem alá kerül; a `evidence_level` hook **már létezik** az IRBR-ben (PR #1):
- **A modul anomáliája → `SUGGESTED`** evidencia-szint. Soha nem „talált szabálysértés", csak „itt megváltozott a minta, érdemes megnézni".
- **B modul találata → `CONSTRUCTED`** (a rendszer állította elő), nem `RETRIEVED` tény.
- **`action-gate` (KRISTA M-1):** mielőtt egy jel *következtetést* hajtana (pl. jelentésbe kerül), a művelet visszafordíthatósága/tétje szerint kell a bizonyosság — jogi jelentésbe írni magas tét → magas léc; `ABSTAIN` → nem következtetünk, a szakértő dönt.
- **`verdict` / `closure-scope` (FRR):** tilos a jel-alapú anomáliát bizonyítékként lezárni; a következtetés nem lépheti túl a jel hatókörét.
- **`record-hygiene`:** a diagnosztikai rekordok **referenciák**, nem nyers érzékeny tartalom; titok-alakú érték elbukik.
- **FRR retrodikció-lencse:** a „visszanézés" magát az FRR backward-lencséi + residual-location fegyelme keretezi (mi hordozta a változást?).

## 5. Technikai illesztés

- **Elhelyezés:** új Rust-modul(ok) az IRBR backendben (`fractal.rs`, `levy_search.rs`), Tauri-parancsként a Vue-frontend felé.
- **Függőségek:** minimál (`rustfft` vagy kézi DFA; a Lévy CMS kézi). **Nincs** ML-stack.
- **Determinizmus:** a Lévy-mintavétel seedelhető (reprodukálható forenzikus futás — jogi követelmény!). A d_F/DFA determinisztikus.
- **Teljesítmény:** csúszóablakos DFA olcsó; a Lévy-keresés a meglévő indexre épül.
- **Tárolás:** a diagnosztikai eredmények a rusqlite-ba, `evidence_level`-lel + `PRAGMA user_version` migrációval (a DB-1 mintára).

## 6. UI/UX (a szakértő szemszöge)

- **A modul:** az ügy-idővonalon **jelölő** a regime-váltásnál, `SUGGESTED` címkével + „miért" (a szerkezet-ujjlenyomat változása). Kattintásra a nyers ablak, nem egy verdikt.
- **B modul:** „**Tárd fel távolabb**" gomb a keresés mellett — a Lévy-jelölteket `CONSTRUCTED`-ként, külön sávban, hogy ne keveredjenek a `RETRIEVED` tényekkel.
- **Mindig látható** az evidencia-szint és a „ez nyom, nem bizonyíték" figyelmeztetés.

## 7. Biztonság és adatvédelem

- **Lokális számítás** (Tauri desktop, hálózat nélkül) — érzékeny forenzikus adat nem hagyja el a gépet.
- A meglévő SEC-fixek (SEC-1/2/3) szelleme: nincs kulcs-szivárgás, IP-guard, méret-korlát.
- `record-hygiene`: a diagnosztika metaadat, nem nyers bizonyíték-tartalom.
- **Auditálhatóság:** minden futás seedelt és naplózott (jogi reprodukálhatóság).

## 8. Előregisztrált mini-teszt (jelez-e egyáltalán valamit?)

A fegyelmünk szerint **nem építjük be hitre**. Mielőtt élesbe kerül:
- **A modul hipotézise:** a DFA-exponens csúszóablakos változása **korrelál** ismert, címkézett regime-váltásokkal (pl. szintetikus vagy anonimizált, ismert töréspontú sorozatokon).
- **Cáfolat (kill):** ha a detektált „anomáliák" nem esnek egybe az ismert töréspontokkal a véletlennél jobban → a modul nem jelez, elvetjük.
- **B modul hipotézise:** a Lévy-keresés **több releváns, nem-nyilvánvaló** találatot hoz, mint a puszta lokális keresés, elfogadható zaj mellett — külső bíró ítéli meg (a validációs csomag rubrika-mintájára).
- **Előregisztráció:** metrikák, küszöbök, kill-criteria a futtatás előtt rögzítve (a meglévő előregisztrációs sablon mintájára).

## 9. Fázisok

1. **Adat-audit:** van-e az IRBR-ben szekvenciális/temporális adat? Ha nincs, előbb ingestion (feltételes).
2. **A modul MVP:** DFA + csúszóablakos regime-jel, szintetikus adaton, mini-teszttel.
3. **Governance-kötés:** `SUGGESTED` címke + `action-gate` a következtetés előtt.
4. **B modul:** Lévy-feltárás a meglévő keresésre, `CONSTRUCTED` sávban.
5. **Éles próba** egy valós (anonimizált) ügyön, külső értékeléssel.

Minden fázis kapuval; bukás → elvetés/pivot, nem továbberőltetés (`action-gate` a fázislépésre).

## 10. Nyitott döntések (a te válaszod kell)

1. **Van-e ma az IRBR-ben idősor/szekvencia-adat?** (Ez dönti el, hogy az A modul azonnal indulhat-e, vagy előbb ingestion kell.)
2. **DFA vagy nyers FFT-slope?** (Javaslat: DFA — robusztusabb nemstacionárius adatra.)
3. **A Lévy-keresés a meglévő index/embedding fölött fusson, vagy külön feltáró-módban?**
4. **Melyik modul előbb?** (Javaslat: A modul — közelebb a „retrodikció" célodhoz.)

## Amit ez a tervezet szándékosan NEM tesz
- Nem építi a neurális háló-csomagolást (csak a jel/keresés-magot).
- Nem állítja, hogy egy jel bizonyíték — mindig `SUGGESTED`/`CONSTRUCTED`, `action-gate` mögött.
- Nem emel `assurance`-t `VERIFIED`-re, és nem épít be hitre — a 8. mini-teszt a kapu.
- Nem módosít semmit a repókban — CANDIDATE_ONLY.
