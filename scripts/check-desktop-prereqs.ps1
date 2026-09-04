$ErrorActionPreference = "Stop"

$issues = @()

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  $issues += "A Rust toolchain nincs telepitve vagy a cargo nincs PATH-ban."
}

if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) {
  $issues += "A rustc nem erheto el."
}

$linkCommand = Get-Command link.exe -ErrorAction SilentlyContinue
$knownLinkPaths = @(
  "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC",
  "C:\Program Files\Microsoft Visual Studio\17\Community\VC\Tools\MSVC",
  "C:\Program Files\Microsoft Visual Studio\18\BuildTools\VC\Tools\MSVC",
  "C:\Program Files\Microsoft Visual Studio\17\BuildTools\VC\Tools\MSVC"
)

$hasVisualCppTools = $false
foreach ($path in $knownLinkPaths) {
  if (Test-Path $path) {
    $hasVisualCppTools = $true
    break
  }
}

if (-not $linkCommand -and -not $hasVisualCppTools) {
  $issues += "A link.exe vagy a Visual C++ toolchain nem erheto el. Telepitsd a Visual Studio Build Tools csomagot C++ build eszkoztarral."
}

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
  $issues += "Az npm nem erheto el."
}

if ($issues.Count -gt 0) {
  Write-Host "Desktop build eloellenorzes: HIBA" -ForegroundColor Red
  $issues | ForEach-Object { Write-Host "- $_" -ForegroundColor Yellow }
  exit 1
}

Write-Host "Desktop build eloellenorzes: OK" -ForegroundColor Green
