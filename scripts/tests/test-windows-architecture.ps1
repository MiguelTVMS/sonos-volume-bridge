$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '../windows-architecture.ps1')
$fixture = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
try {
    foreach ($case in @(@('x64', 0x8664), @('arm64', 0xAA64))) {
        $bytes = [byte[]]::new(134)
        [BitConverter]::GetBytes([uint16]0x5A4D).CopyTo($bytes, 0)
        [BitConverter]::GetBytes([uint32]128).CopyTo($bytes, 60)
        [BitConverter]::GetBytes([uint32]0x4550).CopyTo($bytes, 128)
        [BitConverter]::GetBytes([uint16]$case[1]).CopyTo($bytes, 132)
        [System.IO.File]::WriteAllBytes($fixture, $bytes)
        Assert-WindowsExecutableArchitecture -Path $fixture -Architecture $case[0]
        $wrong = if ($case[0] -eq 'x64') { 'arm64' } else { 'x64' }
        $rejected = $false
        try { Assert-WindowsExecutableArchitecture -Path $fixture -Architecture $wrong } catch { $rejected = $true }
        if (-not $rejected) { throw 'Architecture mismatch was accepted.' }
        # Exercise the production packaging entry point: reject before SDK/Cargo work.
        $rejected = $false
        try { & (Join-Path $PSScriptRoot '../build-msix.ps1') -ExecutablePath $fixture -Architecture $wrong } catch {
            if ($_.Exception.Message -notlike 'Expected * executable, found *') { throw }
            $rejected = $true
        }
        if (-not $rejected) { throw 'MSIX packaging accepted the wrong architecture.' }
    }
    foreach ($invalid in @([byte[]]::new(0), [byte[]]::new(134))) {
        [System.IO.File]::WriteAllBytes($fixture, $invalid)
        $rejected = $false
        try { Get-WindowsExecutableArchitecture -Path $fixture } catch { $rejected = $true }
        if (-not $rejected) { throw 'Invalid executable was accepted.' }
    }
    Write-Output 'Windows architecture checks passed.'
} finally {
    Remove-Item -LiteralPath $fixture -ErrorAction SilentlyContinue
}
