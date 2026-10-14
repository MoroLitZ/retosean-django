param(
    [switch]$Initialize,
    [switch]$Release
)
$ErrorActionPreference = 'Stop'
$taskCargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path -LiteralPath $taskCargo)) {
    $taskCargo = (Get-Command cargo -ErrorAction Stop).Source
}
Push-Location $PSScriptRoot
try {
    $taskBuildFlags = @('--locked')
    if ($Release) { $taskBuildFlags += '--release' }
    if ($Initialize) {
        & $taskCargo run @taskBuildFlags -- init-db
        if ($LASTEXITCODE -ne 0) { throw 'Database initialization failed. Existing databases are never reset automatically.' }
        & $taskCargo run @taskBuildFlags -- seed-demo
        if ($LASTEXITCODE -ne 0) { throw 'Demo initialization failed.' }
    }
    Write-Host 'RetosEAN Rust: http://127.0.0.1:8080 (Ctrl+C to stop)'
    & $taskCargo run @taskBuildFlags -- serve
    if ($LASTEXITCODE -ne 0) { throw 'RetosEAN stopped with an error.' }
} finally { Pop-Location }
