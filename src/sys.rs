//! The low side the system-settings pages ask of Windows beyond a registry key
//! or a switch: the network's adapters and a ping of them, the room a clean-up
//! could win back, and the storage sense's own flag. Each is read as Windows
//! itself keeps it, and written back the way a program of the user's may.
//!
//! Windows only. The pages call into here, and read back what they wrote so a
//! control never lies about what the system now is.

use std::process::Command;
use std::sync::atomic::{AtomicU8, Ordering};

use windows::core::{BOOL, HSTRING, PCWSTR};
use windows::Win32::Devices::Bluetooth::{
    BluetoothAuthenticateDevice, BluetoothEnableDiscovery, BluetoothFindDeviceClose,
    BluetoothFindFirstDevice, BluetoothFindFirstRadio, BluetoothFindNextDevice,
    BluetoothFindNextRadio, BluetoothFindRadioClose, BluetoothGetRadioInfo,
    BluetoothIsDiscoverable, BLUETOOTH_ADDRESS, BLUETOOTH_DEVICE_INFO,
    BLUETOOTH_DEVICE_SEARCH_PARAMS, BLUETOOTH_FIND_RADIO_PARAMS, BLUETOOTH_RADIO_INFO,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE, LPARAM, POINTL, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    ChangeDisplaySettingsExW, EnumDisplayMonitors, EnumDisplaySettingsW, GetDC, GetMonitorInfoW,
    MonitorFromRect, ReleaseDC, CDS_NORESET, CDS_RESET, CDS_SET_PRIMARY, CDS_TYPE,
    CDS_UPDATEREGISTRY, DEVMODEW, DISP_CHANGE_SUCCESSFUL, DM_DISPLAYFREQUENCY, DM_PELSHEIGHT,
    DM_PELSWIDTH, DM_POSITION, ENUM_CURRENT_SETTINGS, ENUM_DISPLAY_SETTINGS_MODE, HDC, HMONITOR,
    MONITORINFO, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::NetworkManagement::IpHelper::{
    GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, IP_ADAPTER_ADDRESSES_LH,
};
use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN, SOCKET_ADDRESS};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegGetValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE,
    REG_DWORD, REG_OPTION_NON_VOLATILE, RRF_RT_REG_DWORD,
};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::ColorSystem::SetDeviceGammaRamp;
use windows::Win32::UI::Shell::{
    SHEmptyRecycleBinW, SHQueryRecycleBinW, ShellExecuteW, SHERB_NOCONFIRMATION,
    SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHQUERYRBINFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};

/// The whole of an adapter, as the network page shows it.
#[derive(Clone)]
pub struct Adapter {
    /// The name the user knows it by ("Wi-Fi", "以太网"), which is also the alias
    /// `netsh` takes when its numbers are set.
    pub name: String,
    /// The device behind it, as Windows describes it ("Intel Wi-Fi 6 AX201").
    pub desc: String,
    /// Its address, dotted; none where it is down or unnumbered.
    pub ipv4: Option<String>,
    /// The gateway it reaches out by.
    pub gateway: Option<String>,
    /// The name servers it was given, joined.
    pub dns: String,
    /// The interface's own index, as Windows numbers it.
    pub index: u32,
    /// Its hardware address, dashed; empty where the interface has none.
    pub mac: String,
    /// Its subnet mask, dotted; empty where it is unnumbered.
    pub mask: String,
    /// Whether an address was handed out to it rather than set by hand.
    pub dhcp: bool,
    /// What kind of link it is, as Windows types it (6 Ethernet, 71 Wi-Fi…).
    pub kind: u32,
    /// The speed of its link, in bits a second; 0 where it is down or unknown.
    pub link_bps: u64,
    /// Whether the link is up now, as the operating status reports it.
    pub up: bool,
}

/// The mask a prefix length (0..32) stands for, dotted.
fn mask_of(prefix: u8) -> String {
    let bits = if prefix == 0 {
        0u32
    } else {
        0xFFFF_FFFFu32.checked_shl(32 - prefix as u32).unwrap_or(0)
    };
    let octets = bits.to_be_bytes();
    format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3])
}

/// Every adapter Windows lists that reaches somewhere, each with the address,
/// gateway and name servers it is now using.
pub fn adapters() -> Vec<Adapter> {
    let mut buf: Vec<u64> = Vec::new();
    let mut size = 0u32;
    loop {
        let list = buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        let status = unsafe {
            GetAdaptersAddresses(
                AF_INET.0 as u32,
                GAA_FLAG_INCLUDE_GATEWAYS,
                None,
                Some(list),
                &mut size,
            )
        };
        if status == ERROR_BUFFER_OVERFLOW {
            buf.resize((size as usize).div_ceil(8), 0);
            continue;
        }
        if status != ERROR_SUCCESS {
            return Vec::new();
        }
        let mut found = Vec::new();
        let mut here = list as *const IP_ADAPTER_ADDRESSES_LH;
        while !here.is_null() {
            let a = unsafe { &*here };
            // The first numbered address down the unicast list, if any.
            let mut at = a.FirstUnicastAddress;
            let mut ipv4 = None;
            let mut prefix = 0u8;
            while !at.is_null() {
                if let Some(said) = dotted(unsafe { (*at).Address }) {
                    ipv4 = Some(said);
                    prefix = unsafe { (*at).OnLinkPrefixLength };
                    break;
                }
                at = unsafe { (*at).Next };
            }
            let mut at = a.FirstGatewayAddress;
            let mut gateway = None;
            while !at.is_null() {
                if let Some(said) = dotted(unsafe { (*at).Address }) {
                    gateway = Some(said);
                    break;
                }
                at = unsafe { (*at).Next };
            }
            let mut at = a.FirstDnsServerAddress;
            let mut servers = Vec::new();
            while !at.is_null() {
                if let Some(said) = dotted(unsafe { (*at).Address }) {
                    servers.push(said);
                }
                at = unsafe { (*at).Next };
            }
            let dns = servers.join("   ");
            let name = unsafe { a.FriendlyName.to_string() }.unwrap_or_default();
            let desc = unsafe { a.Description.to_string() }.unwrap_or_default();
            // Its hardware address, as Windows writes it: hex pairs, dashed.
            let bytes = unsafe { &a.PhysicalAddress[..a.PhysicalAddressLength.min(8) as usize] };
            let mac = bytes
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join("-");
            // A server handed an address out to it, rather than one set by hand.
            let dhcp = dotted(unsafe { a.Dhcpv4Server }).is_some();
            let mask = if ipv4.is_some() {
                mask_of(prefix)
            } else {
                String::new()
            };
            let link_bps = unsafe { a.TransmitLinkSpeed };
            let up = unsafe { a.OperStatus.0 } == 1;
            if !name.is_empty() && (ipv4.is_some() || !dns.is_empty()) {
                found.push(Adapter {
                    name,
                    desc,
                    ipv4,
                    gateway,
                    dns,
                    index: unsafe { a.Anonymous1.Anonymous.IfIndex },
                    mac,
                    mask,
                    dhcp,
                    kind: unsafe { a.IfType },
                    link_bps,
                    up,
                });
            }
            here = unsafe { (*here).Next };
        }
        return found;
    }
}

/// An app the machine would start of its own accord when the user signs in:
/// the name the registry keeps it under, the friendly name Windows shows for it
/// (the launched program's own description), the command it runs, whether it is
/// set under the whole machine or just this user, and whether it is let now.
#[derive(Clone)]
pub struct Startup {
    pub name: String,
    pub display: String,
    pub command: String,
    pub machine: bool,
    pub enabled: bool,
}

