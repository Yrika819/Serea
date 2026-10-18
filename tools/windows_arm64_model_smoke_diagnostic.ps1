param(
    [Parameter(Mandatory = $true)]
    [string] $HostExe,

    [Parameter(Mandatory = $true)]
    [string] $DataDir,

    [string] $TaskId,

    [ValidateRange(1, 12)]
    [int] $Iterations = 3,

    [ValidateRange(1, 60)]
    [int] $TimeoutSeconds = 30
)

$ErrorActionPreference = 'Stop'
$resolvedExe = (Resolve-Path -LiteralPath $HostExe).Path
$resolvedDataDir = (Resolve-Path -LiteralPath $DataDir).Path
$os = Get-CimInstance -ClassName Win32_OperatingSystem
$nativeArch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
$processArch = [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()

if ($env:OS -ne 'Windows_NT' -or $nativeArch -ne 'Arm64') {
    throw "This diagnostic is limited to native Windows ARM64; detected OS=$env:OS native_arch=$nativeArch."
}

function Get-ResourceSample {
    $currentOs = Get-CimInstance -ClassName Win32_OperatingSystem
    $processes = @(Get-Process)
    $handleCount = 0L
    $threadCount = 0L
    foreach ($process in $processes) {
        $handleCount += [long]$process.HandleCount
        $threadCount += [long]$process.Threads.Count
    }

    [pscustomobject]@{
        available_memory_bytes = [long]$currentOs.FreePhysicalMemory * 1024
        system_process_count = $processes.Count
        system_handle_count = $handleCount
        system_thread_count = $threadCount
    }
}

$summary = [ordered]@{
    event = 'serea_synthetic_host_diagnostic_start'
    utc = [DateTime]::UtcNow.ToString('o')
    os_version = $os.Version
    os_architecture = $nativeArch
    process_architecture = $processArch
    host_executable = [IO.Path]::GetFileName($resolvedExe)
    iterations = $Iterations
    timeout_seconds = $TimeoutSeconds
    data_directory = [IO.Path]::GetFileName($resolvedDataDir)
    before = Get-ResourceSample
}
$summary | ConvertTo-Json -Compress

$failures = 0
for ($index = 0; $index -lt $Iterations; $index++) {
    $arguments = @('model-smoke', '--synthetic', '--data-dir', $resolvedDataDir, '--scenario', 'success')
    if ($TaskId) {
        $arguments += @('--task-id', $TaskId)
    }

    $resourcesBefore = Get-ResourceSample
    $startUtc = [DateTime]::UtcNow
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $stdoutPath = [IO.Path]::GetTempFileName()
    $stderrPath = [IO.Path]::GetTempFileName()
    $process = Start-Process -FilePath $resolvedExe -ArgumentList $arguments -PassThru -NoNewWindow `
        -RedirectStandardOutput $stdoutPath `
        -RedirectStandardError $stderrPath
    $completed = $process.WaitForExit($TimeoutSeconds * 1000)
    if (-not $completed) {
        $process.Kill()
        $process.WaitForExit()
        $watch.Stop()
        $exitCode = $null
        $stdoutBytes = $null
        $stderrBytes = $null
        $result = 'timeout'
        $failures++
    } else {
        $watch.Stop()
        $process.Refresh()
        $exitCode = [uint32]$process.ExitCode
        # Captured content is never emitted; retain only byte counts.
        $stdoutBytes = (Get-Item -LiteralPath $stdoutPath).Length
        $stderrBytes = (Get-Item -LiteralPath $stderrPath).Length
        $result = if ($exitCode -eq 0) { 'success' } else { 'nonzero_exit' }
        if ($exitCode -ne 0) { $failures++ }
    }

    [pscustomobject]@{
        event = 'serea_synthetic_host_diagnostic_attempt'
        utc = $startUtc.ToString('o')
        index = $index
        elapsed_ms = [long]$watch.ElapsedMilliseconds
        result = $result
        exit_code_decimal = $exitCode
        exit_code_hex = if ($null -ne $exitCode) { '0x{0:X8}' -f $exitCode } else { $null }
        stdout_bytes = $stdoutBytes
        stderr_bytes = $stderrBytes
        resources_before = $resourcesBefore
        resources = Get-ResourceSample
    } | ConvertTo-Json -Compress
    Remove-Item -LiteralPath $stdoutPath, $stderrPath -Force
}

[pscustomobject]@{
    event = 'serea_synthetic_host_diagnostic_end'
    utc = [DateTime]::UtcNow.ToString('o')
    attempts = $Iterations
    failures = $failures
    after = Get-ResourceSample
} | ConvertTo-Json -Compress

if ($failures -gt 0) { exit 1 }
exit 0
