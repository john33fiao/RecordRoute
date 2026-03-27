param(
    [Parameter(Mandatory = $true)]
    [string]$RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = [System.IO.Path]::GetFullPath($RepoRoot)
if (-not $repoRoot.EndsWith([System.IO.Path]::DirectorySeparatorChar)) {
    $repoRoot += [System.IO.Path]::DirectorySeparatorChar
}

function Get-RelativePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if ($fullPath.StartsWith($repoRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $fullPath.Substring($repoRoot.Length)
    }

    return $fullPath
}

function Get-TrackedTimestampUtc {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $resolvedPath = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).ProviderPath
    return (Get-Item -LiteralPath $resolvedPath).LastWriteTimeUtc
}

$arch = ($env:PROCESSOR_ARCHITECTURE ?? '').ToUpperInvariant()
switch ($arch) {
    'AMD64' { $arch = 'x86_64' }
    'X64' { $arch = 'x86_64' }
    'ARM64' { $arch = 'aarch64' }
    default { $arch = $arch.ToLowerInvariant() }
}

$targetDir = "windows-$arch"

$inputFiles = @(
    [System.IO.Path]::Combine($repoRoot, 'rust', 'Cargo.toml'),
    [System.IO.Path]::Combine($repoRoot, 'rust', 'Cargo.lock'),
    [System.IO.Path]::Combine($repoRoot, 'setup.bat'),
    [System.IO.Path]::Combine($repoRoot, 'scripts', 'build_ffmpeg.bat'),
    [System.IO.Path]::Combine($repoRoot, 'scripts', 'build_whisper.bat'),
    [System.IO.Path]::Combine($repoRoot, 'scripts', 'build_llama.bat'),
    [System.IO.Path]::Combine($repoRoot, '.build', 'ffmpeg', $targetDir, 'install', 'bin', 'ffmpeg.exe'),
    [System.IO.Path]::Combine($repoRoot, '.build', 'ffmpeg', $targetDir, 'install', 'bin', 'ffprobe.exe'),
    [System.IO.Path]::Combine($repoRoot, '.build', 'whisper', $targetDir, 'bin', 'whisper-cli.exe'),
    [System.IO.Path]::Combine($repoRoot, '.build', 'llama', $targetDir, 'bin', 'llama-cli.exe'),
    [System.IO.Path]::Combine($repoRoot, '.build', 'llama', $targetDir, 'bin', 'llama-embedding.exe')
)

$outputFiles = @(
    [System.IO.Path]::Combine($repoRoot, 'package', 'RecordRoute.exe'),
    [System.IO.Path]::Combine($repoRoot, 'package', 'RecordRouteServer.exe'),
    [System.IO.Path]::Combine($repoRoot, 'package', '.build', 'ffmpeg', $targetDir, 'install', 'bin', 'ffmpeg.exe'),
    [System.IO.Path]::Combine($repoRoot, 'package', '.build', 'ffmpeg', $targetDir, 'install', 'bin', 'ffprobe.exe'),
    [System.IO.Path]::Combine($repoRoot, 'package', '.build', 'whisper', $targetDir, 'bin', 'whisper-cli.exe'),
    [System.IO.Path]::Combine($repoRoot, 'package', '.build', 'llama', $targetDir, 'bin', 'llama-cli.exe'),
    [System.IO.Path]::Combine($repoRoot, 'package', '.build', 'llama', $targetDir, 'bin', 'llama-embedding.exe')
)

$latestInput = $null
foreach ($path in $inputFiles) {
    if (Test-Path -LiteralPath $path -PathType Leaf) {
        $timestamp = Get-TrackedTimestampUtc -Path $path
        if (-not $latestInput -or $timestamp -gt $latestInput.Timestamp) {
            $latestInput = [pscustomobject]@{
                Path = $path
                Timestamp = $timestamp
            }
        }
    }
}

$rustSrc = [System.IO.Path]::Combine($repoRoot, 'rust', 'src')
if (Test-Path -LiteralPath $rustSrc -PathType Container) {
    Get-ChildItem -LiteralPath $rustSrc -File -Recurse | ForEach-Object {
        $timestamp = Get-TrackedTimestampUtc -Path $_.FullName
        if (-not $latestInput -or $timestamp -gt $latestInput.Timestamp) {
            $latestInput = [pscustomobject]@{
                Path = $_.FullName
                Timestamp = $timestamp
            }
        }
    }
}

$oldestOutput = $null
foreach ($path in $outputFiles) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        [Console]::Error.WriteLine('RecordRoute package runtime is incomplete.')
        [Console]::Error.WriteLine(("Missing packaged artifact: {0}" -f (Get-RelativePath -Path $path)))
        [Console]::Error.WriteLine('Run setup.bat and rerun run.bat.')
        exit 1
    }

    $timestamp = Get-TrackedTimestampUtc -Path $path
    if (-not $oldestOutput -or $timestamp -lt $oldestOutput.Timestamp) {
        $oldestOutput = [pscustomobject]@{
            Path = $path
            Timestamp = $timestamp
        }
    }
}

if ($latestInput -and $oldestOutput -and $latestInput.Timestamp -gt $oldestOutput.Timestamp) {
    [Console]::Error.WriteLine('RecordRoute package is stale. Repo inputs are newer than the packaged runtime.')
    [Console]::Error.WriteLine(("Latest changed input: {0}" -f (Get-RelativePath -Path $latestInput.Path)))
    [Console]::Error.WriteLine(("Oldest packaged artifact: {0}" -f (Get-RelativePath -Path $oldestOutput.Path)))
    [Console]::Error.WriteLine('Run setup.bat and rerun run.bat.')
    exit 1
}

exit 0