/// The apps kept for start-up, read as the shell keeps them: the entries under
/// both `Run` keys, each paired with its `StartupApproved` mark — a value whose
/// first byte is `3` is one the shell is holding back. Each is named as Windows
/// names it on its own login-items page: the friendly `FileDescription` read
/// from the program the command runs, falling back to the registry's own name.
/// Read through the machine's own PowerShell, for the pair spans two hives, the
/// marks are binary, and the name lives in the file's version resource.
pub fn startup_list() -> Vec<Startup> {
    let script = "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
                  function d($cmd){ $p=''; \
                    if($cmd -match '^\"([^\"]+)\"'){ $p=$matches[1] } \
                    elseif($cmd -match '^(\\S+\\.exe)'){ $p=$matches[1] } \
                    else { $p=($cmd -split ' ')[0] }; \
                    $r=''; if($p -and (Test-Path -LiteralPath $p -ErrorAction SilentlyContinue)){ \
                      $r=(Get-Item -LiteralPath $p).VersionInfo.FileDescription }; if(-not $r){$r=''}; $r }; \
                  function g($run,$app,$mach){ \
                    $k=Get-Item -LiteralPath $run -ErrorAction SilentlyContinue; if(-not $k){return}; \
                    $a=Get-Item -LiteralPath $app -ErrorAction SilentlyContinue; \
                    $k.GetValueNames() | ForEach-Object { $n=$_; if($n){ \
                      $cmd=[string]($k.GetValue($n)); \
                      $on=$true; if($a){$b=$a.GetValue($n); if($b -and $b.Length -ge 1 -and $b[0] -eq 3){$on=$false}}; \
                      '{0}|{1}|{2}|{3}|{4}' -f $on,$mach,$n,(d $cmd),$cmd}}}; \
                  g 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run' '0'; \
                  g 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run' 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run' '1'";
    let said = match run("powershell.exe", &["-NoProfile", "-Command", script]) {
        Some(said) => said,
        None => return Vec::new(),
    };
    let mut found = Vec::new();
    for line in said.lines() {
        let bits = line.splitn(5, '|').collect::<Vec<_>>();
        if bits.len() < 5 || bits[2].is_empty() {
            continue;
        }
        let name = bits[2].trim().to_string();
        // The friendly name where the file gave one, the registry's own name
        // where it did not — so a row is never left without a title.
        let display = {
            let said = bits[3].trim();
            if said.is_empty() {
                name.clone()
            } else {
                said.to_string()
            }
        };
        found.push(Startup {
            name,
            display,
            command: bits[4].trim().to_string(),
            machine: bits[1].trim() == "1",
            enabled: bits[0].trim() == "True",
        });
    }
    found
}

/// Whether to let one start-up app run, written the way the shell reads it back:
/// the `StartupApproved` mark set to twelve bytes leading `2` to let it, `3` to
/// hold it. The name is doubled into a quoted literal so a stray quote cannot
/// break the script. The machine's own entries want the administrator's leave —
/// where it is refused the write simply does not land, and the page reads the
/// old state back rather than lying. Returns whether PowerShell exited clean.
pub fn startup_set(machine: bool, name: &str, on: bool) -> bool {
    let hive = if machine {
        r"HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
    } else {
        r"HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
    };
    let head = if on { 2u8 } else { 3u8 };
    let quoted = name.replace('\'', "''");
    let script = format!(
        "$b=[byte[]]({head},0,0,0,0,0,0,0,0,0,0,0); \
         New-ItemProperty -LiteralPath '{hive}' -Name '{quoted}' -Value $b -PropertyType Binary -Force | Out-Null"
    );
    ok("powershell.exe", &["-NoProfile", "-Command", &script])
}

/// An app Windows keeps a notification setting for: the friendly name it is
/// shown by, the id (AUMID) the setting is kept under, and whether its banners
/// are let now.
#[derive(Clone)]
pub struct NotifyApp {
    pub display: String,
    pub aumid: String,
    pub enabled: bool,
}

/// The apps that have left a per-app notification setting behind, read from the
/// subkeys under `…\Notifications\Settings`: each subkey is one app, its
/// `Enabled` word a banner let (or, where it is `0`, held); a subkey without the
/// word is let, as Windows counts it. The ids are turned into friendly names
/// through `Get-StartApps`, which pairs each Start-menu app's name with the very
/// id the setting is kept under — matched by the package family (the part before
/// the `!`), so an app's `…!App` and `…!MicrosoftEdge` spellings agree. An id
/// `Get-StartApps` cannot name is shown cleaned of its `!` suffix and publisher
/// hash. The throwaway `NotifyIconGeneratedAumid_…` ids the old balloon icons
/// leave behind are left out, as Windows' own page leaves them out. Read through
/// PowerShell, as the start-up list is.
pub fn notify_apps() -> Vec<NotifyApp> {
    let script = "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
                  $fam=@{}; \
                  Get-StartApps | ForEach-Object { if($_.AppID){ $f=($_.AppID -split '!')[0]; if(-not $fam.ContainsKey($f)){ $fam[$f]=$_.Name } } }; \
                  Get-ChildItem 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Notifications\\Settings' -ErrorAction SilentlyContinue | ForEach-Object { \
                    $id=$_.PSChildName; if($id -like 'NOC_*' -or $id -like 'NotifyIconGeneratedAumid_*'){ return }; \
                    $k=Get-Item $_.PSPath; $on=$true; \
                    if(($k.GetValueNames()) -contains 'Enabled'){ if([int]($k.GetValue('Enabled')) -eq 0){ $on=$false } }; \
                    $f=($id -split '!')[0]; if($fam.ContainsKey($f)){ $n=$fam[$f] } else { $n=($f -replace '_[A-Za-z0-9]{13}$','') }; \
                    '{0}|{1}|{2}' -f $on,$id,$n}";
    let said = match run("powershell.exe", &["-NoProfile", "-Command", script]) {
        Some(said) => said,
        None => return Vec::new(),
    };
    let mut found = Vec::new();
    for line in said.lines() {
        let bits = line.splitn(3, '|').collect::<Vec<_>>();
        if bits.len() < 3 || bits[1].is_empty() {
            continue;
        }
        found.push(NotifyApp {
            display: bits[2].trim().to_string(),
            aumid: bits[1].trim().to_string(),
            enabled: bits[0].trim() == "True",
        });
    }
    found.sort_by(|a, b| a.display.to_lowercase().cmp(&b.display.to_lowercase()));
    found
}

/// Lets or holds one app's banners, written the `Enabled` word Windows reads
/// under its id. The id is doubled into a quoted literal so a stray quote cannot
/// break the script. Returns whether PowerShell exited clean.
pub fn notify_set(aumid: &str, on: bool) -> bool {
    let quoted = aumid.replace('\'', "''");
    let script = format!(
        "New-ItemProperty -LiteralPath 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Notifications\\Settings\\{quoted}' -Name 'Enabled' -PropertyType DWord -Value {} -Force | Out-Null",
        u32::from(on)
    );
    ok("powershell.exe", &["-NoProfile", "-Command", &script])
}

/// An app that has left a per-app consent mark for one capability (camera, the
/// microphone, the whereabout): the friendly name, the keys the marks are kept
/// under (one plain program can leave several, across its versioned folders),
/// whether it is a packaged (Store) app rather than a plain program, and
/// whether it is let now (no mark under it an outright `Deny`).
#[derive(Clone)]
pub struct ConsentApp {
    pub display: String,
    pub keys: Vec<String>,
    pub packaged: bool,
    pub allowed: bool,
}

