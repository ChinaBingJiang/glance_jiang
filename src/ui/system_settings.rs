//! The system settings' shared helpers: the OS-level reads and writes that the
//! old settings window uses for its own pages (registry, battery, input, theme,
//! privacy, wallpapers). The Mac-styled window that used to live here is gone —
//! the old window (`super::settings_window`) is now the sole host for every OS
//! category. These free functions remain available as `super::system_settings::X`.

use std::mem::size_of;

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{MonitorFromRect, MONITOR_DEFAULTTONEAREST};
use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetLogicalDrives};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ,
    RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::canvas::{Canvas, Color, Fill, Point};
use super::gfx::Frame;
use super::settings_window::fill;
use super::text::Lang;
use crate::metrics::reg_string;

// ---- Registry foundation ----

/// A string read from the registry; empty where it is not there.
pub(crate) fn reg_sz(root: HKEY, key: &str, value: &str) -> String {
    let (key, value) = (HSTRING::from(key), HSTRING::from(value));
    let mut data = [0u16; 512];
    let mut size = size_of_val(&data) as u32;
    let read = unsafe {
        RegGetValueW(
            root,
            &key,
            &value,
            RRF_RT_REG_SZ,
            None,
            Some(data.as_mut_ptr() as *mut _),
            Some(&mut size),
        )
    };
    if read.is_err() {
        return String::new();
    }
    let length = data.iter().position(|&c| c == 0).unwrap_or(data.len());
    String::from_utf16_lossy(&data[..length])
}

