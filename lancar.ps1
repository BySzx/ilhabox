# Lança uma versão nova da IlhaBox: sobe a versão (1.0.3 -> 1.0.4), commit, tag e push.
# O GitHub compila e publica em ~15 min; a IlhaBox instalada mostra "Atualizar" no celular e nos ajustes.
# A Ilha de casa roda este script sozinha (com -SemPausa) quando o código muda.
param([switch]$SemPausa)
$ErrorActionPreference = 'Continue'
Set-Location $PSScriptRoot
# O workflow vem na raiz (as ferramentas remotas não escrevem em .github) e vai pro lugar aqui.
if (Test-Path 'release.yml') {
  New-Item -ItemType Directory -Force '.github\workflows' | Out-Null
  Move-Item -Force 'release.yml' '.github\workflows\release.yml'
}

function Done($msg, $code) {
  if ($code -eq 0) { Write-Host "`n✔ $msg" -ForegroundColor Green } else { Write-Host "`n✖ $msg" -ForegroundColor Red }
  if (-not $SemPausa) { Read-Host 'Enter pra fechar' }
  Write-Output $msg
  exit $code
}

if (-not (Test-Path '.git')) { Done 'Rode o publicar.ps1 primeiro (configura o GitHub).' 1 }
git diff --quiet; $dirty = $LASTEXITCODE -ne 0
git diff --cached --quiet; if ($LASTEXITCODE -ne 0) { $dirty = $true }
# Also counts commits made after the last version tag (for example by the publicar.ps1).
$lastTag = git describe --tags --abbrev=0 2>$null
$ahead = if ($lastTag) { [int](git rev-list "$lastTag..HEAD" --count) } else { 1 }
if (-not $dirty -and -not (git status --porcelain) -and $ahead -eq 0) { Done "$lastTag já foi enviada; o botão ATUALIZAR aparece sozinho quando o GitHub terminar (~15 min)." 0 }

$conf = 'src-tauri\tauri.conf.json'
$text = Get-Content $conf -Raw
if ($text -notmatch '"version":\s*"(\d+)\.(\d+)\.(\d+)"') { Done "Não achei a versão em $conf" 1 }
$next = "$($Matches[1]).$($Matches[2]).$([int]$Matches[3] + 1)"
$text = [regex]::Replace($text, '"version":\s*"\d+\.\d+\.\d+"', "`"version`": `"$next`"", 1)
[IO.File]::WriteAllText((Resolve-Path $conf), $text, (New-Object Text.UTF8Encoding $false))
$pkg = Get-Content 'package.json' -Raw
$pkg = [regex]::Replace($pkg, '"version":\s*"\d+\.\d+\.\d+"', "`"version`": `"$next`"", 1)
[IO.File]::WriteAllText((Resolve-Path 'package.json'), $pkg, (New-Object Text.UTF8Encoding $false))

git add -A
git commit -m "v$next" | Out-Null
if ($LASTEXITCODE -ne 0) { Done 'Commit falhou.' 1 }
git push origin HEAD
if ($LASTEXITCODE -ne 0) { Done 'Push falhou (sem internet ou sem login no gh?).' 1 }
git tag "v$next"
git push origin "v$next"
if ($LASTEXITCODE -ne 0) { Done 'Push da tag falhou.' 1 }
Done "v$next enviada; fica pronta em ~10 min." 0