/// The apps that have asked for `capability` and left a mark behind, read from
/// the subkeys under `…\CapabilityAccessManager\ConsentStore\<capability>`: a
/// packaged app is a subkey named by its package family, a plain program one
/// under `NonPackaged\` named by its path with `\` turned to `#`. Each carries a
/// `Value` of Allow, Deny, Prompt or none; only `Deny` holds it back, so the
/// rest count as let. The families are named through `Get-StartApps` as the
/// notification list is; a plain program is named by its file's own description
/// (so a Chinese app shows its Chinese name), its bare file name only where the
/// file gives none. Programs that share a name are folded into one row, held
/// back if any of their marks is. Read through PowerShell, as the lists are.
pub fn consent_apps(capability: &str) -> Vec<ConsentApp> {
    let script = format!(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
         $fam=@{{}}; \
         Get-StartApps | ForEach-Object {{ if($_.AppID){{ $f=($_.AppID -split '!')[0]; if(-not $fam.ContainsKey($f)){{ $fam[$f]=$_.Name }} }} }}; \
         $p='HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\{capability}'; \
         Get-ChildItem $p -ErrorAction SilentlyContinue | ForEach-Object {{ \
           $n=$_.PSChildName; \
           if($n -eq 'NonPackaged'){{ Get-ChildItem $_.PSPath -ErrorAction SilentlyContinue | ForEach-Object {{ \
             $k=$_.PSChildName; $v=[string](Get-ItemProperty -LiteralPath $_.PSPath -Name Value -ErrorAction SilentlyContinue).Value; \
             $pp=$k.Replace('#','\\'); \
             $d=(Get-Item -LiteralPath $pp -ErrorAction SilentlyContinue).VersionInfo.FileDescription; \
             if(-not $d){{ $d=(($k -split '#')[-1] -replace '\\.exe$','') }}; \
             'NP|{{0}}|{{1}}|{{2}}' -f $v,$k,$d }} }} \
           else {{ \
             $v=[string](Get-ItemProperty -LiteralPath $_.PSPath -Name Value -ErrorAction SilentlyContinue).Value; \
             if($fam.ContainsKey($n)){{ $d=$fam[$n] }} else {{ $d=($n -replace '_[A-Za-z0-9]{{13}}$','') }}; \
             'UP|{{0}}|{{1}}|{{2}}' -f $v,$n,$d }} }}"
    );
    let said = match run("powershell.exe", &["-NoProfile", "-Command", &script]) {
        Some(said) => said,
        None => return Vec::new(),
    };
    // Folded by the name shown: one program across its versioned folders is one
    // row, and it is held back if any of its marks is a `Deny`.
    let mut found: Vec<ConsentApp> = Vec::new();
    for line in said.lines() {
        let bits = line.splitn(4, '|').collect::<Vec<_>>();
        if bits.len() < 4 || bits[2].is_empty() {
            continue;
        }
        let packaged = bits[0].trim() == "UP";
        let allowed = bits[1].trim() != "Deny";
        let display = bits[3].trim().to_string();
        let key = bits[2].trim().to_string();
        if let Some(same) = found
            .iter_mut()
            .find(|app| app.packaged == packaged && app.display == display)
        {
            same.keys.push(key);
            same.allowed &= allowed;
        } else {
            found.push(ConsentApp {
                display,
                keys: vec![key],
                packaged,
                allowed,
            });
        }
    }
    found.sort_by(|a, b| a.display.to_lowercase().cmp(&b.display.to_lowercase()));
    found
}

/// Lets or holds one app's use of `capability`, written the `Value` Windows
/// reads under each of its keys — the packaged app's own subkey, or the plain
/// program's under `NonPackaged`. Each key is doubled into a quoted literal so a
/// stray quote cannot break the script. Returns whether every write went clean.
pub fn consent_app_set(capability: &str, keys: &[String], packaged: bool, allow: bool) -> bool {
    keys.iter().all(|key| {
        let quoted = key.replace('\'', "''");
        let leaf = if packaged {
            format!(
                r"HKCU:\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\{capability}\{quoted}"
            )
        } else {
            format!(
                r"HKCU:\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\{capability}\NonPackaged\{quoted}"
            )
        };
        let script = format!(
            "New-ItemProperty -LiteralPath '{leaf}' -Name 'Value' -PropertyType String -Value '{}' -Force | Out-Null",
            if allow { "Allow" } else { "Deny" }
        );
        ok("powershell.exe", &["-NoProfile", "-Command", &script])
    })
}

/// `GetAdaptersAddresses`'s two answers, by their win32 number.
const ERROR_SUCCESS: u32 = 0;
const ERROR_BUFFER_OVERFLOW: u32 = 111;

/// A socket address, taken as the four numbers Windows means, if it is IPv4.
fn dotted(address: SOCKET_ADDRESS) -> Option<String> {
    let socket = address.lpSockaddr as *const SOCKADDR_IN;
    if socket.is_null() || unsafe { (*socket).sin_family.0 } as u32 != AF_INET.0 as u32 {
        return None;
    }
    let octets = unsafe { (*socket).sin_addr.S_un.S_addr }.to_ne_bytes();
    Some(format!(
        "{}.{}.{}.{}",
        octets[0], octets[1], octets[2], octets[3]
    ))
}

/// A host answered: how many of four probes got back, and the mean time of
/// those that did, in `ms`. The count is had from the replies' own `TTL=`, the
/// time from the `=…ms` — both said alike in either tongue Windows speaks.
pub struct Probe {
    pub back: u32,
    pub sent: u32,
    pub mean: Option<u32>,
}

/// Four probes of `host` through the system's own `ping`, read back.
pub fn ping(host: &str) -> Option<Probe> {
    let said = run("ping.exe", &["-n", "4", host])?;
    Some(parse_probe(&said, 4))
}

/// A `ping`'s whole output, read: how many replies came (their `TTL=`, kept in
/// ASCII in either tongue) and the mean of their `=…ms`.
pub fn parse_probe(said: &str, sent: u32) -> Probe {
    let back = said.matches("TTL=").count() as u32;
    let mut times = said
        .split(|c| c == ' ' || c == ',' || c == '\n' || c == '=' || c == ':')
        .filter_map(|word| {
            word.strip_suffix("ms")
                .and_then(|ms| ms.parse::<u32>().ok())
        })
        .collect::<Vec<_>>();
    times.sort_unstable();
    let mean = (!times.is_empty()).then(|| times.iter().sum::<u32>() / times.len() as u32);
    Probe { back, sent, mean }
}