/// A word read from the registry; `None` where it is not there.
pub(crate) fn reg_word(root: HKEY, key: &str, value: &str) -> Option<u32> {
    let (key, value) = (HSTRING::from(key), HSTRING::from(value));
    let (mut data, mut size) = (0u32, 4u32);
    let read = unsafe {
        RegGetValueW(
            root,
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

/// A word written to the registry, under the user: the key made if it is not there.
pub(crate) fn reg_write(key: &str, value: &str, data: u32) {
    unsafe {
        let mut hkey = HKEY::default();
        let open = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(key),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if open.is_ok() {
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

/// A string written to the registry, under the user.
pub(crate) fn reg_write_sz(key: &str, value: &str, data: &str) {
    let wide: Vec<u8> = data
        .encode_utf16()
        .chain([0])
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    unsafe {
        let mut hkey = HKEY::default();
        let open = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(key),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if open.is_ok() {
            let _ = RegSetValueExW(hkey, &HSTRING::from(value), None, REG_SZ, Some(&wide));
            let _ = RegCloseKey(hkey);
        }
    }
}

/// A word sent round the desktop so that every window follows the setting just written.
pub(crate) fn broadcast_settings(section: &str) {
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

// ---- Timezone ----

/// The timezone Windows is in now, by its own identifier (what `tzutil` takes).
pub(crate) fn current_zone() -> String {
    reg_sz(
        HKEY_LOCAL_MACHINE,
        r"SYSTEM\CurrentControlSet\Control\TimeZoneInformation",
        "TimeZoneKeyName",
    )
}

// ---- Clock ----

pub(crate) fn clock_24() -> bool {
    reg_sz(
        HKEY_CURRENT_USER,
        r"Control Panel\International",
        "sTimeFormat",
    )
    .contains("HH")
}

pub(crate) fn clock_seconds() -> bool {
    reg_word(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        "ShowSecondsInSystemClock",
    )
    .unwrap_or(0)
        != 0
}

pub(crate) fn toggle_seconds() {
    let path = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
    reg_write(
        path,
        "ShowSecondsInSystemClock",
        u32::from(!clock_seconds()),
    );
    broadcast_settings("SubSystems");
}

pub(crate) fn toggle_clock24() {
    let now = clock_24();
    reg_write_sz(
        r"Control Panel\International",
        "sTimeFormat",
        if now { "h:mm:ss tt" } else { "HH:mm:ss" },
    );
    broadcast_settings("intl");
}
// ---- Language & region ----

/// A multi-string read from the registry, split at its nulls into its parts.
pub(crate) fn reg_multi(root: HKEY, key: &str, value: &str) -> Vec<String> {
    use windows::Win32::System::Registry::RRF_RT_REG_MULTI_SZ;
    let (key, value) = (HSTRING::from(key), HSTRING::from(value));
    let mut units = [0u16; 1024];
    let mut size = (units.len() * 2) as u32;
    let read = unsafe {
        RegGetValueW(
            root,
            &key,
            &value,
            RRF_RT_REG_MULTI_SZ,
            None,
            Some(units.as_mut_ptr() as *mut _),
            Some(&mut size),
        )
    };
    if read.is_err() {
        return Vec::new();
    }
    let n = (size as usize / 2).min(units.len());
    units[..n]
        .split(|&c| c == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf16_lossy(s))
        .collect()
}

/// The regional format's name, as Windows keeps it ("zh-CN").
pub(crate) fn locale_name() -> String {
    reg_sz(
        HKEY_CURRENT_USER,
        r"Control Panel\International",
        "LocaleName",
    )
}

/// The country or region's two-letter code the machine is set to ("CN").
pub(crate) fn geo_name() -> String {
    reg_sz(
        HKEY_CURRENT_USER,
        r"Control Panel\International\Geo",
        "Name",
    )
}

/// The short date pattern in use ("yyyy/M/d").
pub(crate) fn short_date() -> String {
    reg_sz(
        HKEY_CURRENT_USER,
        r"Control Panel\International",
        "sShortDate",
    )
}

/// The time pattern in use ("H:mm:ss").
pub(crate) fn time_format() -> String {
    reg_sz(
        HKEY_CURRENT_USER,
        r"Control Panel\International",
        "sTimeFormat",
    )
}

/// The languages the display may be worn in, most wanted first; empty where
/// the machine keeps only the one it was set up with.
pub(crate) fn preferred_languages() -> Vec<String> {
    reg_multi(
        HKEY_CURRENT_USER,
        r"Control Panel\International\User Profile",
        "PreferredLanguages",
    )
}

/// A country's name for its two-letter code, in the tongue asked for; the code
/// itself where it is not one of the few kept here.
pub(crate) fn country_name(code: &str, lang: Lang) -> String {
    let said = match code {
        "CN" => lang.pick("中国", "China"),
        "HK" => lang.pick("中国香港", "Hong Kong SAR"),
        "TW" => lang.pick("中国台北", "Taipei"),
        "MO" => lang.pick("中国澳门", "Macao SAR"),
        "US" => lang.pick("美国", "United States"),
        "JP" => lang.pick("日本", "Japan"),
        "KR" => lang.pick("韩国", "Korea"),
        "SG" => lang.pick("新加坡", "Singapore"),
        "GB" => lang.pick("英国", "United Kingdom"),
        "DE" => lang.pick("德国", "Germany"),
        "FR" => lang.pick("法国", "France"),
        _ => return code.to_string(),
    };
    said.to_string()
}

// ---- Sharing (the network and sharing centre) ----

/// The workgroup or domain the machine belongs to, where it belongs to one.
pub(crate) fn workgroup() -> String {
    let key = r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters";
    let group = reg_sz(HKEY_LOCAL_MACHINE, key, "Workgroup");
    if group.is_empty() {
        reg_sz(HKEY_LOCAL_MACHINE, key, "Domain")
    } else {
        group
    }
}

// ---- Nearby sharing (AirDrop and Handoff's Windows kin) ----

/// The key the sharing and handoff switches are kept under.
const CDP: &str = r"Software\Microsoft\Windows\CurrentVersion\CDP";

/// Whether nearby sharing is let on: the channel's authorisation level, absent
/// or zero, is off.
pub(crate) fn nearby_sharing() -> bool {
    reg_word(HKEY_CURRENT_USER, CDP, "NearShareChannelUserAuthLevel").unwrap_or(0) != 0
}

/// Turns nearby sharing on to this user's own devices, or off again; the shell
/// is told so the page follows what truly took.
pub(crate) fn set_nearby_sharing(on: bool) {
    reg_write(CDP, "NearShareChannelUserAuthLevel", if on { 2 } else { 0 });
    broadcast_settings("CDPChange");
}

// ---- Backup (file history) ----

/// Where file history keeps its own settings, if it has ever been turned on.
pub(crate) fn file_history_configured() -> bool {
    std::env::var("LOCALAPPDATA")
        .map(|base| std::path::Path::new(&base).join(r"Microsoft\Windows\File History"))
        .map(|path| path.is_dir())
        .unwrap_or(false)
}

// ---- Recovery (startup disk) ----

/// Whether the machine boots by UEFI rather than the older BIOS; `None` where
/// the firmware will not say.
pub(crate) fn firmware_uefi() -> Option<bool> {
    use windows::Win32::System::SystemInformation::{
        FirmwareTypeUefi, GetFirmwareType, FIRMWARE_TYPE,
    };
    let mut kind = FIRMWARE_TYPE(0);
    if unsafe { GetFirmwareType(&mut kind) }.is_ok() {
        Some(kind == FirmwareTypeUefi)
    } else {
        None
    }
}

/// Whether secure boot is worn at start; `None` where the machine does not
/// keep the state (no secure boot at all).
pub(crate) fn secure_boot() -> Option<bool> {
    reg_word(
        HKEY_LOCAL_MACHINE,
        r"SYSTEM\CurrentControlSet\Control\SecureBoot\State",
        "SecureBoot",
    )
    .map(|word| word != 0)
}

/// Restarts into the recovery environment (true) or the firmware's own setup
/// (false), asking for the privilege the restart wants.
pub(crate) fn reboot_to(recovery: bool) {
    use windows::Win32::UI::Shell::ShellExecuteW;
    let params = if recovery {
        "/r /o /t 0"
    } else {
        "/r /fw /t 0"
    };
    unsafe {
        ShellExecuteW(
            None,
            w!("runas"),
            w!("shutdown.exe"),
            &HSTRING::from(params),
            PCWSTR::null(),
            SW_HIDE,
        );
    }
}

// ---- Privacy consent ----

pub(crate) fn consent_on(capability: &str) -> bool {
    let key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\{capability}"
    );
    reg_sz(HKEY_CURRENT_USER, &key, "Value") == "Allow"
}

pub(crate) fn consent_set(capability: &str, on: bool) {
    let key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\{capability}"
    );
    reg_write_sz(&key, "Value", if on { "Allow" } else { "Deny" });
}

// ---- Gaming ----

pub(crate) fn game_flag(name: &str) -> bool {
    reg_word(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\GameDVR",
        name,
    )
    .unwrap_or(0)
        != 0
}

pub(crate) fn game_mode() -> bool {
    reg_word(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\GameBar",
        "AllowAutoGameMode",
    )
    .unwrap_or(1)
        != 0
}

pub(crate) fn toggle_game_flag(name: &str) {
    reg_write(
        r"Software\Microsoft\Windows\CurrentVersion\GameDVR",
        name,
        u32::from(!game_flag(name)),
    );
}

pub(crate) fn toggle_game_mode() {
    let path = r"Software\Microsoft\GameBar";
    let name = "AllowAutoGameMode";
    let on = reg_word(HKEY_CURRENT_USER, path, name).unwrap_or(1) != 0;
    reg_write(path, name, u32::from(!on));
}

// ---- Notifications ----

pub(crate) fn notify_global() -> bool {
    reg_word(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\Notifications\Settings",
        "NOC_GLOBAL_SETTING_TOASTS_ENABLED",
    )
    .unwrap_or(1)
        != 0
}

pub(crate) fn toggle_notify_global() {
    let path = r"Software\Microsoft\Windows\CurrentVersion\Notifications\Settings";
    let name = "NOC_GLOBAL_SETTING_TOASTS_ENABLED";
    let on = reg_word(HKEY_CURRENT_USER, path, name).unwrap_or(1) != 0;
    reg_write(path, name, u32::from(!on));
    broadcast_settings("Environment");
}

pub(crate) fn lock_toast() -> bool {
    reg_word(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\Notifications\Settings",
        "NOC_GLOBAL_SETTING_ALLOW_TOASTS_ABOVE_LOCK",
    )
    .unwrap_or(1)
        != 0
}

pub(crate) fn set_lock_toast(on: bool) {
    reg_write(
        r"Software\Microsoft\Windows\CurrentVersion\Notifications\Settings",
        "NOC_GLOBAL_SETTING_ALLOW_TOASTS_ABOVE_LOCK",
        u32::from(on),
    );
    broadcast_settings("Environment");
}

// ---- Theme: dark, transparent, animation, accent ----

pub(crate) fn system_dark() -> bool {
    reg_word(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "AppsUseLightTheme",
    ) != Some(1)
}

pub(crate) fn set_system_dark(dark: bool) {
    let value = u32::from(!dark);
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            None,
            KEY_WRITE,
            &mut key,
        )
        .is_ok()
        {
            for name in ["AppsUseLightTheme", "SystemUsesLightTheme"] {
                let _ = RegSetValueExW(
                    key,
                    &HSTRING::from(name),
                    None,
                    REG_DWORD,
                    Some(&value.to_le_bytes()),
                );
            }
            let _ = RegCloseKey(key);
        }
        let word: Vec<u16> = "Theme".encode_utf16().chain([0]).collect();
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

pub(crate) fn system_transparent() -> bool {
    reg_word(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "EnableTransparency",
    ) == Some(1)
}

pub(crate) fn set_system_transparent(on: bool) {
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            None,
            KEY_WRITE,
            &mut key,
        )
        .is_ok()
        {
            let data = u32::from(on).to_le_bytes();
            let _ = RegSetValueExW(
                key,
                &HSTRING::from("EnableTransparency"),
                None,
                REG_DWORD,
                Some(&data),
            );
            let _ = RegCloseKey(key);
        }
    }
    broadcast_settings("Themes");
}

pub(crate) fn system_animation() -> bool {
    let mut info = ANIMATIONINFO {
        cbSize: size_of::<ANIMATIONINFO>() as u32,
        iMinAnimate: 0,
    };
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETANIMATION,
            size_of::<ANIMATIONINFO>() as u32,
            Some(&mut info as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    info.iMinAnimate != 0
}

pub(crate) fn set_system_animation(on: bool) {
    let info = ANIMATIONINFO {
        cbSize: size_of::<ANIMATIONINFO>() as u32,
        iMinAnimate: i32::from(on),
    };
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_SETANIMATION,
            size_of::<ANIMATIONINFO>() as u32,
            Some(&info as *const _ as *mut _),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        );
    }
}

pub(crate) fn color_prevalence() -> bool {
    reg_word(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\DWM",
        "ColorPrevalence",
    )
    .unwrap_or(0)
        != 0
}

pub(crate) fn set_color_prevalence(on: bool) {
    reg_write(
        r"Software\Microsoft\Windows\DWM",
        "ColorPrevalence",
        u32::from(on),
    );
    reg_write(
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "ColorPrevalence",
        u32::from(on),
    );
    broadcast_settings("Theme");
}

// ---- Taskbar ----

pub(crate) fn task_view_button() -> bool {
    reg_word(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        "ShowTaskViewButton",
    )
    .unwrap_or(1)
        != 0
}

pub(crate) fn set_task_view_button(on: bool) {
    reg_write(
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        "ShowTaskViewButton",
        u32::from(on),
    );
    broadcast_settings("Environment");
}

pub(crate) fn taskbar_centred() -> bool {
    reg_word(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        "TaskbarAl",
    )
    .unwrap_or(1)
        != 0
}

pub(crate) fn set_taskbar_centred(on: bool) {
    reg_write(
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        "TaskbarAl",
        u32::from(on),
    );
    broadcast_settings("Environment");
}

// ---- High contrast & magnifier ----

#[repr(C)]
struct HighContrast {
    cb_size: u32,
    dw_flags: u32,
    lpsz_scheme: *mut u16,
}

const HCF_ON: u32 = 0x0000_0001;

pub(crate) fn high_contrast() -> bool {
    let mut suit = HighContrast {
        cb_size: size_of::<HighContrast>() as u32,
        dw_flags: 0,
        lpsz_scheme: std::ptr::null_mut(),
    };
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            size_of::<HighContrast>() as u32,
            Some(&mut suit as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    suit.dw_flags & HCF_ON != 0
}

pub(crate) fn set_high_contrast(on: bool) {
    let mut suit = HighContrast {
        cb_size: size_of::<HighContrast>() as u32,
        dw_flags: if on { HCF_ON } else { 0 },
        lpsz_scheme: std::ptr::null_mut(),
    };
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_SETHIGHCONTRAST,
            size_of::<HighContrast>() as u32,
            Some(&mut suit as *mut _ as *mut _),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        );
    }
}

// ---- Input: mouse, wheel, keyboard ----

macro_rules! spi_get {
    ($action:expr) => {{
        let mut value = 0i32;
        unsafe {
            let _ = SystemParametersInfoW(
                $action,
                0,
                Some(&mut value as *mut _ as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
        }
        value
    }};
}

macro_rules! spi_set {
    ($action:expr, $value:expr) => {{
        let value: i32 = $value;
        unsafe {
            let _ = SystemParametersInfoW(
                $action,
                0,
                Some(&value as *const _ as *mut _),
                SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
            );
        }
    }};
}

pub(crate) fn read_input() -> (i32, i32, i32, i32) {
    (
        spi_get!(SPI_GETMOUSESPEED),
        spi_get!(SPI_GETWHEELSCROLLLINES),
        spi_get!(SPI_GETKEYBOARDDELAY),
        spi_get!(SPI_GETKEYBOARDSPEED),
    )
}

pub(crate) fn set_mouse_speed(v: i32) {
    spi_set!(SPI_SETMOUSESPEED, v);
}

pub(crate) fn set_scroll_lines(v: i32) {
    spi_set!(SPI_SETWHEELSCROLLLINES, v);
}

pub(crate) fn set_keyboard_delay(v: i32) {
    spi_set!(SPI_SETKEYBOARDDELAY, v);
}

pub(crate) fn set_keyboard_rate(v: i32) {
    spi_set!(SPI_SETKEYBOARDSPEED, v);
}

// ---- Battery & drives ----

pub(crate) fn battery_status() -> Option<(u8, bool, bool)> {
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    if status.BatteryFlag & 128 != 0 || status.BatteryLifePercent == 255 {
        return None;
    }
    Some((
        status.BatteryLifePercent,
        status.BatteryFlag & 8 != 0,
        status.ACLineStatus == 1,
    ))
}

pub(crate) fn drive_space() -> Vec<(String, u64, u64)> {
    let mounted = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|letter| mounted & (1 << letter) != 0)
        .filter_map(|letter| {
            let name = format!("{}:", (b'A' + letter) as char);
            let root: Vec<u16> = format!("{name}\\").encode_utf16().chain([0]).collect();
            let (mut total, mut free) = (0u64, 0u64);
            unsafe {
                GetDiskFreeSpaceExW(
                    PCWSTR(root.as_ptr()),
                    None,
                    Some(&mut total),
                    Some(&mut free),
                )
            }
            .ok()?;
            (total > 0).then(|| (name, total - free, total))
        })
        .collect()
}

// ---- Wallpaper ----

fn is_wall(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("jpg") | Some("jpeg") | Some("png")
    )
}

fn push_wall(path: &std::path::Path, out: &mut Vec<(String, String)>) {
    if !is_wall(path) {
        return;
    }
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    out.push((name, path.to_string_lossy().into_owned()));
}

fn collect_walls(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Ok(sub) = std::fs::read_dir(&path) {
                for below in sub.flatten() {
                    push_wall(&below.path(), out);
                }
            }
        } else {
            push_wall(&path, out);
        }
    }
}

