//! How long each app has been the one in front, kept day by day — the numbers
//! the 屏幕使用时间 page shows. Glance runs the whole time the PC is up, so a
//! thread that asks, every few seconds, which window is frontmost gathers this
//! without any help from Windows. The tally lives in one file beside the
//! settings, on this PC only.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// How long a day's worth of seconds is counted for one app at a time.
const STEP: u64 = 10;

/// Days of usage: the day as `YYYYMMDD`, and for it each app's seconds.
type Days = BTreeMap<u32, BTreeMap<String, u64>>;

/// Each day's seconds split across its 24 hours, so the page can show when in
/// the day the screen was kept busy.
type Hours = BTreeMap<u32, [u64; 24]>;

/// The tally, and whether the thread that fills it has been started.
static LEDGER: OnceLock<Mutex<Days>> = OnceLock::new();
/// The same tally told by the hour, kept beside it in a file of its own.
static HOUR_LEDGER: OnceLock<Mutex<Hours>> = OnceLock::new();
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Where the tally is kept, beside the settings.
fn file() -> PathBuf {
    crate::settings::config_dir().join("screen-time.tsv")
}

/// Where the hourly tally is kept, beside the other.
fn hours_file() -> PathBuf {
    crate::settings::config_dir().join("screen-time-hours.tsv")
}

/// The hourly tally, read from its file the first time it is asked for.
fn hour_ledger() -> &'static Mutex<Hours> {
    HOUR_LEDGER.get_or_init(|| Mutex::new(load_hours()))
}

/// The hourly file read: one `day<TAB>hour<TAB>seconds` to a line.
fn load_hours() -> Hours {
    let mut map = Hours::new();
    let Ok(text) = std::fs::read_to_string(hours_file()) else {
        return map;
    };
    for line in text.lines() {
        let Some((day, rest)) = line.split_once('\t') else {
            continue;
        };
        let Some((hour, secs)) = rest.split_once('\t') else {
            continue;
        };
        let (Ok(day), Ok(hour), Ok(secs)) = (
            day.parse::<u32>(),
            hour.parse::<usize>(),
            secs.parse::<u64>(),
        ) else {
            continue;
        };
        if hour < 24 {
            map.entry(day).or_insert([0u64; 24])[hour] += secs;
        }
    }
    map
}

/// The hourly tally written back, oldest days dropped as the app tally is.
fn save_hours(map: &Hours) {
    let keep: Vec<&u32> = {
        let mut keys: Vec<&u32> = map.keys().collect();
        keys.sort_unstable();
        keys.into_iter().rev().take(21).collect()
    };
    let mut text = String::new();
    for day in keep.iter().copied() {
        if let Some(row) = map.get(day) {
            for (hour, secs) in row.iter().enumerate() {
                if *secs > 0 {
                    text.push_str(&format!("{day}\t{hour}\t{secs}\n"));
                }
            }
        }
    }
    let _ = std::fs::write(hours_file(), text);
}

/// The tally, read from the file the first time it is asked for.
fn ledger() -> &'static Mutex<Days> {
    LEDGER.get_or_init(|| Mutex::new(load()))
}

/// The file read: one `day<TAB>app<TAB>seconds` to a line; nothing where it is
/// missing or cannot be read.
fn load() -> Days {
    let mut days = Days::new();
    let Ok(text) = std::fs::read_to_string(file()) else {
        return days;
    };
    for line in text.lines() {
        let Some((day, rest)) = line.split_once('\t') else {
            continue;
        };
        let Some((app, secs)) = rest.split_once('\t') else {
            continue;
        };
        let (Ok(day), Ok(secs)) = (day.parse::<u32>(), secs.parse::<u64>()) else {
            continue;
        };
        if !app.is_empty() {
            *days
                .entry(day)
                .or_default()
                .entry(app.to_string())
                .or_default() += secs;
        }
    }
    days
}