/// A command run with no window of its own, its output taken as it comes. The
/// bytes are read past the odd character rather than failed, for `ping` speaks
/// the console's code page, whose numbers and `TTL=` survive.
fn run(program: &str, args: &[&str]) -> Option<String> {
    use std::os::windows::process::CommandExt;
    let out = Command::new(program)
        .args(args)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A timezone the clock may be set to: Windows' own identifier (what `tzutil`
/// takes and gives back), and the name to show for it in either tongue. A
/// honest short list of the zones people actually reach for, not the whole
/// registry.
pub const ZONES: &[(&str, &str, &str)] = &[
    ("UTC", "协调世界时", "Coordinated Universal"),
    (
        "China Standard Time",
        "北京 · 上海 · 乌鲁木齐",
        "Beijing · Shanghai · Ürümqi",
    ),
    ("Hong Kong SAR Standard Time", "中国香港", "Hong Kong SAR"),
    ("Taiwan Standard Time", "中国台北", "Taipei"),
    ("Tokyo Standard Time", "东京", "Tokyo"),
    ("Korea Standard Time", "首尔", "Seoul"),
    ("Singapore Standard Time", "新加坡", "Singapore"),
    ("India Standard Time", "加尔各答", "Kolkata"),
    ("Arabian Standard Time", "迪拜", "Dubai"),
    ("Russian Standard Time", "莫斯科", "Moscow"),
    (
        "W. Europe Standard Time",
        "柏林 · 巴黎 · 阿姆斯特丹",
        "Berlin · Paris · Amsterdam",
    ),
    (
        "Central European Standard Time",
        "华沙 · 布拉格",
        "Warsaw · Prague",
    ),
    (
        "Eastern Europe Standard Time",
        "基辅 · 布加勒斯特",
        "Kyiv · Bucharest",
    ),
    ("GMT Standard Time", "伦敦 · 都柏林", "London · Dublin"),
    ("Pacific Standard Time", "洛杉矶", "Los Angeles"),
    ("Mountain Standard Time", "丹佛", "Denver"),
    ("Central Standard Time", "芝加哥", "Chicago"),
    ("Eastern Standard Time", "纽约", "New York"),
    (
        "AUS Eastern Standard Time",
        "悉尼 · 墨尔本",
        "Sydney · Melbourne",
    ),
    ("New Zealand Standard Time", "奥克兰", "Auckland"),
];

/// The clock moved to a timezone through `tzutil /s`. It wants the privilege to
/// change the system time, so it is honest about failing when run unraised.
pub fn set_time_zone(id: &str) -> bool {
    ok("tzutil.exe", &["/s", id])
}

/// The clock corrected against its time source through the time service. This
/// too needs the service and the privilege, and says so when it could not.
pub fn sync_clock() -> bool {
    ok("w32tm.exe", &["/resync"])
}

/// The DNS cache cleared, through the system's own `ipconfig`. No window, and
/// whether it took is told back so the page never claims a flush that failed.
pub fn flush_dns() -> bool {
    ok("ipconfig.exe", &["/flushdns"])
}

/// A command run for its answer rather than its output: whether it exited clean.
fn ok(program: &str, args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    Command::new(program)
        .args(args)
        .creation_flags(0x0800_0000)
        .status()
        .map(|code| code.success())
        .unwrap_or(false)
}

/// A whole command line run raised to the administrator, the console kept out of
/// sight (see `elevation`'s own `runas`): `cmd.exe` handed `/c <line>`. Whether it
/// was started at all — the user may turn the ask down, which is told back so the
/// page never claims a change it did not make. What the command then did is read
/// back from the system afresh, not from this answer.
fn raised(line: &str) -> bool {
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    let params = format!("/c {line}");
    let started = unsafe {
        ShellExecuteW(
            None,
            w!("runas"),
            w!("cmd.exe"),
            &HSTRING::from(params),
            PCWSTR::null(),
            SW_HIDE,
        )
    };
    // ShellExecute reports success with a value above 32; a turned-down ask is 1223.
    started.0 as usize > 32
}

/// `alias`'s interface, quoted as `netsh` wants it.
fn iface(alias: &str) -> String {
    format!("name=\"{alias}\"")
}

/// The numbers `alias` reaches by, set by hand: its address, mask, and gateway
/// (none given, none set). Raised, so it may take. Whether the ask went through.
pub fn set_address(alias: &str, ip: &str, mask: &str, gateway: Option<&str>) -> bool {
    let gateway = gateway.filter(|said| !said.is_empty());
    let line = match gateway {
        Some(said) => format!(
            "netsh interface ip set address {} static {ip} {mask} {said}",
            iface(alias)
        ),
        None => format!(
            "netsh interface ip set address {} static {ip} {mask}",
            iface(alias)
        ),
    };
    raised(&line)
}

/// `alias` told to take its numbers automatically again: DHCP for both its
/// address and its name servers. Raised, so it may take.
pub fn set_dhcp(alias: &str) -> bool {
    let line = format!(
        "netsh interface ip set address {} dhcp & netsh interface ip set dns {} dhcp",
        iface(alias),
        iface(alias)
    );
    raised(&line)
}

/// The name servers `alias` looks through, set by hand: the first, then the
/// second where one is given. Any address it is reached by is left as it was.
/// Raised, so it may take.
pub fn set_dns(alias: &str, first: &str, second: Option<&str>) -> bool {
    let second = second.filter(|said| !said.is_empty());
    let line = match second {
        Some(said) => format!(
            "netsh interface ip set dns {} static {first} & netsh interface ip add dns {} {said} index=2",
            iface(alias),
            iface(alias)
        ),
        None => format!(
            "netsh interface ip set dns {} static {first}",
            iface(alias)
        ),
    };
    raised(&line)
}

/// `alias`'s lease let go and asked for again, through `ipconfig` raised. Whether
/// the ask went through.
pub fn renew(alias: &str) -> bool {
    let name = iface(alias);
    let quoted = format!("\"{alias}\"");
    raised(&format!(
        "netsh interface ip set address {name} dhcp & netsh interface ip set dns {name} dhcp & ipconfig /release {quoted} & ipconfig /renew {quoted}"
    ))
}

/// Windows' own reset of the whole network stack, raised: `internet reset` then
/// `internet reset ipv6`, and the machine told to restart to let it take. Kept
/// for the one button that asks first.
pub fn winsock_reset() -> bool {
    raised("netsh int ip reset & netsh winsock reset")
}

/// One line of `netstat`: its protocol, its two ends, its state, and the process
/// holding it.
#[derive(Clone)]
pub struct Conn {
    pub proto: String,
    pub local: String,
    pub foreign: String,
    pub state: String,
    pub pid: u32,
}

/// The connections open now, read through the system's own `netstat` (no
/// privilege it needs): TCP's established and listening, and UDP's in use, each
/// as a line. Its header and blank lines are passed over.
pub fn connections() -> Vec<Conn> {
    let Some(said) = run("netstat.exe", &["-ano"]) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in said.lines() {
        let mut cols = line.split_whitespace();
        let (Some(proto), Some(local)) = (cols.next(), cols.next()) else {
            continue;
        };
        let foreign = cols.next().unwrap_or("—").to_string();
        // UDP lines carry no state; TCP's last word is the pid, its before that
        // the state.
        let rest: Vec<&str> = cols.collect();
        let pid = rest.last().and_then(|said| said.parse().ok()).unwrap_or(0);
        let state = if proto.eq_ignore_ascii_case("udp") {
            String::new()
        } else {
            rest.get(rest.len().saturating_sub(2))
                .map_or_else(String::new, |said| said.to_string())
        };
        // The established few first are what a person looks for; listening kept
        // too, for the local port a service holds.
        out.push(Conn {
            proto: proto.to_uppercase(),
            local: local.to_string(),
            foreign,
            state,
            pid,
        });
    }
    out.sort_by_key(|conn| {
        // Established, then listening, then the rest; each in the order read.
        match conn.state.as_str() {
            "ESTABLISHED" => 0,
            "LISTENING" => 1,
            "" => 2,
            _ => 3,
        }
    });
    out
}

/// The user's and the system's temporary folders.
fn temp_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Some(temp) = std::env::var_os("TEMP") {
        dirs.push(std::path::PathBuf::from(temp));
    }
    if let Some(windir) = std::env::var_os("SystemRoot") {
        dirs.push(std::path::Path::new(&windir).join("Temp"));
    }
    dirs
}

/// The room the temporary files take. A file that cannot be looked at (no
/// leave, or gone as we asked) is walked past.
pub fn temp_bytes() -> u64 {
    temp_dirs().iter().map(|dir| measured(dir)).sum()
}

/// The bytes under a folder, its subfolders taken one by one off a stack so a
/// deep or looping tree cannot fill the call stack.
fn measured(dir: &std::path::Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(here) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&here) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total += meta.len();
            }
        }
    }
    total
}

/// The temporary files taken away as far as they can be: what is locked by a
/// running program is left, and the room freed is told back.
pub fn clean_temp() -> u64 {
    let mut gone = 0u64;
    for dir in temp_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let meta = match entry.metadata() {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            let went = if meta.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            if went.is_ok() {
                gone += meta.len();
            }
        }
    }
    gone
}

/// The recycle bin: the room it takes and the number of things in it, as the
/// shell keeps them across every drive together.
pub fn recycle() -> (u64, u64) {
    let mut bin = SHQUERYRBINFO {
        cbSize: size_of::<SHQUERYRBINFO>() as u32,
        i64Size: 0,
        i64NumItems: 0,
    };
    if unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut bin) }.is_err() {
        return (0, 0);
    }
    (bin.i64Size.max(0) as u64, bin.i64NumItems.max(0) as u64)
}

/// The recycle bin emptied, the whole way through, with no box and no sound of
/// its own — the page says what it did.
pub fn empty_recycle() -> bool {
    unsafe {
        SHEmptyRecycleBinW(
            None,
            PCWSTR::null(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        )
    }
    .is_ok()
}

/// The storage sense's own key, under the user.
const SENSE: &str =
    r"Software\Microsoft\Windows\CurrentVersion\StorageSense\Parameters\StoragePolicy";

/// Whether storage sense is on.
pub fn storage_sense() -> bool {
    word_at(SENSE, "01").unwrap_or(0) != 0
}

/// Storage sense written on or off, and the change sent round the desktop.
pub fn set_storage_sense(on: bool) {
    write_word(SENSE, "01", u32::from(on));
    broadcast("Environment");
}

/// A word read from the registry under the user.
fn word_at(key: &str, value: &str) -> Option<u32> {
    let (key, value) = (HSTRING::from(key), HSTRING::from(value));
    let mut data = 0u32;
    let mut size = 4u32;
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &key,
            &value,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut _ as *mut _),
            Some(&mut size),
        )
    };
    read.is_ok().then_some(data)
}

/// A word written under the user, the key made if it is not there.
fn write_word(key: &str, value: &str, data: u32) {
    unsafe {
        let mut hkey = HKEY::default();
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(key),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        )
        .is_ok()
        {
            let _ = RegSetValueExW(
                hkey,
                &HSTRING::from(value),
                None,
                REG_DWORD,
                Some(&data.to_le_bytes()),
            );
            let _ = RegCloseKey(hkey);
        }
    }
}

/// A word sent round the desktop so the shell follows what was just written.
fn broadcast(section: &str) {
    let word: Vec<u16> = section.encode_utf16().chain([0]).collect();
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(word.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            200,
            None,
        );
    }
}

/// A Bluetooth device the radio has met: the name it was given, its address as
/// the six groups of hex people read them, and whether it is remembered (paired)
/// and now connected.
#[derive(Clone)]
pub struct Device {
    pub name: String,
    pub address: String,
    pub paired: bool,
    pub connected: bool,
}

