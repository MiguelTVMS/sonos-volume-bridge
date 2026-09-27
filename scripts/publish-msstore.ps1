[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$InputDirectory,
    [string]$AppId = $env:MICROSOFT_STORE_PRODUCT_ID,
    [switch]$ValidateOnly
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$uploads = @(Get-ChildItem -LiteralPath $InputDirectory -File)
if ($uploads.Count -ne 1 -or $uploads[0].Extension -ne '.msixupload') {
    throw 'The input directory must contain exactly one verified .msixupload file.'
}
if ($ValidateOnly) { $AppId = 'validation-only' }
if ([string]::IsNullOrWhiteSpace($AppId)) { throw 'Microsoft Store product configuration is missing.' }
$arguments = @('publish', '.', '--inputDirectory', (Resolve-Path -LiteralPath $InputDirectory).Path,
               '--appId', $AppId, '--uploadTimeout', '300')
# v0.4.3 checks local configuration before invoking even --help. Supply a
# non-authenticating placeholder only for help, then restore the exact original.
if ($ValidateOnly) {
    $settings = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Microsoft/MSStore.CLI/settings.json'
    $hadSettings = Test-Path -LiteralPath $settings
    $original = if ($hadSettings) { [System.IO.File]::ReadAllBytes($settings) } else { $null }
    try {
        New-Item -ItemType Directory -Path (Split-Path -Parent $settings) -Force | Out-Null
        @{
            SellerId = 0
            TenantId = [guid]::Empty.ToString()
            ClientId = [guid]::Empty.ToString()
            CertificateFilePath = Join-Path ([System.IO.Path]::GetTempPath()) "$([guid]::NewGuid()).missing"
        } | ConvertTo-Json | Set-Content -LiteralPath $settings -Encoding utf8NoBOM
        & msstore @arguments --help
        if ($LASTEXITCODE -ne 0) { throw 'Microsoft Store command validation failed.' }
    } finally {
        if ($hadSettings) { [System.IO.File]::WriteAllBytes($settings, $original) }
        else { Remove-Item -LiteralPath $settings -ErrorAction SilentlyContinue }
    }
} else {
    & msstore @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Microsoft Store command failed.' }
}
