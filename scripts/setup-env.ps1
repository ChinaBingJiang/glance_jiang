<#
.SYNOPSIS
    Glance 开发环境一键配置：装 MSVC 生成工具 + Windows SDK、Rust（MSVC 宿主），
    可选 NSIS。已装过的组件自动跳过。

.DESCRIPTION
    这台机器没有任何包管理器（winget/choco/scoop 都没有），所以本脚本直接用官方
    安装器 + 静默参数。会触发一次 UAC 提权（VS 与 NSIS 需要管理员权限）。

    用法：
        powershell -ExecutionPolicy Bypass -File scripts\setup-env.ps1
        powershell -ExecutionPolicy Bypass -File scripts\setup-env.ps1 -IncludeNsis
        powershell -ExecutionPolicy Bypass -File scripts\setup-env.ps1 -Passive   # 显示安装进度界面

.PARAMETER IncludeNsis
    一并安装 NSIS（只有打包安装程序 scripts\release.ps1 才需要）。

.PARAMETER Passive
    用 --passive（显示进度条）而非 --quiet（完全静默）安装 VS Build Tools；NSIS 同理去掉 /S。
    出问题时用这个更容易看到卡在哪一步。
#>
[CmdletBinding()]
param(
    [switch] $IncludeNsis,
    [switch] $Passive
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # 关掉 Invoke-WebRequest 的进度条，快很多

# ---------------------------------------------------------------------------
# 0. 需要管理员权限则自动重新提权（VS 安装器与 NSIS 都要写 Program Files）
# ---------------------------------------------------------------------------
$isAdmin = ([Security.Principal.WindowsPrincipal] `
    [Security.Principal.WindowsIdentity]::GetCurrent()
    ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-Host '需要管理员权限，正在重新以管理员身份启动脚本……' -ForegroundColor Yellow
    $self = $PSCommandPath
    $args = "-NoProfile -ExecutionPolicy Bypass -File `"$self`""
    if ($IncludeNsis) { $args += ' -IncludeNsis' }
    if ($Passive)     { $args += ' -Passive' }
    Start-Process -FilePath 'powershell.exe' -ArgumentList $args -Verb RunAs
    exit
}

# 单用户管理员账户下，提权后的 $env:USERPROFILE 与平时一致，Rust 会装进同一个用户目录。
$tmp = $env:TEMP

function Step($msg)  { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Ok($msg)    { Write-Host "    ✔ $msg" -ForegroundColor Green }
function Skip($msg)  { Write-Host "    ⏭  $msg（已存在，跳过）" -ForegroundColor DarkGray }
function Dl($url, $out) {
    Write-Host "    下载 $url" -ForegroundColor DarkGray
    Invoke-WebRequest -Uri $url -OutFile $out -UseBasicParsing
}

# ---------------------------------------------------------------------------
# 1. MSVC 生成工具 + Windows SDK（解决 cl / link / rc 与 SDK 缺失）
# ---------------------------------------------------------------------------
Step '检查 / 安装 Visual Studio Build Tools（C++ 生成工具 + Windows 11 SDK）'

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"

# 是否已装好 C++ 生成工具与 Windows SDK
function Test-VSInstalled {
    if (-not (Test-Path $vswhere)) { return $false }
    $inst = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -requires Microsoft.VisualStudio.Component.Windows11SDK.22621 `
        -property installationPath 2>$null
    return [bool]$inst
}

# 是否已有 VS 安装器在跑（引导器 / 主安装进程）
function Get-VSInstallerRunning {
    Get-Process vs_buildtools,vs_setup_bootstrapper -ErrorAction SilentlyContinue
}

if (Test-VSInstalled) {
    Skip 'VS Build Tools（含 C++ 生成工具与 Windows SDK）'
} else {
    # 上一次运行可能正卡在下载/安装里：先等它结束，别去抢同一个安装器和临时文件
    if (Get-VSInstallerRunning) {
        Step '检测到 Visual Studio 安装器正在运行，等待其完成后再继续（期间请勿关闭）……'
        while (Get-VSInstallerRunning) { Start-Sleep 15 }
        if (Test-VSInstalled) { Skip 'VS Build Tools（前一次安装已完成）'; $vsDone = $true }
        else { $vsDone = $false }
    } else { $vsDone = $false }

    if (-not $vsDone) {
        # 每次用唯一临时文件名，避免与被其它进程占用的旧文件冲突
        $vs = Join-Path $tmp ("vs_buildtools_{0}.exe" -f [guid]::NewGuid().ToString('N').Substring(0, 8))
        Dl 'https://aka.ms/vs/17/release/vs_buildtools.exe' $vs
        $mode = if ($Passive) { '--passive' } else { '--quiet' }
        Step '正在安装 VS Build Tools，下载 + 安装可能需要十几到几十分钟，请勿关闭窗口……'
        # /quiet 无界面，/passive 有进度条；--wait 阻塞到装完再返回
        $p = Start-Process -FilePath $vs -PassThru -Wait -ArgumentList @(
            $mode, '--norestart', '--wait',
            '--add', 'Microsoft.VisualStudio.Workload.VCTools',
            '--add', 'Microsoft.VisualStudio.Component.Windows11SDK.22621',
            '--includeRecommended'
        )
        # 1602/1614 常为用户取消或已存在安装，非致命
        if ($p.ExitCode -notin 0, 3010) { Remove-Item $vs -EA SilentlyContinue; throw "VS Build Tools 安装失败，退出码 $($p.ExitCode)" }
        Remove-Item $vs -EA SilentlyContinue
        Ok 'VS Build Tools 安装完成'
    }
}

# ---------------------------------------------------------------------------
# 2. Rust（rustup + stable MSVC 工具链）
# ---------------------------------------------------------------------------
Step '检查 / 安装 Rust（rustup-init，默认 stable-x86_64-pc-windows-msvc）'

function Find-Cargo {
    $c = Get-Command cargo -ErrorAction SilentlyContinue
    if ($c) { return $c.Source }
    $user = "$env:USERPROFILE\.cargo\bin\cargo.exe"
    if (Test-Path $user) { return $user }
    # 作者机器上的自定义位置（若沿用 build-env.sh）
    if (Test-Path 'D:\Tools\cargo\bin\cargo.exe') { return 'D:\Tools\cargo\bin\cargo.exe' }
    return $null
}

$cargo = Find-Cargo
if ($cargo) {
    Skip "Rust（$cargo）"
} else {
    $rustup = Join-Path $tmp ("rustup-init_{0}.exe" -f [guid]::NewGuid().ToString('N').Substring(0, 8))
    Dl 'https://win.rustup.rs/x86_64' $rustup
    Step '正在安装 Rust 工具链……'
    # -y 非交互；默认宿主 msvc；装完会自动写入用户 PATH
    & $rustup -y --default-toolchain stable-x86_64-pc-windows-msvc --profile default
    Remove-Item $rustup -EA SilentlyContinue
    if ($LASTEXITCODE -ne 0) { throw "rustup-init 失败，退出码 $LASTEXITCODE" }
    $cargo = Find-Cargo
    if (-not $cargo) { throw 'Rust 安装后未找到 cargo，请重开终端再试' }
    Ok 'Rust 安装完成'
}

# ---------------------------------------------------------------------------
# 3. NSIS（可选：只有生成安装包才需要）
# ---------------------------------------------------------------------------
if ($IncludeNsis) {
    Step '检查 / 安装 NSIS（makensis）'
    $nsis = Get-Command makensis -ErrorAction SilentlyContinue
    $nsisPath = if ($nsis) { $nsis.Source } else { "${env:ProgramFiles(x86)}\NSIS\makensis.exe" }
    if (Test-Path $nsisPath) {
        Skip 'NSIS'
    } else {
        $nsisExe = Join-Path $tmp ("nsis-setup_{0}.exe" -f [guid]::NewGuid().ToString('N').Substring(0, 8))
        Dl 'https://prdownloads.sourceforge.net/nsis/nsis-3.10-setup.exe?download' $nsisExe
        if ($Passive) { & $nsisExe } else { Start-Process $nsisExe -Wait -ArgumentList '/S' }
        Remove-Item $nsisExe -EA SilentlyContinue
        Ok 'NSIS 安装完成（makensis 默认在 C:\Program Files (x86)\NSIS\）'
    }
}

# ---------------------------------------------------------------------------
# 4. 自检
# ---------------------------------------------------------------------------
Step '环境自检'
Write-Host '    提示：以下命令请【重开一个终端】后再跑，PATH 才会刷新。' -ForegroundColor Yellow
try { & $cargo --version }        catch { Write-Host '    cargo 未就绪' -ForegroundColor Red }
try { & 'rustc' --version }       catch { Write-Host '    rustc 未就绪（可能需重开终端）' -ForegroundColor Yellow }

Write-Host "`n全部完成。下一步在项目根目录执行：cargo build --release" -ForegroundColor Green
if (-not $Passive) {
    Write-Host "（若想看到安装进度界面，重跑本脚本并加 -Passive）" -ForegroundColor DarkGray
}
