# IRBR telepítése (felhasználóknak)

Nem kell PowerShell, parancssor vagy más program. Három lépés:

1. **Töltsd le** a kapott telepítőt (`IRBR_..._x64-setup.exe`).
2. **Kattints rá duplán.** Ha a Windows „A Windows megvédte a számítógépét" üzenetet mutat, kattints a **További információ** szövegre, majd a **Futtatás mindenképpen** gombra (ez a program még nincs digitálisan aláírva, ezért kérdez rá).
3. Kövesd a telepítőt (**Tovább**, **Telepítés**, **Befejezés**). Rendszergazdai jog nem kell.

Ezután az **IRBR** a Start menüben és az asztalon is megtalálható, onnan indítható.

**Eltávolítás:** Windows Beállítások → Alkalmazások → IRBR → Eltávolítás.

**Az adataid** (ügyek, archivált források) a felhasználói profilban maradnak (`%APPDATA%\hu.irbr.desktop`). Eltávolításkor alapból nem törlődnek (a telepítő külön megkérdezi), frissítésnél megmaradnak.

## Ha nem indul el

- A Windows 10-en lehet, hogy a telepítő első indításkor letölti a Microsoft WebView2 komponenst (internet kell hozzá); Windows 11-en ez már a rendszer része.
- Ha a program mégsem nyílik meg, kérd a fejlesztő segítségét, és küldd el az üzenetet, amit látsz.

---

# A telepítő elkészítése (fejlesztőknek)

**A) GitHubon, semmit nem kell telepíteni (ajánlott).** GitHub → *Actions* → **Windows telepítő** → *Run workflow*. Pár perc múlva a futás oldalán, az *Artifacts* alatt letölthető az `IRBR-telepito` (benne a `setup.exe`). Címkével (`git tag v0.1.0 && git push origin v0.1.0`) vázlat-kiadás is készül.

**B) Saját Windows gépen:** dupla kattintás a repó gyökerében lévő **`Telepito-keszitese.bat`** fájlra (előfeltétel: Node.js és Rust az MSVC build toolsszal). A kész telepítő a `src-tauri\target\release\bundle\nsis` mappába kerül, a program megnyitja.

**Később érdemes:** kódaláírási tanúsítvány (megszünteti a SmartScreen-figyelmeztetést), és automatikus frissítés (Tauri updater).