/// The names of the Bluetooth radios this PC carries — empty when there is none,
/// so the page can say honestly there is no radio to manage.
pub fn radios() -> Vec<String> {
    let mut found = Vec::new();
    unsafe {
        let params = BLUETOOTH_FIND_RADIO_PARAMS {
            dwSize: size_of::<BLUETOOTH_FIND_RADIO_PARAMS>() as u32,
        };
        let mut radio = HANDLE(std::ptr::null_mut());
        let Ok(find) = BluetoothFindFirstRadio(&params, &mut radio) else {
            return found;
        };
        loop {
            let mut info: BLUETOOTH_RADIO_INFO = std::mem::zeroed();
            info.dwSize = size_of::<BLUETOOTH_RADIO_INFO>() as u32;
            if BluetoothGetRadioInfo(radio, &mut info) == ERROR_SUCCESS {
                let name = wide(&info.szName);
                if !name.is_empty() {
                    found.push(name);
                }
            }
            // The radio's own handle is ours to close before the next takes it.
            let _ = CloseHandle(radio);
            if BluetoothFindNextRadio(find, &mut radio).is_err() {
                break;
            }
        }
        let _ = BluetoothFindRadioClose(find);
    }
    found
}

/// Whether the radio is now open to being discovered by other devices.
pub fn discoverable() -> bool {
    unsafe { BluetoothIsDiscoverable(None).as_bool() }
}

/// The radio made discoverable, or shut to discovery; told back whether it took.
/// Passing no radio means the default one, the way the toggle in the shell works.
pub fn set_discoverable(on: bool) -> bool {
    unsafe { BluetoothEnableDiscovery(None, on).as_bool() }
}

/// The devices the radio has met — those paired, and any remembered — read from
/// its own book without an inquiry, so it is quick and asks nothing of the air.
pub fn remembered() -> Vec<Device> {
    let mut found = Vec::new();
    unsafe {
        let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
            dwSize: size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
            fReturnAuthenticated: BOOL::from(true),
            fReturnRemembered: BOOL::from(true),
            fReturnConnected: BOOL::from(true),
            fReturnUnknown: BOOL::from(false),
            fIssueInquiry: BOOL::from(false),
            cTimeoutMultiplier: 1,
            hRadio: HANDLE(std::ptr::null_mut()),
        };
        let mut info: BLUETOOTH_DEVICE_INFO = std::mem::zeroed();
        info.dwSize = size_of::<BLUETOOTH_DEVICE_INFO>() as u32;
        let Ok(find) = BluetoothFindFirstDevice(&params, &mut info) else {
            return found;
        };
        loop {
            let name = wide(&info.szName);
            if !name.is_empty() {
                found.push(Device {
                    name,
                    address: mac(&info.Address),
                    paired: info.fRemembered.as_bool(),
                    connected: info.fConnected.as_bool(),
                });
            }
            if BluetoothFindNextDevice(find, &mut info).is_err() {
                break;
            }
        }
        let _ = BluetoothFindDeviceClose(find);
    }
    found
}

/// The devices near the radio right now: an inquiry is asked of one radio for
/// about `units` × 1.28 seconds, so this blocks while it listens to the air and
/// is called off the UI thread. It brings back remembered devices and any new
/// one it hears, unnamed ones left out.
pub fn nearby(units: u8) -> Vec<Device> {
    let mut found = Vec::new();
    unsafe {
        // An inquiry must be put to a single radio, so take the first one's
        // handle; the find is closed at once, leaving the radio open for the ask.
        let rparams = BLUETOOTH_FIND_RADIO_PARAMS {
            dwSize: size_of::<BLUETOOTH_FIND_RADIO_PARAMS>() as u32,
        };
        let mut radio = HANDLE(std::ptr::null_mut());
        let Ok(rfind) = BluetoothFindFirstRadio(&rparams, &mut radio) else {
            return found;
        };
        let _ = BluetoothFindRadioClose(rfind);
        let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
            dwSize: size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
            fReturnAuthenticated: BOOL::from(true),
            fReturnRemembered: BOOL::from(true),
            fReturnConnected: BOOL::from(true),
            fReturnUnknown: BOOL::from(true),
            fIssueInquiry: BOOL::from(true),
            cTimeoutMultiplier: units,
            hRadio: radio,
        };
        let mut info: BLUETOOTH_DEVICE_INFO = std::mem::zeroed();
        info.dwSize = size_of::<BLUETOOTH_DEVICE_INFO>() as u32;
        if let Ok(find) = BluetoothFindFirstDevice(&params, &mut info) {
            loop {
                let name = wide(&info.szName);
                if !name.is_empty() {
                    found.push(Device {
                        name,
                        address: mac(&info.Address),
                        paired: info.fRemembered.as_bool(),
                        connected: info.fConnected.as_bool(),
                    });
                }
                if BluetoothFindNextDevice(find, &mut info).is_err() {
                    break;
                }
            }
            let _ = BluetoothFindDeviceClose(find);
        }
        let _ = CloseHandle(radio);
    }
    found
}

/// The instance id of the machine's Bluetooth adapter — the radio hardware
/// itself, not a device behind it. Enumerated through the Bluetooth device class
/// (`{e0cbf06c-…}`), which `SetupAPI` reports even while the adapter is disabled;
/// the first id that is not a `BTHENUM\` (a device the radio has met) is the
/// adapter. None when the PC carries no such hardware.
fn adapter_instance() -> Option<String> {
    use windows::core::GUID;
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
        SetupDiGetDeviceInstanceIdW, DIGCF_PRESENT, SP_DEVINFO_DATA,
    };
    let class = GUID::from_u128(0xe0cb_f06c_cd8b_4647_bb8a_263b_4e17_f2fc);
    unsafe {
        let Ok(set) = SetupDiGetClassDevsW(Some(&class), PCWSTR::null(), None, DIGCF_PRESENT)
        else {
            return None;
        };
        let mut out = None;
        for index in 0.. {
            let mut data = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };
            if SetupDiEnumDeviceInfo(set, index, &mut data).is_err() {
                break;
            }
            let mut need = 0u32;
            let _ = SetupDiGetDeviceInstanceIdW(set, &data, None, Some(&mut need));
            if need == 0 {
                continue;
            }
            let mut buf = vec![0u16; need as usize];
            if SetupDiGetDeviceInstanceIdW(set, &data, Some(&mut buf), None).is_err() {
                continue;
            }
            let id = wide(&buf);
            if !id.to_uppercase().starts_with("BTHENUM\\") {
                out = Some(id);
                break;
            }
        }
        let _ = SetupDiDestroyDeviceInfoList(set);
        out
    }
}

/// Whether the adapter is now switched on: a live radio is listed. When the
/// adapter is disabled there is no radio to find, so this reads it truly.
pub fn powered() -> bool {
    !radios().is_empty()
}

/// Whether the machine has Bluetooth hardware to switch at all.
pub fn has_adapter() -> bool {
    adapter_instance().is_some()
}

/// Switch the adapter on or off — a real power, not just discovery. Raised to the
/// administrator, since enabling or disabling a device wants the privilege; told
/// back whether the ask was started, and the page reads the radio again to see what
/// truly took.
pub fn set_power(on: bool) -> bool {
    let Some(id) = adapter_instance() else {
        return false;
    };
    let verb = if on {
        "/enable-device"
    } else {
        "/disable-device"
    };
    raised(&format!("pnputil {verb} \"{id}\""))
}

/// Pair with a device by opening the system's own Bluetooth wizard, which asks
/// whatever that device wants (a passkey, a confirmation, nothing) and then
/// connects it. `address` is the six hex groups as the page shows them; left unpaired
/// if it does not parse. The dialog belongs to Windows, run on the UI thread.
pub fn pair(address: &str, name: &str) -> bool {
    let bytes: Vec<u8> = address
        .split(':')
        .filter_map(|part| u8::from_str_radix(part, 16).ok())
        .collect();
    if bytes.len() != 6 {
        return false;
    }
    let mut info: BLUETOOTH_DEVICE_INFO = unsafe { std::mem::zeroed() };
    info.dwSize = size_of::<BLUETOOTH_DEVICE_INFO>() as u32;
    unsafe {
        let at = info.Address.Anonymous.rgBytes.as_mut_ptr();
        for i in 0..6 {
            *at.add(5 - i) = bytes[i];
        }
    }
    let name_wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
    let len = name_wide.len().min(info.szName.len());
    info.szName[..len].copy_from_slice(&name_wide[..len]);
    // 0 means the wizard ran and pairing took; any other code, it was refused or
    // cancelled. The dialog is the system's own, run on the UI thread.
    unsafe { BluetoothAuthenticateDevice(None, None, &mut info, None) == 0 }
}

