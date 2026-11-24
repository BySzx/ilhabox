# Primeira publicação da IlhaBox. A Ilha de casa roda isto sozinha (com -SemPausa) quando
# encontra Documents\IlhaBox sem git; dá pra rodar à mão também.
# - código no repositório PRIVADO BySzx/ilhabox
# - instaladores no repositório PÚBLICO BySzx/ilhabox-releases (só o .exe e o latest.json)
param([switch]$SemPausa)
$ErrorActionPreference = 'Continue'
Set-Location $PSScriptRoot
# O workflow vem na raiz (as ferramentas remotas não escrevem em .github) e vai pro lugar aqui.
if (Test-Path 'release.yml') {
  New-Item -ItemType Directory -Force '.github\workflows' | Out-Null
  Move-Item -Force 'release.yml' '.github\workflows\release.yml'
}
$code = 'BySzx/ilhabox'
$pub = 'BySzx/ilhabox-releases'
$key = Join-Path (Split-Path $PSScriptRoot -Parent) 'ilha-updater.key'

function Fail($msg) { Write-Host "`n✖ $msg" -ForegroundColor Red; if (-not $SemPausa) { Read-Host 'Enter pra fechar' }; Write-Output $msg; exit 1 }
function Step($msg) { Write-Host "`n▸ $msg" -ForegroundColor Cyan }

foreach ($tool in @(@{ cmd = 'git'; id = 'Git.Git' }, @{ cmd = 'gh'; id = 'GitHub.cli' })) {
  if (-not (Get-Command $tool.cmd -ErrorAction SilentlyContinue)) {
    Step "Instalando $($tool.cmd)…"
    winget install --id $tool.id -e --accept-source-agreements --accept-package-agreements
    Fail "Instalei o $($tool.cmd). Feche este terminal, abra de novo e rode o script outra vez."
  }
}
if (-not (Test-Path $key)) { Fail "Não achei a chave em $key" }

gh auth status *> $null
if ($LASTEXITCODE -ne 0) {
  if ($SemPausa) { Fail 'Sem login no GitHub (gh).' }
  Step 'Login no GitHub (abre o navegador)'
  gh auth login --web --git-protocol https --scopes workflow
  if ($LASTEXITCODE -ne 0) { Fail 'Login no GitHub falhou.' }
}
if (-not ((gh auth status 2>&1 | Out-String) -match "'workflow'")) {
  if ($SemPausa) { Fail 'O login do gh não tem a permissão workflow.' }
  Step 'Liberando permissão pro build automático (abre o navegador)'
  gh auth refresh -h github.com -s workflow
  if ($LASTEXITCODE -ne 0) { Fail 'Não deu pra liberar a permissão workflow.' }
}
gh auth setup-git *> $null

Step 'Preparando o git'
if (-not (Test-Path '.git')) { git init -b main | Out-Null }
if (-not (git config user.name)) { git config user.name 'BySzx'; git config user.email 'BySzx@users.noreply.github.com' }
git add -A
git diff --cached --quiet
if ($LASTEXITCODE -ne 0) { git commit -m 'IlhaBox' | Out-Null }

Step "Subindo o código pro github.com/$code (privado)"
gh repo view $code *> $null
if ($LASTEXITCODE -ne 0) {
  gh repo create $code --private --source . --remote origin --push --description 'IlhaBox: a Ilha do trabalho, num celular pelo USB'
} else {
  if (-not (git remote)) { git remote add origin "https://github.com/$code.git" }
  # The repo may already have history (first install made from elsewhere): put this folder on top of it.
  git fetch origin main 2>$null
  if ($LASTEXITCODE -eq 0) {
    git reset --soft origin/main
    # Workflow files only exist in the repo (they can't be copied to this folder): keep them.
    git checkout origin/main -- .github 2>$null
    git commit -m 'IlhaBox' 2>$null | Out-Null
  }
  git push -u origin main
}
if ($LASTEXITCODE -ne 0) { Fail 'Push do código falhou.' }

Step "Criando github.com/$pub (público, só instaladores)"
gh repo view $pub *> $null
if ($LASTEXITCODE -ne 0) {
  gh repo create $pub --public --add-readme --description 'Instaladores e atualizações da IlhaBox'
  if ($LASTEXITCODE -ne 0) { Fail 'Não deu pra criar o repositório de instaladores.' }
}

Step 'Guardando a chave de atualização e o acesso aos instaladores como secrets'
Get-Content $key -Raw | gh secret set TAURI_SIGNING_PRIVATE_KEY -R $code
if ($LASTEXITCODE -ne 0) { Fail 'Não deu pra salvar a chave.' }
gh auth token | gh secret set RELEASES_TOKEN -R $code
if ($LASTEXITCODE -ne 0) { Fail 'Não deu pra salvar o acesso aos instaladores.' }

$version = (Get-Content 'src-tauri\tauri.conf.json' -Raw | ConvertFrom-Json).version
Step "Criando a tag v$version (dispara o build)"
git tag "v$version" 2>$null
git push origin "v$version"
if ($LASTEXITCODE -ne 0) { Fail 'Push da tag falhou.' }

Write-Host "`n✔ Pronto! A versão v$version fica pronta em ~15 min em https://github.com/$pub/releases" -ForegroundColor Green
if (-not $SemPausa) { gh browse -R $code --actions 2>$null; Read-Host 'Enter pra fechar' }
Write-Output "IlhaBox v$version enviada; fica pronta em ~15 min."
