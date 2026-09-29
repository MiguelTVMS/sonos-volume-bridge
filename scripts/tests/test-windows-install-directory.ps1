param([string]$MakeNsis = "$env:LOCALAPPDATA\tauri\NSIS\makensis.exe")
$ErrorActionPreference = 'Stop'
$root = Split-Path (Split-Path $PSScriptRoot)
$template = Get-Content (Join-Path $root 'src-tauri/windows/installer.nsi') -Raw
$restore = [regex]::Match($template, '(?s)Function RestorePreviousInstallLocation\r?\n.*?FunctionEnd').Value
$move = [regex]::Match($template, '(?s)Function MigratePreviousInstallDirectory\r?\n.*?FunctionEnd').Value
if (-not $restore -or -not $move) { throw 'Production migration functions missing.' }
# Assert actual production wiring, then execute the same functions on isolated fixtures.
$install = [regex]::Match($template, '(?s)Section Install\r?\n.*?SectionEnd').Value
if ($install.IndexOf('Call MigratePreviousInstallDirectory') -lt 0 -or
    $install.IndexOf('Call MigratePreviousInstallDirectory') -gt $install.IndexOf('SetOutPath')) {
    throw 'Migration must run before writing the installation.'
}
$directory = Join-Path $root 'target/directory-regression'
New-Item -ItemType Directory -Force $directory | Out-Null
$fixture = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
$key = "Software\SpeakerVolumeBridgeDirectoryTest-$PID"
try {
    foreach ($case in @('clean', 'legacy', 'renamed', 'custom', 'collision', 'locked', 'machine')) {
        $base = Join-Path $fixture $case
        $old = Join-Path $base 'Sonos Volume Bridge'
        $new = Join-Path $base 'Speaker Volume Bridge'
        $previous = if ($case -eq 'clean') { '' } elseif ($case -eq 'renamed') { $new } elseif ($case -eq 'custom') { Join-Path $base 'Custom' } else { $old }
        $expected = if ($case -eq 'custom') { $previous } else { $new }
        New-Item -ItemType Directory -Force $base | Out-Null
        if ($previous) {
            New-Item -ItemType Directory -Force $previous | Out-Null
            Set-Content (Join-Path $previous 'preserved.txt') 'preserve me'
        }
        if ($case -eq 'collision') {
            New-Item -ItemType Directory -Force $new | Out-Null
            Set-Content (Join-Path $new 'unrelated.txt') 'do not overwrite'
        }
        $functions = "$restore`n$move"
        # Map OS folders to private fixtures, leaving production branching intact.
        $local = if ($case -eq 'machine') { "$base\unused" } else { $base }
        $functions = $functions.Replace('$LOCALAPPDATA', $local).Replace('$PROGRAMFILES64', $base).Replace('$PROGRAMFILES', $base)
        $output = Join-Path $directory "$case.exe"
        $source = @'
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!define LEGACYPRODUCTNAME "Sonos Volume Bridge"
!define PRODUCTNAME "Speaker Volume Bridge"
!define MANUPRODUCTKEY "@KEY@"
Name "Directory regression"
OutFile "@OUTPUT@"
RequestExecutionLevel user
SilentInstall silent
Var PreviousInstallDir
Var RelocateInstallDir
@FUNCTIONS@
Section
SetShellVarContext current
WriteRegStr HKCU "${MANUPRODUCTKEY}" "" "@PREVIOUS@"
StrCpy $INSTDIR "@NEW@"
Call RestorePreviousInstallLocation
${If} $INSTDIR != "@EXPECTED@"
  SetErrorLevel 11
  Quit
${EndIf}
Call MigratePreviousInstallDirectory
FileOpen $0 "@BASE@\completed.txt" w
FileWrite $0 "done"
FileClose $0
SetErrorLevel 0
SectionEnd
'@
        $source = $source.Replace('@KEY@', $key).Replace('@OUTPUT@', $output).Replace('@FUNCTIONS@', $functions).Replace('@PREVIOUS@', $previous).Replace('@NEW@', $new).Replace('@EXPECTED@', $expected).Replace('@BASE@', $base)
        $file = Join-Path $directory "$case.nsi"
        Set-Content -LiteralPath $file -Value $source
        & $MakeNsis /V2 $file
        if ($LASTEXITCODE -ne 0) { throw 'Directory regression harness failed to compile.' }
        $lock = $null
        try {
            if ($case -eq 'locked') {
                $lock = [IO.File]::Open((Join-Path $old 'preserved.txt'), 'Open', 'ReadWrite', 'None')
            }
            $process = Start-Process -FilePath $output -WindowStyle Hidden -Wait -PassThru
        } finally {
            if ($lock) { $lock.Dispose() }
        }
        $completed = Test-Path (Join-Path $base 'completed.txt')
        if ($case -in @('collision', 'locked')) {
            if ($completed -or -not (Test-Path (Join-Path $old 'preserved.txt'))) {
                throw "Unsafe move failure: $case"
            }
            if ($case -eq 'collision' -and (Get-Content (Join-Path $new 'unrelated.txt')) -ne 'do not overwrite') {
                throw 'Destination was modified.'
            }
        } else {
            if ($process.ExitCode -ne 0 -or -not $completed) { throw "Migration failed: $case" }
            if ($previous -and (Get-Content (Join-Path $expected 'preserved.txt')) -ne 'preserve me') {
                throw "Existing files were not preserved: $case"
            }
            if ($case -in @('legacy', 'machine') -and (Test-Path $old)) { throw 'Legacy directory remains.' }
        }
    }
} finally {
    Remove-Item "HKCU:\$key" -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item $fixture -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'Production NSIS directory selection and migration checks passed.'