pub(crate) fn wallpaper_list() -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Ok(windir) = std::env::var("WINDIR") {
        collect_walls(
            &std::path::Path::new(&windir).join(r"Web\Wallpaper"),
            &mut out,
        );
    }
    out.sort();
    out.dedup_by(|a, b| a.1 == b.1);
    out
}

pub(crate) fn set_wallpaper(path: &str) {
    let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(wide.as_ptr() as *mut _),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        );
    }
}

pub(crate) fn wallpaper_path() -> String {
    let mut path = [0u16; 260];
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            path.len() as u32,
            Some(path.as_mut_ptr() as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if read.is_err() {
        return String::new();
    }
    let length = path.iter().position(|&c| c == 0).unwrap_or(0);
    String::from_utf16_lossy(&path[..length])
}

// ---- About: the machine's facts ----

const GB: u64 = 1024 * 1024 * 1024;

fn gigabytes(bytes: u64) -> String {
    let gb = (bytes as f64 / GB as f64 * 10.0).round() / 10.0;
    if gb.fract() == 0.0 {
        format!("{} GB", gb as u64)
    } else {
        format!("{gb:.1} GB")
    }
}

fn said(first: String, second: String) -> String {
    if first.is_empty() {
        second
    } else {
        first
    }
}

pub(crate) fn computer_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_default()
}

