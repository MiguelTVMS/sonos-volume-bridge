function Get-WindowsExecutableArchitecture {
    param([Parameter(Mandatory)][string]$Path)
    $stream = [System.IO.File]::OpenRead((Resolve-Path -LiteralPath $Path))
    $reader = [System.IO.BinaryReader]::new($stream)
    try {
        if ($reader.ReadUInt16() -ne 0x5A4D) { throw 'Invalid DOS executable header.' }
        $stream.Position = 0x3C
        $offset = $reader.ReadUInt32()
        if ($offset -lt 0x40 -or $offset + 6 -gt $stream.Length) { throw 'Invalid PE header offset.' }
        $stream.Position = $offset
        if ($reader.ReadUInt32() -ne 0x00004550) { throw 'Invalid PE signature.' }
        switch ($reader.ReadUInt16()) {
            0x8664 { return 'x64' }
            0xAA64 { return 'arm64' }
            default { throw 'Only Windows x64 and ARM64 executables are supported.' }
        }
    } finally {
        $reader.Dispose()
    }
}

function Assert-WindowsExecutableArchitecture {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][ValidateSet('x64', 'arm64')][string]$Architecture
    )
    $actual = Get-WindowsExecutableArchitecture -Path $Path
    if ($actual -ne $Architecture) {
        throw "Expected $Architecture executable, found $actual."
    }
}
