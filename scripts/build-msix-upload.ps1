[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PackageDirectory,
    [Parameter(Mandatory)][string]$OutputPath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.IO.Compression.FileSystem

$packages = @(Get-ChildItem -LiteralPath $PackageDirectory -Filter '*.msix')
if ($packages.Count -ne 2) { throw 'Expected exactly one x64 and one ARM64 MSIX.' }
$architectures = @()
$commonIdentity = $null
$version = $null
foreach ($package in $packages) {
    $archive = [System.IO.Compression.ZipFile]::OpenRead($package.FullName)
    try {
        $entry = $archive.GetEntry('AppxManifest.xml')
        if ($null -eq $entry) { throw 'MSIX manifest is missing.' }
        $reader = [System.IO.StreamReader]::new($entry.Open())
        try { $manifest = [xml]$reader.ReadToEnd() } finally { $reader.Dispose() }
        $identity = $manifest.Package.Identity
        $architecture = [string]$identity.ProcessorArchitecture
        if ($architecture -notin @('x64', 'arm64') -or $architecture -in $architectures) {
            throw 'Expected distinct x64 and ARM64 package identities.'
        }
        if (-not $package.Name.EndsWith("_${architecture}.msix")) {
            throw 'MSIX filename and manifest architecture disagree.'
        }
        $key = @($identity.Name, $identity.Publisher, $identity.Version) -join '|'
        if ($null -ne $commonIdentity -and $key -ne $commonIdentity) {
            throw 'MSIX packages must share a name, publisher and version.'
        }
        $commonIdentity = $key
        $version = [string]$identity.Version
        $architectures += $architecture
    } finally { $archive.Dispose() }
}

$makeAppxCommand = Get-Command MakeAppx.exe -ErrorAction SilentlyContinue
if ($null -ne $makeAppxCommand) {
    $makeAppx = $makeAppxCommand.Source
} else {
    $sdkRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/bin'
    $makeAppx = Get-ChildItem -LiteralPath $sdkRoot -Filter MakeAppx.exe -Recurse |
        Where-Object FullName -Match '\\x64\\MakeAppx\.exe$' |
        Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if ([string]::IsNullOrWhiteSpace($makeAppx)) { throw 'MakeAppx was not found.' }

$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
if ([System.IO.Path]::GetExtension($OutputPath) -ne '.msixupload') {
    throw 'OutputPath must end in .msixupload.'
}
$temporary = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    $inputs = Join-Path $temporary 'packages'
    $upload = Join-Path $temporary 'upload'
    New-Item -ItemType Directory -Path $inputs, $upload | Out-Null
    foreach ($package in $packages) { Copy-Item -LiteralPath $package.FullName -Destination $inputs }
    $bundle = Join-Path $upload 'SonosVolumeBridge.msixbundle'
    & $makeAppx bundle /d $inputs /p $bundle /bv $version /o
    if ($LASTEXITCODE -ne 0) { throw 'MSIX bundle creation failed.' }

    $verified = Join-Path $temporary 'verified'
    & $makeAppx unbundle /p $bundle /d $verified /o
    if ($LASTEXITCODE -ne 0) { throw 'MSIX bundle verification failed.' }
    foreach ($package in $packages) {
        $unpacked = Join-Path $verified $package.Name
        if (-not (Test-Path -LiteralPath $unpacked) -or
            (Get-FileHash -LiteralPath $unpacked).Hash -ne (Get-FileHash -LiteralPath $package.FullName).Hash) {
            throw 'Bundle did not preserve both architecture packages.'
        }
    }
    $archivePath = Join-Path $temporary 'SonosVolumeBridge.msixupload'
    [System.IO.Compression.ZipFile]::CreateFromDirectory($upload, $archivePath)
    New-Item -ItemType Directory -Path (Split-Path -Parent $OutputPath) -Force | Out-Null
    Move-Item -LiteralPath $archivePath -Destination $OutputPath -Force
} finally {
    Remove-Item -LiteralPath $temporary -Recurse -Force
}
Write-Output 'Created and verified the combined x64/ARM64 Store upload.'
