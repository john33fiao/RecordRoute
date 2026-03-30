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

function Get-PlatformArch {
    $arch = [System.Environment]::GetEnvironmentVariable('PROCESSOR_ARCHITEW6432')
    if ([string]::IsNullOrWhiteSpace($arch)) {
        $arch = [System.Environment]::GetEnvironmentVariable('PROCESSOR_ARCHITECTURE')
    }
    if ($null -eq $arch) {
        $arch = ''
    }

    switch ($arch.ToUpperInvariant()) {
        'AMD64' { return 'x86_64' }
        'X64' { return 'x86_64' }
        'ARM64' { return 'aarch64' }
        default { return $arch.ToLowerInvariant() }
    }
}

function Update-LatestInput {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return
    }

    $timestamp = Get-TrackedTimestampUtc -Path $Path
    if (-not $script:latestInput -or $timestamp -gt $script:latestInput.Timestamp) {
        $script:latestInput = [pscustomobject]@{
            Path = $Path
            Timestamp = $timestamp
        }
    }
}

function Update-LatestInputTree {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Directory
    )

    if (-not (Test-Path -LiteralPath $Directory -PathType Container)) {
        return
    }

    Get-ChildItem -LiteralPath $Directory -File -Recurse | ForEach-Object {
        Update-LatestInput -Path $_.FullName
    }
}

function Update-OldestOutput {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        [Console]::Error.WriteLine('RecordRoute package runtime is incomplete.')
        [Console]::Error.WriteLine(("Missing packaged artifact: {0}" -f (Get-RelativePath -Path $Path)))
        [Console]::Error.WriteLine('Run setup.bat and rerun run.bat.')
        exit 1
    }

    $timestamp = Get-TrackedTimestampUtc -Path $Path
    if (-not $script:oldestOutput -or $timestamp -lt $script:oldestOutput.Timestamp) {
        $script:oldestOutput = [pscustomobject]@{
            Path = $Path
            Timestamp = $timestamp
        }
    }
}

$targetDir = "windows-$(Get-PlatformArch)"

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

$script:latestInput = $null
foreach ($path in $inputFiles) {
    Update-LatestInput -Path $path
}
Update-LatestInputTree -Directory ([System.IO.Path]::Combine($repoRoot, 'rust', 'src'))

$script:oldestOutput = $null
foreach ($path in $outputFiles) {
    Update-OldestOutput -Path $path
}

if ($script:latestInput -and $script:oldestOutput -and $script:latestInput.Timestamp -gt $script:oldestOutput.Timestamp) {
    [Console]::Error.WriteLine('RecordRoute package is stale. Repo inputs are newer than the packaged runtime.')
    [Console]::Error.WriteLine(("Latest changed input: {0}" -f (Get-RelativePath -Path $script:latestInput.Path)))
    [Console]::Error.WriteLine(("Oldest packaged artifact: {0}" -f (Get-RelativePath -Path $script:oldestOutput.Path)))
    [Console]::Error.WriteLine('Run setup.bat and rerun run.bat.')
    exit 1
}

exit 0