/// A screen, as the pane shows it.
#[derive(Clone)]
struct Shown {
    size: String,
    scale: String,
    primary: bool,
}

/// The screens as they are now, each with the scale it was made for.
fn read_screens() -> Vec<Shown> {
    crate::screens::all()
        .iter()
        .map(|screen| {
            let (wide, tall) = (
                screen.monitor.right - screen.monitor.left,
                screen.monitor.bottom - screen.monitor.top,
            );
            let dpi = unsafe {
                let monitor = MonitorFromRect(&screen.monitor, MONITOR_DEFAULTTONEAREST);
                let (mut dpi, mut vertical) = (96u32, 96u32);
                let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut vertical);
                dpi
            };
            Shown {
                size: format!("{wide} × {tall}"),
                scale: format!("{}%", dpi as u32 * 100 / 96),
                primary: screen.primary,
            }
        })
        .collect()
}

/// Each screen as the about page lists it: whether it is the main one, its size,
/// and the scale it was made for.
pub(crate) fn about_screens() -> Vec<(bool, String, String)> {
    read_screens()
        .into_iter()
        .map(|s| (s.primary, s.size, s.scale))
        .collect()
}

/// What this PC is made of, as the pane reads it.
struct About {
    machine: String,
    os: String,
    cpu: String,
    threads: usize,
    memory: String,
    gpus: Vec<String>,
    screen: String,
    serial: String,
    bios: String,
}