/// A name held as Windows holds it — a run of UTF-16 cut at its first zero.
fn wide(name: &[u16]) -> String {
    let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    String::from_utf16_lossy(&name[..end]).trim().to_string()
}

/// A Bluetooth address as its six groups, most significant first — the bytes are
/// kept the other way round, so they are turned about to be read.
fn mac(address: &BLUETOOTH_ADDRESS) -> String {
    let b = unsafe { address.Anonymous.rgBytes };
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        b[5], b[4], b[3], b[2], b[1], b[0]
    )
}

/// A mode a display can be told to wear: its size, and how often it is drawn.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Mode {
    pub wide: u32,
    pub tall: u32,
    pub hz: u32,
}

/// The GDI name of the screen standing at this rectangle (`\\.\DISPLAY1`);
/// empty where no screen is. The name is what the modes are asked of, and
/// what a change is put to.
pub fn device_of(monitor: &RECT) -> String {
    unsafe {
        let at = MonitorFromRect(monitor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(at, &mut info as *mut _ as *mut MONITORINFO).as_bool() {
            return String::new();
        }
        wide(&info.szDevice)
    }
}

/// A device name as the wide calls want it: UTF-16 with its end marked.
fn named(device: &str) -> Vec<u16> {
    let mut name: Vec<u16> = device.encode_utf16().collect();
    name.push(0);
    name
}

/// The resolutions a display offers, each kept once with the fastest refresh
/// it said for that one — the list Mac's panel drops down.
pub fn modes(device: &str) -> Vec<Mode> {
    let mut found = Vec::new();
    if device.is_empty() {
        return found;
    }
    let name = named(device);
    let mut index = 0u32;
    loop {
        let mut dm = DEVMODEW::default();
        dm.dmSize = size_of::<DEVMODEW>() as u16;
        // (A driver that will not end its list is given a head of it.)
        if index >= 512
            || !(unsafe {
                EnumDisplaySettingsW(
                    PCWSTR(name.as_ptr()),
                    ENUM_DISPLAY_SETTINGS_MODE(index),
                    &mut dm,
                )
            })
            .as_bool()
        {
            break;
        }
        found.push(Mode {
            wide: dm.dmPelsWidth,
            tall: dm.dmPelsHeight,
            hz: dm.dmDisplayFrequency,
        });
        index += 1;
    }
    folded(found)
}

/// Each resolution but once, at the fastest rate seen of it, largest first.
fn folded(all: Vec<Mode>) -> Vec<Mode> {
    let mut kept: Vec<Mode> = Vec::new();
    for mode in all {
        match kept
            .iter_mut()
            .find(|held| held.wide == mode.wide && held.tall == mode.tall)
        {
            Some(held) if mode.hz > held.hz => held.hz = mode.hz,
            Some(_) => {}
            None => kept.push(mode),
        }
    }
    kept.sort_by(|a, b| (b.wide * b.tall).cmp(&(a.wide * a.tall)));
    kept
}

/// The mode a display wears now.
pub fn current(device: &str) -> Option<Mode> {
    if device.is_empty() {
        return None;
    }
    let name = named(device);
    let mut dm = DEVMODEW::default();
    dm.dmSize = size_of::<DEVMODEW>() as u16;
    unsafe { EnumDisplaySettingsW(PCWSTR(name.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm) }
        .as_bool()
        .then(|| Mode {
            wide: dm.dmPelsWidth,
            tall: dm.dmPelsHeight,
            hz: dm.dmDisplayFrequency,
        })
}

/// Tells a display to wear a mode, and keeps the change in the registry as
/// Windows' own panel does: true when it took. A mode the driver will not
/// draw is refused, and the display is left as it was.
pub fn set_mode(device: &str, mode: Mode) -> bool {
    if device.is_empty() {
        return false;
    }
    let name = named(device);
    let mut dm = DEVMODEW::default();
    dm.dmSize = size_of::<DEVMODEW>() as u16;
    dm.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
    dm.dmPelsWidth = mode.wide;
    dm.dmPelsHeight = mode.tall;
    dm.dmDisplayFrequency = mode.hz;
    let took = unsafe {
        ChangeDisplaySettingsExW(
            PCWSTR(name.as_ptr()),
            Some(&dm as *const _),
            None,
            CDS_UPDATEREGISTRY,
            None,
        )
    };
    took == DISP_CHANGE_SUCCESSFUL
}

/// An update the system says it has put on: what it is called, its number, and
/// the day it went in. Read through Windows' own `Get-CimInstance`, for the
/// list lives in the WMI store rather than in one registry key.
#[derive(Clone)]
pub struct Patch {
    pub when: String,
    pub id: String,
    pub what: String,
}

/// The updates installed, newest first, as far as the list goes. The query
/// takes a few seconds off the machine, so the page asks for it in the background
/// rather than waiting at the window; saying nothing is honest when it fails.
pub fn patches() -> Vec<Patch> {
    let query = "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
                 Get-CimInstance Win32_QuickFixEngineering | Sort-Object InstalledOn -Descending | \
                 ForEach-Object { '{0}|{1}|{2}' -f ($_.InstalledOn -as [datetime]).ToString('yyyy-MM-dd'), $_.HotFixID, $_.Description }";
    let said = match run("powershell.exe", &["-NoProfile", "-Command", query]) {
        Some(said) => said,
        None => return Vec::new(),
    };
    told(&said)
}

/// The rows the query gave back, taken three fields to a line; a line that
/// says less than that is dropped rather than half-shown.
fn told(said: &str) -> Vec<Patch> {
    let mut found = Vec::new();
    for line in said.lines() {
        let bits = line.splitn(3, '|').collect::<Vec<_>>();
        if bits.len() < 3 || bits[1].is_empty() {
            continue;
        }
        found.push(Patch {
            when: bits[0].trim().to_string(),
            id: bits[1].trim().to_string(),
            what: bits[2].trim().to_string(),
        });
    }
    found
}

/// Windows' own licence, as the machine's licensing service keeps it: how
/// the system stands activated, the channel it came in on, and the last few
/// of its product key.
#[derive(Clone, Debug, PartialEq)]
pub struct License {
    /// One of `licensed`, `trial`, `notification`, `partial`, `unlicensed`.
    pub state: &'static str,
    /// Where the licence came from: `Retail`, `Volume`, `OEM` — as its own
    /// name says. Empty where the name does not tell.
    pub channel: String,
    /// The part of the key Windows shows; empty on a volume licence.
    pub key: String,
}

/// The licence as the licensing service reports it, through the machine's
/// own query. It takes a second or two, so the page asks in the background
/// (as it does the installed updates); saying nothing is honest when it
/// fails. The filter keeps the Windows channel alone, of all the products
/// the service lists.
pub fn licensing() -> Option<License> {
    let query = "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
                 Get-CimInstance -ClassName SoftwareLicensingProduct -Filter \
                 \"ApplicationID='55c92734-d682-4d71-912e-4312df99c78b'\" | \
                 ForEach-Object { '{0}|{1}|{2}' -f $_.LicenseStatus, $_.PartialProductKey, $_.Name }";
    let said = run("powershell.exe", &["-NoProfile", "-Command", query])?;
    licence(&said)
}

/// The rows the query gave back, one licence to a line: the activated one
/// first where several channels lie on the machine (an OEM key beside a
/// retail one), else the first that says anything of itself.
fn licence(said: &str) -> Option<License> {
    let mut fallback = None;
    for line in said.lines() {
        let bits = line.splitn(3, '|').collect::<Vec<_>>();
        if bits.len() < 3 {
            continue;
        }
        let Ok(status) = bits[0].trim().parse::<u32>() else {
            continue;
        };
        let found = License {
            state: match status {
                1 => "licensed",
                5 => "trial",
                4 => "partial",
                2 | 6 => "notification",
                _ => "unlicensed",
            },
            // (The name reads "Windows 11 Pro, Retail edition" or "…,
            // OEM_DM channel": what the comma leaves, cut at its first
            // underbar or space, is the channel's own word.)
            channel: bits[2]
                .rsplit(',')
                .next()
                .unwrap_or("")
                .trim()
                .split(['_', ' '])
                .next()
                .unwrap_or("")
                .to_string(),
            key: bits[1].trim().to_string(),
        };
        if found.state == "licensed" {
            return Some(found);
        }
        if fallback.is_none() && (status != 0 || !found.key.is_empty()) {
            fallback = Some(found);
        }
    }
    fallback
}

/// How long the display is waited on before it goes dark, and how long before
/// the machine sleeps — on the mains and on the battery, in seconds; nothing
/// when Windows has no answer for one of them. `powercfg` says its figures as
/// `0x` indexes, the mains one and the battery's last of all.
pub fn idle_times(video: bool) -> (Option<u32>, Option<u32>) {
    let (group, setting) = if video {
        ("SUB_VIDEO", "VIDEOIDLE")
    } else {
        ("SUB_SLEEP", "STANDBYIDLE")
    };
    let said = match run("powercfg.exe", &["/q", "SCHEME_CURRENT", group, setting]) {
        Some(said) => said,
        None => return (None, None),
    };
    let all = indexes(&said);
    let n = all.len();
    if n < 2 {
        return (None, None);
    }
    (Some(all[n - 2]), Some(all[n - 1]))
}

/// The `0x` figures `powercfg`'s answer carries, in the order it says them.
fn indexes(said: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for line in said.lines() {
        if let Some(at) = line.rfind("0x") {
            let digits = line[at + 2..]
                .chars()
                .take_while(|c| c.is_ascii_hexdigit())
                .collect::<String>();
            if let Some(num) = u32::from_str_radix(&digits, 16).ok() {
                out.push(num);
            }
        }
    }
    out
}

/// The waiting told to Windows anew, for both the mains and the battery — zero
/// meaning it is never come to — and the plan made live so it takes effect at
/// once. Whether all of it went in clean is told back.
pub fn set_idle(video: bool, seconds: u32) -> bool {
    let (group, setting) = if video {
        ("SUB_VIDEO", "VIDEOIDLE")
    } else {
        ("SUB_SLEEP", "STANDBYIDLE")
    };
    let said = seconds.to_string();
    let took = ok(
        "powercfg.exe",
        &["/setacvalueindex", "SCHEME_CURRENT", group, setting, &said],
    ) && ok(
        "powercfg.exe",
        &["/setdcvalueindex", "SCHEME_CURRENT", group, setting, &said],
    );
    took && ok("powercfg.exe", &["/setactive", "SCHEME_CURRENT"])
}

/// The warmth the night light lends the screen: none, or three depths of amber.
/// Kept in the running program only — the gamma is worn while the display
/// draws, and a mode change or a reboot washes it off by itself.
static NIGHT: AtomicU8 = AtomicU8::new(0);

/// How warm the screen is worn now, as this running Glance put it on.
pub fn night() -> u32 {
    NIGHT.load(Ordering::Relaxed) as u32
}

/// The gamma a depth asks for: the reds left whole while the greens and blues
/// are drawn down, which is the amber of a screen at night.
fn night_ramp(level: u32) -> [[u16; 3]; 128] {
    let (green, blue) = match level {
        1 => (0.92, 0.80),
        2 => (0.86, 0.68),
        _ if level >= 3 => (0.80, 0.56),
        _ => (1.0, 1.0),
    };
    let mut ramp = [[0u16; 3]; 128];
    for (at, row) in ramp.iter_mut().enumerate() {
        let step = (at as u32 * 65535 / 127) as u16;
        row[0] = step;
        row[1] = (step as f32 * green).min(65535.0) as u16;
        row[2] = (step as f32 * blue).min(65535.0) as u16;
    }
    ramp
}

/// The warmth put on the display's own drawing, told back whether it took.
/// The plain ramp (none) is what a fresh login wears, so the way off is to
/// write the straight line back.
pub fn set_night(level: u32) -> bool {
    let level = level.min(3);
    let ramp = night_ramp(level);
    let screen = unsafe { GetDC(None) };
    let gave = unsafe { SetDeviceGammaRamp(screen, &ramp as *const _ as *const core::ffi::c_void) };
    unsafe {
        let _ = ReleaseDC(None, screen);
    }
    let took = gave.as_bool();
    if took {
        NIGHT.store(level as u8, Ordering::Relaxed);
    }
    took
}

/// How long the machine has been up since, in seconds — the tick Windows
/// counts from its own starting.
pub fn uptime() -> u64 {
    (unsafe { GetTickCount64() }) / 1000
}

/// How much of its designed capacity the battery can still hold, as its own
/// report tells: `powercfg` writes one to a temporary file — which an ordinary
/// login may — and the two figures are weighed. Nothing when the machine keeps
/// no battery or the report says nothing of one.
pub fn health() -> Option<u32> {
    let script = "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
                  $f=Join-Path $env:TEMP 'glance-battery.xml'; \
                  powercfg /batteryreport /output $f /duration 1 | Out-Null; \
                  $x=[xml](Get-Content $f -Raw); Remove-Item $f -ErrorAction SilentlyContinue; \
                  '{0}|{1}' -f $x.BatteryReport.Battery.DesignCapacity, \
                  $x.BatteryReport.Battery.FullChargedCapacity";
    let said = run("powershell.exe", &["-NoProfile", "-Command", script])?;
    health_of(&said)
}

/// The two figures the report gave back, weighed into a percent kept whole.
fn health_of(said: &str) -> Option<u32> {
    let bits = said.trim().splitn(2, '|').collect::<Vec<_>>();
    if bits.len() != 2 {
        return None;
    }
    let design: f64 = bits[0].trim().parse().ok()?;
    let full: f64 = bits[1].trim().parse().ok()?;
    if design <= 0.0 || full <= 0.0 {
        return None;
    }
    Some(((full / design * 100.0).round() as u32).min(100))
}

/// How far the text is scaled, as the system keeps it under the user; a hundred
/// is the plain size the machine was born with.
pub fn text_scale() -> u32 {
    word_at(r"Software\Microsoft\Accessibility", "TextScaleFactor").unwrap_or(100)
}

/// The text scaled to the share asked, and the change sent round the desktop.
/// Windows writes the same figure the Settings app does; how soon every program
/// follows is the system's own mood, so a page says it may wait for a re-login.
pub fn set_text_scale(share: u32) {
    write_word(
        r"Software\Microsoft\Accessibility",
        "TextScaleFactor",
        share.clamp(100, 225),
    );
    broadcast("AccessibilitySettings");
}

/// The accent the desktop wears, as the window manager keeps it: blue and green
/// and red shut in one word; nothing while the system was never told one.
pub fn accent() -> Option<u32> {
    word_at(r"Software\Microsoft\Windows\DWM", "AccentColor")
}

/// The accent written for the desktop and its menus alike, and the theme sent
/// round so the shell takes it up. Windows itself makes sure of a live redraw;
/// here the most a program may do is ask.
pub fn set_accent(rgb: u32) {
    let colour = rgb & 0x00ff_ffff;
    write_word(r"Software\Microsoft\Windows\DWM", "AccentColor", colour);
    write_word(r"Software\Microsoft\Windows\DWM", "AccentColorMenu", colour);
    broadcast("Themes");
}

/// How far the desktop is grown: the DPI Windows applied at the last log-in,
/// as it is kept under the user, told back as a share of the plain hundred.
pub fn scale() -> u32 {
    step_of(word_at(r"Control Panel\Desktop\WindowMetrics", "AppliedDPI").unwrap_or(96))
}

/// The offered growth nearest a figure — the steps are a quarter and a half
/// and three-quarters and whole again over the plain, and a figure between
/// them is read as the step it leans to.
fn step_of(dpi: u32) -> u32 {
    [100u32, 125, 150, 175, 200]
        .into_iter()
        .min_by_key(|step| (*step as i64 - dpi as i64 * 100 / 96).abs())
        .unwrap_or(100)
}

/// A screen of the desktop as it lies: the GDI name it is known by, where it
/// stands, and whether things start on it.
struct Watch {
    name: String,
    left: i32,
    top: i32,
    primary: bool,
}

/// Every screen Windows is drawing on, heard at once off the monitor list.
fn watchtowers() -> Vec<Watch> {
    unsafe extern "system" fn hear(monitor: HMONITOR, _: HDC, _: *mut RECT, found: LPARAM) -> BOOL {
        let found = unsafe { &mut *(found.0 as *mut Vec<Watch>) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(monitor, &mut info as *mut _ as *mut MONITORINFO) }.as_bool() {
            let at = info.monitorInfo.rcMonitor;
            found.push(Watch {
                name: wide(&info.szDevice),
                left: at.left,
                top: at.top,
                // MONITORINFOF_PRIMARY, which the crate does not name.
                primary: info.monitorInfo.dwFlags & 1 != 0,
            });
        }
        BOOL::from(true)
    }
    let mut found = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(hear),
            LPARAM(&mut found as *mut _ as isize),
        );
    }
    found
}

