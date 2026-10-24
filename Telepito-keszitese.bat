@echo off
rem ===========================================================================
rem  IRBR - telepito (setup.exe) keszitese EGY KATTINTASSAL.
rem  Ezt a fejleszto futtatja egyszer; a felhasznalok csak a kesz setup.exe-t
rem  kapjak meg, nekik semmit nem kell futtatniuk vagy telepiteniuk elozetesen.
rem ===========================================================================
setlocal
cd /d "%~dp0"
title IRBR telepito keszitese

echo.
echo  IRBR telepito keszitese
echo  =======================
echo.

where node >nul 2>nul
if errorlevel 1 goto no_node
where cargo >nul 2>nul
if errorlevel 1 goto no_rust

echo [1/3] Fuggosegek telepitese (npm install)...
call npm install
if errorlevel 1 goto fail

echo.
echo [2/3] Az alkalmazas es a telepito epitese (ez nehany percig tarthat)...
call npm run tauri build
if errorlevel 1 goto fail

echo.
echo [3/3] Kesz!
set "OUT=%~dp0src-tauri\target\release\bundle\nsis"
echo.
echo  A telepito itt van:
echo    %OUT%
echo.
echo  A *.exe fajlt kell tovabbadni a felhasznaloknak: dupla kattintassal telepitheto,
echo  nem kell hozza rendszergazdai jog, PowerShell vagy mas program.
echo.
if exist "%OUT%" start "" "%OUT%"
pause
exit /b 0

:no_node
echo  HIBA: a Node.js nincs telepitve. Toltsd le es telepitsd: https://nodejs.org  (LTS verzio)
echo  Utana inditsd ujra ezt a fajlt.
goto end_fail

:no_rust
echo  HIBA: a Rust nincs telepitve. Toltsd le es telepitsd: https://rustup.rs
echo  (A telepitesnel a "Visual Studio C++ Build Tools" is kell; a telepito felajanlja.)
echo  Utana inditsd ujra ezt a fajlt.
goto end_fail

:fail
echo.
echo  HIBA tortent az epites kozben. Nezd meg a fenti hibauzenetet, es kuldd el a fejlesztonek.

:end_fail
echo.
pause
exit /b 1