fn read_about() -> About {
    let info = &crate::app().info;
    let bios_key = w!(r"HARDWARE\DESCRIPTION\System\BIOS");
    let machine_key = w!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\OEMInformation");
    let stock = crate::smbios::system();
    let badge = reg_string(machine_key, w!("Model"));
    let named = reg_string(bios_key, w!("SystemProductName"));
    About {
        machine: said(
            said(badge, stock.product),
            if named.is_empty() {
                info.board.clone()
            } else {
                named
            },
        ),
        serial: stock.serial,
        bios: stock.bios,
        os: crate::diagnostics::windows(w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"))
            .strip_prefix("Windows: ")
            .unwrap_or("")
            .to_string(),
        cpu: info.cpu_name.clone(),
        threads: info.threads,
        memory: info
            .memory_modules
            .clone()
            .unwrap_or_else(|| gigabytes(info.mem_total)),
        gpus: info.gpus.iter().map(|gpu| gpu.name.clone()).collect(),
        screen: {
            let screens = read_screens();
            screens
                .iter()
                .find(|one| one.primary)
                .or_else(|| screens.first())
                .map(|one| format!("{} · {}", one.size, one.scale))
                .unwrap_or_default()
        },
    }
}

pub(crate) fn windows_edition() -> String {
    read_about().os
}

/// This PC, as the about page tells it: a label and a value to a row.
pub(crate) fn about_facts(lang: Lang) -> Vec<(String, String)> {
    let about = read_about();
    let none = lang.pick("固件没有登记", "Nothing stamped");
    let told = |said: &str| -> String {
        if said.is_empty() {
            none.to_string()
        } else {
            said.to_string()
        }
    };
    let host = computer_name();
    let head = if about.machine.is_empty() {
        host.clone()
    } else {
        about.machine.clone()
    };
    let gpus = about.gpus.join(" · ");
    // The further hardware the app already knows from the firmware and Windows: the
    // board's own model, each physical drive, and the adapter traffic leaves by.
    let info = &crate::app().info;
    let board = info.board.clone();
    let drives = info.drives.join(" · ");
    let nic = info.network_adapter.clone().unwrap_or_default();
    vec![
        (lang.pick("设备名称", "Device name").to_string(), host),
        (lang.pick("型号", "Model").to_string(), told(&head)),
        (lang.pick("系统", "System").to_string(), told(&about.os)),
        (
            lang.pick("处理器", "Processor").to_string(),
            format!("{} · {}", about.cpu, about.threads),
        ),
        (
            lang.pick("内存", "Memory").to_string(),
            about.memory.clone(),
        ),
        (lang.pick("主板", "Motherboard").to_string(), told(&board)),
        (lang.pick("显卡", "Graphics").to_string(), told(&gpus)),
        (lang.pick("硬盘", "Storage").to_string(), told(&drives)),
        (
            lang.pick("显示器", "Display").to_string(),
            told(&about.screen),
        ),
        (lang.pick("网卡", "Network").to_string(), told(&nic)),
        (
            lang.pick("序列号", "Serial number").to_string(),
            told(&about.serial),
        ),
        (
            lang.pick("BIOS 版本", "BIOS version").to_string(),
            told(&about.bios),
        ),
    ]
}

// ---- Device art (used by About preview) ----

/// The machine drawn: a laptop or desktop in the archive's style (96 wide).
pub(crate) fn device_art(frame: &Frame, x: f32, y: f32, laptop: bool) {
    let k = 96.0 / 152.0;
    let (sx, sy) = (|vx: f32| x + vx * k, |vy: f32| y + vy * k);
    let (lid, void, edge, metal, foot) = (
        Color::hex(0x20242E, 0.95),
        Color::hex(0x04050A, 0.9),
        Color::hex(0xFFFFFF, 0.32),
        Color::hex(0xB4BCCC, 0.55),
        Color::hex(0x000000, 0.16),
    );
    if laptop {
        fill(frame, lid, sx(21.0), sy(6.0), 110.0 * k, 70.0 * k, 4.6 * k);
        frame.stroke_rounded(
            Fill::Solid(edge),
            sx(21.0),
            sy(6.0),
            110.0 * k,
            70.0 * k,
            4.6 * k,
            1.2 * k,
        );
        fill(frame, void, sx(24.4), sy(9.2), 103.2 * k, 63.6 * k, 2.4 * k);
        frame.fill_shape(
            &[
                Point {
                    x: sx(26.4),
                    y: sy(11.2),
                },
                Point {
                    x: sx(125.6),
                    y: sy(11.2),
                },
                Point {
                    x: sx(125.6),
                    y: sy(70.8),
                },
                Point {
                    x: sx(26.4),
                    y: sy(70.8),
                },
            ],
            Fill::Down {
                top: sy(11.2),
                from: Color::hex(0x8FB4FF, 0.60),
                bottom: sy(70.8),
                to: Color::hex(0xA86BFF, 0.34),
            },
        );
        fill(frame, void, sx(68.0), sy(11.2), 16.0 * k, 5.0 * k, 2.0 * k);
        frame.fill_shape(
            &[
                Point {
                    x: sx(12.0),
                    y: sy(78.0),
                },
                Point {
                    x: sx(140.0),
                    y: sy(78.0),
                },
                Point {
                    x: sx(148.0),
                    y: sy(100.0),
                },
                Point {
                    x: sx(4.0),
                    y: sy(100.0),
                },
            ],
            Fill::Solid(metal),
        );
        fill(frame, void, sx(54.0), sy(82.0), 44.0 * k, 14.0 * k, 2.0 * k);
    } else {
        fill(frame, lid, sx(48.0), sy(6.0), 56.0 * k, 94.0 * k, 4.0 * k);
        frame.stroke_rounded(
            Fill::Solid(edge),
            sx(48.0),
            sy(6.0),
            56.0 * k,
            94.0 * k,
            4.0 * k,
            1.2 * k,
        );
        fill(frame, void, sx(60.0), sy(18.0), 16.0 * k, 16.0 * k, 8.0 * k);
        fill(frame, metal, sx(52.0), sy(56.0), 48.0 * k, 4.0 * k, 2.0 * k);
        fill(frame, foot, sx(44.0), sy(100.0), 64.0 * k, 4.0 * k, 2.0 * k);
    }
}