/// A screen's own current mode retold with a new place on the desktop. The
/// flags say whether the change lands at once or waits staged for a flush.
fn placed(device: &str, x: i32, y: i32, flags: CDS_TYPE) -> bool {
    let name = named(device);
    let mut dm = DEVMODEW::default();
    dm.dmSize = size_of::<DEVMODEW>() as u16;
    if !(unsafe { EnumDisplaySettingsW(PCWSTR(name.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm) })
        .as_bool()
    {
        return false;
    }
    dm.dmFields = dm.dmFields | DM_POSITION;
    dm.Anonymous1.Anonymous2.dmPosition = POINTL { x, y };
    unsafe {
        ChangeDisplaySettingsExW(
            PCWSTR(name.as_ptr()),
            Some(&dm as *const _),
            None,
            flags,
            None,
        ) == DISP_CHANGE_SUCCESSFUL
    }
}

/// Crown another screen as the one things start on. The desktop is shifted
/// whole — every screen keeps the place it had against the others, and the
/// chosen one is carried to the origin wearing `CDS_SET_PRIMARY`, which is
/// how the old panel itself moved the crown — the change staged, flushed,
/// and then read back: true only when the origin now lies on that screen.
pub fn set_primary(device: &str) -> bool {
    let board = watchtowers();
    let Some(chosen) = board.iter().find(|one| one.name == device) else {
        return false;
    };
    if chosen.primary {
        return true;
    }
    for one in &board {
        let took = if one.name == device {
            placed(
                &one.name,
                0,
                0,
                CDS_UPDATEREGISTRY | CDS_SET_PRIMARY | CDS_NORESET,
            )
        } else {
            placed(
                &one.name,
                one.left - chosen.left,
                one.top - chosen.top,
                CDS_UPDATEREGISTRY | CDS_NORESET,
            )
        };
        // (A screen the driver would not move is walked past: the flush lands
        // what was staged, and the read-back below is the only telling.)
        let _ = took;
    }
    unsafe {
        let _ = ChangeDisplaySettingsExW(PCWSTR::null(), None, None, CDS_RESET, None);
    }
    watchtowers()
        .into_iter()
        .find(|one| one.primary)
        .is_some_and(|one| one.name == device)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_two_last_indexes_are_the_mains_and_the_battery() {
        let said = "电源设置 GUID: 7516b74f-8dfc-4695-9575-4aca2f3e48b8 (显示器关闭)\n\
                    设置的最小可能值: 0x00000000\n\
                    设置的最大可能值: 0xffffffff\n\
                    当前交流电源设置索引: 0x00000078\n\
                    当前直流电源设置索引: 0x0000002d\n";
        let all = super::indexes(said);
        assert_eq!(all.len(), 4);
        assert_eq!(all[all.len() - 2], 120);
        assert_eq!(all[all.len() - 1], 45);
    }

    #[test]
    fn an_update_row_is_three_sayings_and_a_number() {
        let rows =
            super::told("2026-01-14|KB5051974|Security Update\n\nnope|\n2026-02-02|KB1|A | B\n");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "KB5051974");
        assert_eq!(rows[0].when, "2026-01-14");
        // (The third field keeps every bar after its own two: descriptions do not have them.)
        assert_eq!(rows[1].what, "A | B");
    }

    #[test]
    fn each_resolution_is_kept_once_at_its_fastest() {
        let all = vec![
            super::Mode {
                wide: 1920,
                tall: 1080,
                hz: 60,
            },
            super::Mode {
                wide: 1280,
                tall: 720,
                hz: 50,
            },
            super::Mode {
                wide: 1920,
                tall: 1080,
                hz: 144,
            },
            super::Mode {
                wide: 3840,
                tall: 2160,
                hz: 30,
            },
        ];
        let kept = super::folded(all);
        // (Largest first, and 1080p once — at the rate it can be told to run.)
        assert_eq!(kept.len(), 3);
        assert_eq!((kept[0].wide, kept[0].tall), (3840, 2160));
        assert_eq!(kept[1].hz, 144);
    }

    #[test]
    fn the_probe_counts_its_replies_and_mean() {
        // (A reply's line, as `ping` says it in either tongue: `TTL=` and a
        // `=…ms`, both kept in ASCII.)
        let english = "Pinging 1.1.1.1 with 32 bytes: Reply: bytes=32 time=12ms TTL=51 \
                       Reply: bytes=32 time=18ms TTL=51 Request timed out.";
        let probe = super::parse_probe(english, 4);
        assert_eq!(probe.back, 2);
        assert_eq!(probe.mean, Some(15));
    }

    #[test]
    fn the_battery_report_weighs_into_a_percent() {
        assert_eq!(super::health_of("50000|42500"), Some(85));
        assert_eq!(super::health_of(" 4440 | 4440 \n"), Some(100));
        // (A machine with no battery says nothing of one: empty figures, or no
        // second figure at all — and either way there is no percent to show.)
        assert_eq!(super::health_of(" | "), None);
        assert_eq!(super::health_of("50000"), None);
        assert_eq!(super::health_of("0|10"), None);
    }

    #[test]
    fn the_night_ramp_keeps_reds_and_draws_down_the_rest() {
        let plain = super::night_ramp(0);
        // (None: every colour keeps the same straight line.)
        assert_eq!(plain[64][0], plain[64][1]);
        assert_eq!(plain[64][1], plain[64][2]);
        assert_eq!(plain[127][0], 65535);
        assert_eq!(plain[127][1], 65535);
        let warm = super::night_ramp(3);
        // (The top of red stays whole; green and blue fall under it.)
        assert_eq!(warm[127][0], 65535);
        assert!((warm[127][1] as u32) < 65535);
        assert!((warm[127][2] as u32) < warm[127][1] as u32);
    }

    #[test]
    fn a_dpi_is_read_as_the_growth_it_leans_to() {
        assert_eq!(super::step_of(96), 100);
        assert_eq!(super::step_of(120), 125);
        assert_eq!(super::step_of(144), 150);
        assert_eq!(super::step_of(168), 175);
        assert_eq!(super::step_of(192), 200);
        // (Between steps: the nearer one, and past the last its own.)
        assert_eq!(super::step_of(130), 125);
        assert_eq!(super::step_of(240), 200);
    }

    #[test]
    #[ignore]
    fn the_board_is_watched_and_one_screen_wears_the_crown() {
        // (Run by hand on a machine with screens on it: exactly one of the
        // board wears the crown, and names are the GDI ones the pages use.)
        let board = super::watchtowers();
        assert!(!board.is_empty());
        assert_eq!(board.iter().filter(|one| one.primary).count(), 1);
        assert!(board.iter().all(|one| !one.name.is_empty()));
    }

    #[test]
    fn the_licence_takes_the_activated_channel_and_its_own_words() {
        let got = |said: &str| super::licence(said);
        // (Two channels lie on the machine: the activated one answers first,
        // wherever it stands in the list.)
        let two = "0||Windows 11 Pro, Volume_KMSCLIENT channel\n\
                   1|8HRB2|Windows 11 IoT Enterprise, Retail edition\n";
        assert_eq!(
            got(two),
            Some(super::License {
                state: "licensed",
                channel: "Retail".to_string(),
                key: "8HRB2".to_string(),
            })
        );
        // (None activated: the first that says anything of itself is kept.)
        let cold = "0||Windows 11 Pro, OEM_DM channel\n5|XXXXX|Windows 11 Pro, OEM channel\n";
        assert_eq!(
            got(cold).map(|l| (l.state, l.channel, l.key)),
            Some(("trial", "OEM".to_string(), "XXXXX".to_string()))
        );
        // (Nothing of a licence, or nothing that reads like one.)
        assert_eq!(got("\n"), None);
        assert_eq!(got("nope"), None);
    }
}
