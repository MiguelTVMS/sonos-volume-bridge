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
        if ($archive.Entries.Count -ne 1 -or $archive.Entries[0].Name -ne 'SonosVolumeBridge.msixbundle') {
            throw 'Upload must contain exactly the verified bundle.'
        }
    } finally { $archive.Dispose() }

    & (Join-Path $PSScriptRoot '../publish-msstore.ps1') -InputDirectory $temporary -ValidateOnly

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
