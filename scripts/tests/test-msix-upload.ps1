[CmdletBinding()]
param([Parameter(Mandatory)][string]$PackageDirectory)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$builder = Join-Path $PSScriptRoot '../build-msix-upload.ps1'
$temporary = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    $output = Join-Path $temporary 'valid.msixupload'
    & $builder -PackageDirectory $PackageDirectory -OutputPath $output
    $archive = [System.IO.Compression.ZipFile]::OpenRead($output)
    try {
        if ($archive.Entries.Count -ne 1 -or $archive.Entries[0].Name -ne 'SpeakerVolumeBridge.msixbundle') {
            throw 'Upload must contain exactly the verified bundle.'
        }
    } finally { $archive.Dispose() }

    $settings = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Microsoft/MSStore.CLI/settings.json'
    $hadSettings = Test-Path -LiteralPath $settings
    $originalSettings = if ($hadSettings) { [System.IO.File]::ReadAllBytes($settings) } else { $null }
    & (Join-Path $PSScriptRoot '../publish-msstore.ps1') -InputDirectory $temporary -ValidateOnly
    if ($hadSettings) {
        if ([Convert]::ToBase64String([System.IO.File]::ReadAllBytes($settings)) -ne [Convert]::ToBase64String($originalSettings)) {
            throw 'CLI validation did not restore the original local configuration.'
        }
    } elseif (Test-Path -LiteralPath $settings) {
        throw 'CLI validation left a placeholder configuration behind.'
    }

    # Execute the production wrapper with an intercepted CLI. The old positional
    # '.' selects Electron and must fail even though --help accepts it.
    function global:msstore {
        if ($args[0] -ne 'publish' -or $args[1] -ne $output -or
            -not (Test-Path -LiteralPath $args[1] -PathType Leaf) -or
            [System.IO.Path]::GetExtension($args[1]) -ne '.msixupload') {
            throw 'Publishing must select the actual MSIX upload file, never the project directory.'
        }
        if ($args[-1] -ne '--help') { throw 'Dry run must not publish.' }
        $global:LASTEXITCODE = 0
    }
    try {
        & (Join-Path $PSScriptRoot '../publish-msstore.ps1') -InputDirectory $temporary -ValidateOnly
    } finally { Remove-Item Function:\msstore }

    foreach ($case in @('missing', 'wrong-architecture-name', 'mismatched-version')) {
        $inputs = Join-Path $temporary $case
        New-Item -ItemType Directory -Path $inputs | Out-Null
        Get-ChildItem -LiteralPath $PackageDirectory -Filter '*.msix' | Copy-Item -Destination $inputs
        $arm = Get-ChildItem -LiteralPath $inputs -Filter '*_arm64.msix' | Select-Object -First 1
        switch ($case) {
            'missing' { Remove-Item -LiteralPath $arm.FullName }
            'wrong-architecture-name' { Rename-Item -LiteralPath $arm.FullName -NewName 'wrong_x64.msix' }
            'mismatched-version' {
                $zip = [System.IO.Compression.ZipFile]::Open($arm.FullName, 'Update')
                try {
                    $entry = $zip.GetEntry('AppxManifest.xml')
                    $reader = [System.IO.StreamReader]::new($entry.Open())
                    try { $manifest = [xml]$reader.ReadToEnd() } finally { $reader.Dispose() }
                    $manifest.Package.Identity.Version = '0.0.0.0'
                    $entry.Delete()
                    $writer = [System.IO.StreamWriter]::new($zip.CreateEntry('AppxManifest.xml').Open())
                    try { $writer.Write($manifest.OuterXml) } finally { $writer.Dispose() }
                } finally { $zip.Dispose() }
            }
        }
        $rejected = Join-Path $temporary "$case.msixupload"
        $failed = $false
        try { & $builder -PackageDirectory $inputs -OutputPath $rejected } catch { $failed = $true }
        if (-not $failed -or (Test-Path -LiteralPath $rejected)) {
            throw "Invalid package set was not rejected: $case"
        }
    }
} finally { Remove-Item -LiteralPath $temporary -Recurse -Force }
Write-Output 'Combined upload and invalid-package regressions passed.'
