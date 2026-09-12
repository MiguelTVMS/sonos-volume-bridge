# Entry point for Codex local environments on Windows (PowerShell 5.1+).
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('setup', 'run', 'check', 'build')]
    [string]$Action
)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '../..')

# Windows PowerShell does not throw when a native executable fails.
function Invoke-Checked {
    param([string]$Program, [string[]]$Arguments)
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Program failed with exit code $LASTEXITCODE"
    }
}

switch ($Action) {
    'setup' {
        Get-Command rustup, cargo, node, pnpm.cmd -ErrorAction Stop | Out-Null
        $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
        if (-not (Test-Path $vswhere)) {
            throw 'Install Visual Studio Build Tools with Desktop development with C++ and a Windows SDK.'
        }
        $installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if (-not $installation) {
            throw 'Install the Visual Studio Desktop development with C++ workload and a Windows SDK.'
        }
        Invoke-Checked rustup @('show', 'active-toolchain')
        Invoke-Checked cargo @('tauri', '--version')
        Invoke-Checked cargo @('fetch', '--locked')
        Invoke-Checked pnpm.cmd @('--dir', 'ui', 'install', '--frozen-lockfile')
        Invoke-Checked pnpm.cmd @('--dir', 'ui', 'run', 'build')
    }
    'run' { Invoke-Checked cargo @('tauri', 'dev') }
    'check' {
        Invoke-Checked cargo @('fmt', '--check')
        Invoke-Checked cargo @('clippy', '--workspace', '--all-targets', '--', '-D', 'warnings')
        Invoke-Checked cargo @('test', '--workspace')
        Invoke-Checked pnpm.cmd @('--dir', 'ui', 'run', 'format')
        Invoke-Checked pnpm.cmd @('--dir', 'ui', 'run', 'lint')
        Invoke-Checked pnpm.cmd @('--dir', 'ui', 'test')
        Invoke-Checked pnpm.cmd @('--dir', 'ui', 'run', 'build')
    }
    'build' { Invoke-Checked cargo @('tauri', 'build', '--no-bundle') }
}
