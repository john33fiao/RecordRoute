param(
    [Parameter(Mandatory = $true)]
    [string]$LauncherPath,

    [Parameter(Mandatory = $true)]
    [string]$FrontendDir
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Stop-ProcessTree {
    param(
        [System.Diagnostics.Process]$Process
    )

    if ($null -eq $Process) {
        return
    }

    try {
        $Process.Refresh()
    } catch {
        return
    }

    if ($Process.HasExited) {
        return
    }

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        & taskkill.exe /PID $Process.Id /T /F 2>$null | Out-Null
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }

    try {
        $Process.WaitForExit()
    } catch {
    }
}

function Start-ChildProcess {
    param(
        [Parameter(Mandatory = $true)]
        [string]$FilePath,

        [string]$Arguments = '',

        [Parameter(Mandatory = $true)]
        [string]$WorkingDirectory
    )

    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = $FilePath
    $startInfo.Arguments = $Arguments
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardInput = $false
    $startInfo.RedirectStandardOutput = $false
    $startInfo.RedirectStandardError = $false
    $startInfo.CreateNoWindow = $false

    return [System.Diagnostics.Process]::Start($startInfo)
}

$launcherProcess = $null
$frontendProcess = $null
$status = 0

try {
    $launcherWorkingDirectory = Split-Path -Parent $LauncherPath
    $launcherProcess = Start-ChildProcess -FilePath $LauncherPath -WorkingDirectory $launcherWorkingDirectory
    $frontendProcess = Start-ChildProcess -FilePath $env:ComSpec -Arguments '/d /c npm run dev' -WorkingDirectory $FrontendDir

    while ($true) {
        $launcherProcess.Refresh()
        if ($launcherProcess.HasExited) {
            $status = $launcherProcess.ExitCode
            break
        }

        $frontendProcess.Refresh()
        if ($frontendProcess.HasExited) {
            $status = $frontendProcess.ExitCode
            break
        }

        Start-Sleep -Seconds 1
    }
}
catch [System.Management.Automation.PipelineStoppedException] {
    $status = 130
}
finally {
    Stop-ProcessTree -Process $frontendProcess
    Stop-ProcessTree -Process $launcherProcess
}

exit $status
