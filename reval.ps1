<#
.SYNOPSIS
    Convenience wrapper to run or build TideOS Eval Kit: reval.
.DESCRIPTION
    Runs `Tools/reval/target/release/reval.exe`, building it first if missing.
.EXAMPLE
    .\reval.ps1 trace analyze
    .\reval.ps1 hygiene .
    .\reval.ps1 gate --corpus fixtures/traces.jsonl --baseline fixtures/baseline.json
#>
[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ArgsList
)

$ErrorActionPreference = 'Stop'
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$ExePath = Join-Path $ScriptDir "target\release\reval.exe"

if (-not (Test-Path $ExePath)) {
    Write-Host "reval.exe not found. Building release binary..." -ForegroundColor Cyan
    & cargo build --manifest-path (Join-Path $ScriptDir "Cargo.toml") --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Cargo build failed with exit code $LASTEXITCODE"
        exit $LASTEXITCODE
    }
}

& $ExePath @ArgsList
exit $LASTEXITCODE
