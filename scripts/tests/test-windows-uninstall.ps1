param([string]$MakeNsis = "$env:LOCALAPPDATA\tauri\NSIS\makensis.exe")
$ErrorActionPreference = 'Stop'
$root = Split-Path (Split-Path $PSScriptRoot)
$template = Get-Content (Join-Path $root 'src-tauri/windows/installer.nsi') -Raw
# Execute the production uninstall guard and registry commands against isolated test keys.
$section = ($template -split 'Section Uninstall', 2)[1]
$cleanup = [regex]::Match($section, '(?s)\$\{If\} \$UpdateMode <> 1.*?\$\{EndIf\}').Value
$cleanup = $cleanup.Replace('!insertmacro DeleteAppUserModelId', '')
# The first nested guard follows the cleanup; omit shortcut handling from this harness.
$cleanup = ($cleanup -split '; Remove start menu shortcut', 2)[0] + '${EndIf}'
$testKey = "Software\SpeakerVolumeBridgeUninstallTest-$PID"
$cleanup = $cleanup.Replace('Software\Classes', $testKey)
$directory = Join-Path $root 'target/uninstall-regression'
New-Item -ItemType Directory -Force $directory | Out-Null
try {
    foreach ($mode in @(0, 1)) {
        $output = Join-Path $directory "cleanup-$mode.exe"
        $source = @'
!include "LogicLib.nsh"
!define BUNDLEID "test.app"
Name "Uninstall regression"
OutFile "@OUTPUT@"
RequestExecutionLevel user
SilentInstall silent
Var UpdateMode
Section
StrCpy $UpdateMode @MODE@
WriteRegStr HKCU "@KEY@\AppUserModelId\test.app" "DisplayName" "test"
WriteRegStr HKCU "@KEY@\CLSID\{a607018c-48b4-45c8-b0b2-46c243fde206}" "" "test"
@CLEANUP@
SetErrorLevel 0
ClearErrors
ReadRegStr $0 HKCU "@KEY@\AppUserModelId\test.app" "DisplayName"
${If} $UpdateMode = 0
  ${IfNot} ${Errors}
    SetErrorLevel 1
  ${EndIf}
${Else}
  ${If} ${Errors}
    SetErrorLevel 2
  ${EndIf}
${EndIf}
ClearErrors
ReadRegStr $0 HKCU "@KEY@\CLSID\{a607018c-48b4-45c8-b0b2-46c243fde206}" ""
${If} $UpdateMode = 0
  ${IfNot} ${Errors}
    SetErrorLevel 3
  ${EndIf}
${Else}
  ${If} ${Errors}
    SetErrorLevel 4
  ${EndIf}
${EndIf}
SectionEnd
'@
        $source = $source.Replace('@OUTPUT@', $output).Replace('@MODE@', "$mode").Replace('@KEY@', $testKey).Replace('@CLEANUP@', $cleanup)
        $file = Join-Path $directory "cleanup-$mode.nsi"
        Set-Content -LiteralPath $file -Value $source
        & $MakeNsis /V2 $file
        if ($LASTEXITCODE -ne 0) { throw 'Uninstall regression harness failed to compile.' }
        $process = Start-Process -FilePath $output -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw "Uninstall regression failed for update mode $mode (exit $($process.ExitCode))." }
    }
} finally {
    $key = "HKCU:\$testKey"
    if (Test-Path -LiteralPath $key) { Remove-Item -LiteralPath $key -Recurse -Force }
}
Write-Output 'Full uninstall removes both registrations; updates preserve both.'