/// The tally written back, oldest days dropped so the file stays small.
fn save(days: &Days) {
    // (Only the last three weeks are kept: enough for the page's charts, and
    // never a file that grows without end.)
    let keep: Vec<&u32> = {
        let mut keys: Vec<&u32> = days.keys().collect();
        keys.sort_unstable();
        keys.into_iter().rev().take(21).collect()
    };
    let mut text = String::new();
    for day in keep.iter().copied() {
        for (app, secs) in days.get(day).into_iter().flatten() {
            text.push_str(&format!("{day}\t{app}\t{secs}\n"));
        }
    }
    let _ = std::fs::write(file(), text);
}

/// Today, as `YYYYMMDD`, from the clock — the local day, UTC being near enough
/// for a usage tally.
fn today() -> u32 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    let (y, m, d) = civil(secs / 86_400);
    (y * 10_000 + m * 100 + d) as u32
}

/// The hour of the local clock, 0 to 23 — the hourly tally is keyed to it, so
/// a bar stands under the hour the user actually lived through.
fn local_hour() -> usize {
    (unsafe { GetLocalTime() }).wHour as usize
}

/// The `n` days back from today, as `YYYYMMDD`.
fn day_back(n: i64) -> u32 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    let (y, m, d) = civil(secs / 86_400 - n);
    (y * 10_000 + m * 100 + d) as u32
}

/// Days since the epoch turned to a year, month and day (Howard Hinnant's
/// civil algorithm), so no calendar library is needed.
fn civil(days: i64) -> (i32, i32, i32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1461 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    ((if y <= 0 { y + 1 } else { y }) as i32, m as i32, d as i32)
}

/// What process `window` runs, by the name of its program: nothing where it
/// cannot be opened or named.
fn front(window: HWND) -> String {
    if window.is_invalid() {
        return String::new();
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    if pid == 0 {
        return String::new();
    }
    let mut name = [0u16; 1024];
    let mut length = name.len() as u32;
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let named = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(name.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        if named.is_err() || length == 0 {
            return String::new();
        }
        let path = String::from_utf16_lossy(&name[..length as usize]);
        // The program's own name, without its folder or its `.exe`.
        std::path::Path::new(&path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or(path)
    }
}

/// Start the thread that gathers usage, once; where it is already running,
/// nothing more is done.
pub fn start() {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(STEP));
        let app = front(unsafe { GetForegroundWindow() });
        if app.is_empty() {
            continue;
        }
        let day = today();
        let hour = local_hour();
        let write = {
            let mut days = ledger()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let apps = days.entry(day).or_default();
            *apps.entry(app).or_default() += STEP;
            let mut map = hour_ledger()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            map.entry(day).or_insert([0u64; 24])[hour] += STEP;
            day % 6 == 0
        };
        if write {
            let days = ledger()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            save(&days);
            let map = hour_ledger()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            save_hours(&map);
        }
    });
}

/// The app in front right now, by name.
pub fn foreground() -> String {
    front(unsafe { GetForegroundWindow() })
}

/// Today's usage: the whole, and each app by its seconds, most first.
pub fn today_usage() -> (u64, Vec<(String, u64)>) {
    let days = ledger()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    usage_of(&days, &today())
}

/// The whole for one day.
fn usage_of(days: &Days, day: &u32) -> (u64, Vec<(String, u64)>) {
    let Some(apps) = days.get(day) else {
        return (0, Vec::new());
    };
    let total = apps.values().sum();
    let mut list: Vec<(String, u64)> = apps.clone().into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1));
    (total, list)
}

/// Yesterday's whole, in seconds.
pub fn yesterday_usage() -> u64 {
    let days = ledger()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    usage_of(&days, &(day_back(1))).0
}

/// The last seven days, oldest first: each as its `YYYYMMDD` and its whole.
pub fn week() -> Vec<(u32, u64)> {
    let days = ledger()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    (0..7)
        .rev()
        .map(|back| {
            let day = day_back(back);
            (day, usage_of(&days, &day).0)
        })
        .collect()
}

