[CmdletBinding()]
param(
    [string]$ExecutablePath,
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$targetRoot = Join-Path $repositoryRoot 'target'
$packageSource = Join-Path $repositoryRoot 'packaging\windows-msix'

if ([string]::IsNullOrWhiteSpace($ExecutablePath)) {
    $ExecutablePath = Join-Path $targetRoot 'release\sonos-volume-bridge.exe'
}
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $targetRoot 'release\bundle\msix'
}

$ExecutablePath = [System.IO.Path]::GetFullPath($ExecutablePath)
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
if (-not (Test-Path -LiteralPath $ExecutablePath -PathType Leaf)) {
    throw "The release executable was not found at $ExecutablePath. Run 'cargo tauri build --no-bundle' first."
}

$cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
if ($null -eq $cargo) {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
}
if ($null -eq $cargo) {
    throw 'Cargo was not found on PATH.'
}

$metadataText = & $cargo.Source metadata --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) {
    throw 'Cargo metadata failed.'
}
$metadata = $metadataText | ConvertFrom-Json
$applicationPackage = $metadata.packages | Where-Object { $_.name -eq 'sonos-volume-bridge' }
if ($null -eq $applicationPackage) {
    throw 'The sonos-volume-bridge package was not found in cargo metadata.'
}

$semanticVersion = [System.Management.Automation.SemanticVersion]::Parse($applicationPackage.version)
$numericVersionParts = @($semanticVersion.Major, $semanticVersion.Minor, $semanticVersion.Patch, 0)
if ($numericVersionParts | Where-Object { $_ -lt 0 -or $_ -gt 65535 }) {
    throw "The workspace version $($applicationPackage.version) cannot be represented as an MSIX version."
}
$msixVersion = $numericVersionParts -join '.'

$makeAppxCommand = Get-Command MakeAppx.exe -ErrorAction SilentlyContinue
if ($null -ne $makeAppxCommand) {
    $makeAppx = $makeAppxCommand.Source
} else {
    $windowsKitsRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $makeAppx = Get-ChildItem -LiteralPath $windowsKitsRoot -Filter MakeAppx.exe -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\MakeAppx\.exe$' } |
        Sort-Object FullName -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}
if ([string]::IsNullOrWhiteSpace($makeAppx)) {
    throw 'MakeAppx.exe was not found. Install the Windows SDK.'
}

$stagingDirectory = Join-Path $targetRoot 'msix\staging-x64'
$verificationDirectory = Join-Path $targetRoot 'msix\verified-x64'
foreach ($directory in @($stagingDirectory, $verificationDirectory)) {
    $resolvedDirectory = [System.IO.Path]::GetFullPath($directory)
    if (-not $resolvedDirectory.StartsWith([System.IO.Path]::GetFullPath($targetRoot), [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to clear a directory outside the target tree: $resolvedDirectory"
    }
    if (Test-Path -LiteralPath $resolvedDirectory) {
        Remove-Item -LiteralPath $resolvedDirectory -Recurse -Force
    }
    New-Item -ItemType Directory -Path $resolvedDirectory | Out-Null
}

$assetsDirectory = Join-Path $stagingDirectory 'Assets'
New-Item -ItemType Directory -Path $assetsDirectory | Out-Null
Copy-Item -LiteralPath $ExecutablePath -Destination (Join-Path $stagingDirectory 'sonos-volume-bridge.exe')
Copy-Item -LiteralPath (Join-Path $packageSource 'Assets\StoreLogo.png') -Destination $assetsDirectory
Copy-Item -LiteralPath (Join-Path $packageSource 'Assets\Square44x44Logo.png') -Destination $assetsDirectory
Copy-Item -LiteralPath (Join-Path $packageSource 'Assets\Square150x150Logo.png') -Destination $assetsDirectory

$manifestTemplate = Get-Content -LiteralPath (Join-Path $packageSource 'AppxManifest.xml.template') -Raw
$manifest = $manifestTemplate.Replace('{{VERSION}}', $msixVersion)
$manifestPath = Join-Path $stagingDirectory 'AppxManifest.xml'
Set-Content -LiteralPath $manifestPath -Value $manifest -Encoding utf8NoBOM

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$packagePath = Join-Path $OutputDirectory "SonosVolumeBridge_${msixVersion}_x64.msix"
if (Test-Path -LiteralPath $packagePath) {
    Remove-Item -LiteralPath $packagePath -Force
}

& $makeAppx pack /d $stagingDirectory /p $packagePath /o
if ($LASTEXITCODE -ne 0) {
    throw 'MakeAppx failed to create the package.'
}

& $makeAppx unpack /p $packagePath /d $verificationDirectory /o
if ($LASTEXITCODE -ne 0) {
    throw 'MakeAppx failed to verify the generated package.'
}

$verifiedManifest = [xml](Get-Content -LiteralPath (Join-Path $verificationDirectory 'AppxManifest.xml') -Raw)
$namespace = New-Object System.Xml.XmlNamespaceManager($verifiedManifest.NameTable)
$namespace.AddNamespace('f', 'http://schemas.microsoft.com/appx/manifest/foundation/windows10')
$identity = $verifiedManifest.SelectSingleNode('/f:Package/f:Identity', $namespace)
$language = $verifiedManifest.SelectSingleNode('/f:Package/f:Resources/f:Resource', $namespace)
if ($identity.Name -ne 'Miguel.MS.SonosVolumeBridge' -or
    $identity.Publisher -ne 'CN=7D58CCC9-6311-4A59-95A9-FF7375C0ECDC' -or
    $identity.Version -ne $msixVersion -or
    $identity.ProcessorArchitecture -ne 'x64' -or
    $language.Language -ne 'en-US') {
    throw 'The generated package identity, version, architecture, or language is invalid.'
}
if (-not (Test-Path -LiteralPath (Join-Path $verificationDirectory 'sonos-volume-bridge.exe') -PathType Leaf)) {
    throw 'The generated package does not contain the application executable.'
}

Write-Output "Created Microsoft Store package: $packagePath"
Write-Output "MSIX version: $msixVersion"
Write-Output 'The package is unsigned for Partner Center submission. Sign it with a trusted development certificate before local installation.'
