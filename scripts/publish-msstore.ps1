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
# Parse the actual production arguments without authenticating or changing Store state.
if ($ValidateOnly) { $arguments += '--help' }
& msstore @arguments
if ($LASTEXITCODE -ne 0) { throw 'Microsoft Store command failed.' }