/// A day's `YYYYMMDD` said as `MM-DD`.
pub fn label(day: u32) -> String {
    format!("{:02}-{:02}", (day / 100) % 100, day % 100)
}

/// Today's seconds across the 24 hours, hour 0 first.
pub fn hours_today() -> [u64; 24] {
    let map = hour_ledger()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    map.get(&today()).copied().unwrap_or([0u64; 24])
}

/// Today's usage sorted into the kinds of app it was spent on, most first; each
/// as its name in Chinese and in English, and its seconds. Programs are told
/// apart by the words their own name holds, so the guess is only as good as the
/// name — what fits none is counted 其他.
pub fn categories() -> Vec<(&'static str, &'static str, u64)> {
    // Ordered: the first kind whose word the name holds wins, so a game stays a
    // game and an editor stays development.
    const KINDS: [(&str, &str, &[&str]); 7] = [
        (
            "游戏",
            "Games",
            &[
                "steam",
                "epic",
                "league",
                "wegame",
                "minecraft",
                "valorant",
                "genshin",
                "game",
            ],
        ),
        (
            "开发",
            "Development",
            &[
                "code",
                "qoder",
                "idea",
                "pycharm",
                "webstorm",
                "goland",
                "rustrover",
                "studio",
                "devenv",
                "terminal",
                "wt",
                "powershell",
                "cmd",
                "git",
                "node",
                "python",
                "java",
                "docker",
                "wsl",
            ],
        ),
        (
            "社交",
            "Social",
            &[
                "wechat", "weixin", "qq", "dingtalk", "telegram", "discord", "whatsapp", "slack",
                "teams", "mail", "outlook", "飞书", "微信", "钉钉",
            ],
        ),
        (
            "创意",
            "Creativity",
            &[
                "photoshop",
                "premiere",
                "aftereffect",
                "figma",
                "blender",
                "gimp",
                "davinci",
                "illustrator",
                "capcut",
                "剪映",
            ],
        ),
        (
            "生产力",
            "Productivity",
            &[
                "word",
                "excel",
                "powerpoint",
                "onenote",
                "notion",
                "obsidian",
                "wps",
                "office",
                "libre",
                "evernote",
                "notepad",
            ],
        ),
        (
            "娱乐",
            "Entertainment",
            &[
                "douyin", "bilibili", "spotify", "music", "netease", "video", "movie", "抖音",
                "哔哩",
            ],
        ),
        (
            "工具",
            "Utilities",
            &[
                "explorer", "settings", "taskmgr", "control", "glance", "7z", "winrar",
            ],
        ),
    ];
    let (_, apps) = today_usage();
    let mut found: Vec<(&'static str, &'static str, u64)> = Vec::new();
    for (app, secs) in apps {
        let low = app.to_lowercase();
        let (zh, en) = KINDS
            .iter()
            .find(|(_, _, words)| words.iter().any(|word| low.contains(word)))
            .map_or(("其他", "Other"), |(zh, en, _)| (zh, en));
        if let Some(same) = found.iter_mut().find(|(fz, _, _)| *fz == zh) {
            same.2 += secs;
        } else {
            found.push((zh, en, secs));
        }
    }
    found.sort_by(|a, b| b.2.cmp(&a.2));
    found
}

/// Seconds told as macOS tells them: hours and minutes, or `<1 分` when short.
pub fn say(seconds: u64, zh: bool) -> String {
    if seconds < 60 {
        return if zh {
            "<1 分".to_string()
        } else {
            "<1 min".to_string()
        };
    }
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours == 0 {
        if zh {
            format!("{minutes} 分")
        } else {
            format!("{minutes} min")
        }
    } else if zh {
        format!("{hours} 时 {minutes} 分")
    } else {
        format!("{hours} hr {minutes} min")
    }
}
