<#
.SYNOPSIS
    Publishes Glance to this PC: closes what is running, builds, installs into
    C:\Program Files\Glance, and starts Glance again.

.DESCRIPTION
    The whole round, in the order it is wanted: close, build, publish, open.
    Program Files is protected, so run it elevated:
        Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','j:\Projects\glance\scripts\publish.ps1'
    What it says is also written to target\publish.log, as the elevated window
    closes once it is done.

    -NoBuild publishes the build that is already there. -NoOpen publishes without
    starting Glance again. -WithRemote builds the phone remote too (only its own
    crate changed, so it is not built every time). -KeepDock leaves the settings
    file alone.

    It builds the fast profile by default (incremental, no LTO — see [profile.fast]
    in Cargo.toml), so a one-line change lands in well under a minute; pass -Release
    for the shippable whole-crate LTO build, which takes minutes.
#>
[CmdletBinding()]
param(
    [switch] $NoBuild,
    [switch] $NoOpen,
    [switch] $WithRemote,
    [switch] $KeepDock,
    [switch] $Release
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$buildProfile = if ($Release) { 'release' } else { 'fast' }
$profileFlag = if ($Release) { '--release' } else { '--profile fast' }
$built = Join-Path $root "target\$buildProfile"
$where = 'C:\Program Files\Glance'
$config = Join-Path $env:APPDATA 'dev.weiyi.glance\settings.json'
$names = 'glance', 'glance-sensors', 'glance-remote'

Start-Transcript -Path (Join-Path $root 'target\publish.log') -Append | Out-Null
trap {
    Write-Host "FAILED: $_"
    Stop-Transcript | Out-Null
    Start-Sleep -Seconds 20
    exit 1
}

Set-Location -LiteralPath $root

# ---- closed -------------------------------------------------------------------
Write-Host '==> closing what is running'
$running = Get-Process -Name $names -ErrorAction SilentlyContinue
if ($running) {
    # (Glance keeps its settings as they are changed, not only as it goes, so
    # this loses nothing it holds.)
    $running | ForEach-Object { Write-Host ("    {0} (pid {1})" -f $_.Name, $_.Id) }
    Stop-Process -Name $names -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
} else {
    Write-Host '    nothing running'
}

# ---- built ---------------------------------------------------------------------
if (-not $NoBuild) {
    Write-Host '==> building'
    $bash = 'C:\Program Files\Git\bin\bash.exe'
    if (-not (Test-Path -LiteralPath $bash)) { throw "$bash not found; the build needs the MSVC environment build-env.sh sets" }
    # (The main crate by the chosen profile. The remote is a separate crate that knows only the built-in profiles, so
    # it stays --release. The same build.sh runs, by whatever profile is asked for.)
    # Windows PowerShell 5.1 turns a native command's stderr (cargo's warnings) into a
    # terminating error under Stop, so relax it just for this call and judge by the exit code.
    Write-Host "    profile: $buildProfile"
    $ErrorActionPreference = 'Continue'
    & $bash -lc ('cd "' + ($root -replace '\\', '/') + '" && . ./build-env.sh && cargo build ' + $profileFlag + $(if ($WithRemote) { ' && cargo build --release --manifest-path remote/Cargo.toml' } else { '' }))
    $build = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($build -ne 0) { throw "cargo build exited $build" }
}

# ---- published -------------------------------------------------------------------
Write-Host '==> installing'
$files = 'glance.exe', 'glance-sensors.exe'
if ($WithRemote) { $files += 'glance-remote.exe' }
foreach ($name in $files) {
    $from = Join-Path $built $name
    if ($WithRemote -and $name -eq 'glance-remote.exe') { $from = Join-Path $root 'remote\target\release\glance-remote.exe' }
    if (-not (Test-Path -LiteralPath $from)) { throw "$from is not built; run without -NoBuild" }
    $to = Join-Path $where $name
    # (A copy of what is already there is written aside once, and taken no
    # further: run twice over the same build, there is nothing new to keep.)
    $same = (Test-Path -LiteralPath $to) -and
        ((Get-FileHash -LiteralPath $from -Algorithm SHA256).Hash -eq (Get-FileHash -LiteralPath $to -Algorithm SHA256).Hash)
    if ($same) {
        Write-Host "    $name is already this build"
        continue
    }
    if (Test-Path -LiteralPath $to) {
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        Copy-Item -LiteralPath $to -Destination "$to.before-$stamp" -Force
        Write-Host "    kept the file that was there: $to.before-$stamp"
    }
    # (The exe just stopped can still be memory-mapped for a moment, and Windows refuses to overwrite it then
    # (ERROR_USER_MAPPED_FILE), so wait for the hold to clear and try again rather than failing the publish.)
    for ($try = 1; ; $try++) {
        try {
            Copy-Item -LiteralPath $from -Destination $to -Force
            break
        } catch {
            if ($try -ge 15) { throw }
            Write-Host "    $to still held, waiting ($try)"
            Start-Sleep -Seconds 1
        }
    }
    $now = Get-Item -LiteralPath $to
    Write-Host ("    {0}  {1} bytes  {2}" -f $now.Name, $now.Length, $now.LastWriteTime)
}

if (-not $KeepDock) {
    Write-Host '==> the dock'
    if (Test-Path -LiteralPath $config) {
        $raw = Get-Content -LiteralPath $config -Raw
        if ($raw -match '"dock"') {
            Write-Host '    already in the settings: left as it is'
        } else {
            Copy-Item -LiteralPath $config -Destination "$config.before-dock" -Force
            Write-Host "    the settings as they were: $config.before-dock"
            # (DockSettings reads what is not written as its own default, so
            # the switch alone is enough: its tiles come with it.)
            $head = "{`n"
            if (-not $raw.StartsWith($head)) { $head = "{`r`n" }
            if (-not $raw.StartsWith($head)) { throw "cannot read the settings: $config does not begin with an object" }
            $added = $head + "  `"dock`": {`n    `"on`": true`n  },`n" + $raw.Substring($head.Length)
            $text = New-Object System.Text.UTF8Encoding($false)
            [System.IO.File]::WriteAllText($config, $added, $text)
            Write-Host '    turned on'
        }
    } else {
        Write-Host "    no settings file yet ($config): Glance starts with its own defaults, where the dock is off - turn it on under Settings"
    }
}

# ---- opened ----------------------------------------------------------------------
if ($NoOpen) { Write-Host '==> not opened'; Stop-Transcript | Out-Null; exit 0 }
Write-Host '==> opening Glance'
Start-Process -FilePath (Join-Path $where 'glance.exe')
Start-Sleep -Seconds 6
$up = Get-Process -Name 'glance' -ErrorAction SilentlyContinue
if (-not $up) { throw "glance.exe is not running after 6 seconds; read the log at $config" }
Write-Host ("    running as pid {0}" -f ($up.Id -join ', '))
Stop-Transcript | Out-Null
