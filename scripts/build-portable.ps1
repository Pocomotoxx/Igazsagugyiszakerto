$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$releaseRoot = Join-Path $root "release"
$portableRoot = Join-Path $releaseRoot "portable"
$appRoot = Join-Path $portableRoot "IRBR-portable"
$releaseBinRoot = Join-Path $root "src-tauri\target\release"
$readmeSource = Join-Path $root "README.md"
$worklogSource = Join-Path $root ".libary\WORKLOG.md"
$checkScript = Join-Path $PSScriptRoot "check-desktop-prereqs.ps1"
$zipTarget = Join-Path $portableRoot "IRBR-portable.zip"
$portableReadmeTarget = Join-Path $appRoot "ELSO-INDITAS.txt"
$manifestTarget = Join-Path $appRoot "RELEASE-MANIFEST.json"
$packageJsonPath = Join-Path $root "package.json"
$tauriConfigPath = Join-Path $root "src-tauri\tauri.conf.json"

& powershell -ExecutionPolicy Bypass -File $checkScript

Push-Location $root
try {
  npx tauri build --no-bundle
}
finally {
  Pop-Location
}

$exeCandidate = Get-ChildItem $releaseBinRoot -Filter *.exe |
  Where-Object { $_.Name -notin @("tauri-driver.exe") } |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 1

if (-not $exeCandidate) {
  throw "A portable buildhez nem talalhato futtathato allomany ebben a mappaban: $releaseBinRoot"
}

if (Test-Path $appRoot) {
  Remove-Item $appRoot -Recurse -Force
}

New-Item -ItemType Directory -Path $appRoot -Force | Out-Null
Copy-Item $exeCandidate.FullName (Join-Path $appRoot "IRBR.exe") -Force

if (Test-Path $readmeSource) {
  Copy-Item $readmeSource (Join-Path $appRoot "README.md") -Force
}

if (Test-Path $worklogSource) {
  Copy-Item $worklogSource (Join-Path $appRoot "WORKLOG.md") -Force
}

$packageInfo = Get-Content $packageJsonPath -Raw | ConvertFrom-Json
$tauriInfo = Get-Content $tauriConfigPath -Raw | ConvertFrom-Json
$builtAt = Get-Date -Format "yyyy-MM-dd HH:mm:ss"

$portableInfo = @"
IRBR Portable csomag

Tartalom:
- IRBR.exe
- README.md
- WORKLOG.md

Megjegyzes:
- A futtatas Windows alatt WebView2 runtime-ot igenyelhet.
- A helyi adatbazis es a riport/archivum fajlok a felhasznaloi alkalmazasadatok ala kerulnek.
"@

Set-Content -Path (Join-Path $appRoot "PORTABLE.txt") -Value $portableInfo -Encoding UTF8

$firstRunInfo = @"
IRBR - elso inditas

1. Futtasd az `IRBR.exe` fajlt ebbol a mappabol.
2. Ha a Windows vedelmi figyelmeztetest ad, engedelyezd a helyi futtatast.
3. Ha hianyzo megjelenitesi komponens hibat kapsz, telepitsd a Microsoft Edge WebView2 Runtime-ot.
4. Az alkalmazas a helyi adatbazist, az archivumot es a riportokat a felhasznaloi alkalmazasadatok alatt hozza letre.

Build adatok:
- termek: $($tauriInfo.productName)
- verzio: $($packageInfo.version)
- build ido: $builtAt
- forras exe: $($exeCandidate.Name)
"@

Set-Content -Path $portableReadmeTarget -Value $firstRunInfo -Encoding UTF8

$manifest = [ordered]@{
  productName = $tauriInfo.productName
  version = $packageInfo.version
  identifier = $tauriInfo.identifier
  buildTime = $builtAt
  sourceExecutable = $exeCandidate.Name
  packagedExecutable = "IRBR.exe"
  packageType = "portable"
  files = @(
    "IRBR.exe",
    "README.md",
    "WORKLOG.md",
    "PORTABLE.txt",
    "ELSO-INDITAS.txt",
    "RELEASE-MANIFEST.json"
  )
}

$manifest | ConvertTo-Json -Depth 4 | Set-Content -Path $manifestTarget -Encoding UTF8

if (Test-Path $zipTarget) {
  Remove-Item $zipTarget -Force
}

Compress-Archive -Path (Join-Path $appRoot "*") -DestinationPath $zipTarget -Force

Write-Host "Portable csomag elkeszult:" -ForegroundColor Green
Write-Host "Forras futtathato: $($exeCandidate.FullName)"
Write-Host $appRoot
Write-Host $zipTarget
