//! The stage drawn for the machine itself: its system, its name and hardware, whether there is a new
//! version, and the room left on every drive.

use super::*;

impl Ui {

    /// The PC's own categories' preview: a card marked with the page, and where
    /// a plain reading serves, that reading told — a shade, a count, a toggle's
    /// word. Nothing is dressed up as live that does not follow the system.
    pub(super) fn paint_os_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
        use crate::ui::system_settings as sys;
        let lang = self.lang;
        let p = |zh, en| pick(lang, zh, en).to_string();
        fill(frame, palette.card, area.x, area.y, area.w, area.h, 8.0);
        stroke_inside(frame, area, 8.0, palette.card_stroke);
        // Four pages are shown as the machine and its network really are now: their
        // owner's picture, the machine's own shape over its live readings, and the
        // traffic over the wire — not a lone icon and a line of words.
        match self.page {
            Page::About => return self.paint_about_preview(frame, palette, area),
            Page::Update => return self.paint_update_preview(frame, palette, area),
            Page::Storage => return self.paint_storage_preview(frame, palette, area),
            Page::Startup => return self.paint_startup_preview(frame, palette, area),
            Page::DateTime => return self.paint_datetime_preview(frame, palette, area),
            Page::Language => return self.paint_language_preview(frame, palette, area),
            Page::Sharing => return self.paint_sharing_preview(frame, palette, area),
            Page::Nearby => return self.paint_nearby_preview(frame, palette, area),
            Page::Backup => return self.paint_backup_preview(frame, palette, area),
            Page::Recovery => return self.paint_recovery_preview(frame, palette, area),
            Page::Account => return self.paint_account_preview(frame, palette, area),
            Page::Wifi => return self.paint_wifi_preview(frame, palette, area),
            Page::Network => return self.paint_network_preview(frame, palette, area),
            Page::Bluetooth => return self.paint_bt_preview(frame, palette, area),
            Page::Display => return self.paint_display_preview(frame, palette, area),
            Page::ScreenTime => return self.paint_screen_time_preview(frame, palette, area),
            Page::Privacy => return self.paint_privacy_preview(frame, palette, area),
            Page::Sound => return self.paint_sound_preview(frame, palette, area),
            _ => {}
        }
        let cx = area.x + area.w / 2.0;
        // What the page turns, told as the system has it now.
        let mut bits: Vec<String> = Vec::new();
        match self.page {
            Page::Appearance | Page::ControlCenter => {
                bits.push(if sys::system_dark() {
                    p("深色", "Dark")
                } else {
                    p("浅色", "Light")
                });
                bits.push(if sys::system_transparent() {
                    p("磨砂玻璃", "Frosted")
                } else {
                    p("不透明", "Opaque")
                });
                bits.push(if sys::system_animation() {
                    p("动画开", "Animation on")
                } else {
                    p("动画关", "Animation off")
                });
                bits.push(if sys::color_prevalence() {
                    p("强调色上边框", "Accent on borders")
                } else {
                    p("强调色仅应用内", "Accent in apps")
                });
                // The colour itself, named as the appearance page calls it.
                if let Some(word) = crate::sys::accent() {
                    let rgb = word & 0x00ff_ffff;
                    if let Some((_, zh, en)) = ACCENTS.iter().find(|(c, ..)| *c == rgb) {
                        bits.push(format!("{} {}", p("强调色", "Accent"), pick(lang, zh, en)));
                    }
                }
            }
            Page::Desktop => {
                bits.push(if sys::clock_seconds() {
                    p("时钟带秒", "To the second")
                } else {
                    p("时钟不带秒", "No seconds")
                });
                bits.push(if sys::taskbar_centred() {
                    p("图标居中", "Icons centred")
                } else {
                    p("图标靠左", "Icons left")
                });
                bits.push(if sys::task_view_button() {
                    p("有任务视图按钮", "Task view shown")
                } else {
                    p("无任务视图按钮", "No task view")
                });
            }
            Page::DateTime => {
                bits.push(if sys::clock_24() {
                    p("24 小时制", "24-hour")
                } else {
                    p("12 小时制", "12-hour")
                });
                if sys::clock_seconds() {
                    bits.push(p("时钟带秒", "To the second"));
                }
                let zone = crate::ui::system_settings::current_zone();
                if !zone.is_empty() {
                    bits.push(zone);
                }
                if let Some(done) = self.os.clock_said {
                    bits.push(p(
                        if done { "已校时" } else { "校时未成" },
                        if done { "Clock set" } else { "Sync failed" },
                    ));
                }
            }
            Page::Gaming => {
                bits.push(if sys::game_mode() {
                    p("游戏模式开", "Game mode on")
                } else {
                    p("游戏模式关", "Game mode off")
                });
                if sys::game_flag("AppCaptureEnabled") {
                    bits.push(p("游戏栏", "Game bar"));
                }
            }
            Page::Notify => {
                bits.push(if sys::notify_global() {
                    p("允许通知", "On")
                } else {
                    p("通知关", "Off")
                });
                if sys::lock_toast() {
                    bits.push(p("锁屏可弹", "Above the lock"));
                }
            }
            Page::Focus => {
                bits.push(if !sys::notify_global() {
                    p("免打扰中", "Do not disturb")
                } else {
                    p("未开免打扰", "Silence off")
                });
            }
            Page::Access => {
                bits.push(if sys::high_contrast() {
                    p("高对比度开", "High contrast on")
                } else {
                    p("高对比度关", "High contrast off")
                });
                let share = crate::sys::text_scale();
                bits.push(format!("{} {share}%", p("文字", "Text")));
            }
            Page::Input => {
                let (pointer, wheel, _, _) = sys::read_input();
                bits.push(format!("{} {pointer}", p("指针", "Pointer")));
                bits.push(format!("{} {wheel}", p("滚轮", "Wheel")));
            }
            Page::Wallpaper => {
                let path = sys::wallpaper_path();
                if path.is_empty() {
                    bits.push(p("纯色", "A plain colour"));
                } else if let Some(name) = path.rsplit(['\\', '/']).next() {
                    bits.push(name.to_string());
                }
            }
            // The pages read of the machine, shown as the readings came in.
            Page::Sound => {
                let (pct, muted) = self.os.volume;
                bits.push(format!("{} {pct}%", p("音量", "Volume")));
                if muted {
                    bits.push(p("静音", "Muted"));
                }
                if let Some(out) = self.os.outs.iter().find(|out| out.given) {
                    bits.push(out.name.clone());
                }
            }
            Page::Battery => {
                match sys::battery_status() {
                    Some((pct, charging, mains)) => {
                        bits.push(format!("{pct}%"));
                        bits.push(if charging {
                            p("充电中", "Charging")
                        } else if mains {
                            p("接电源", "On mains")
                        } else {
                            p("用电池", "On battery")
                        });
                    }
                    None => bits.push(p("无电池", "No battery")),
                }
                if let Some(health) = self.os.health {
                    bits.push(format!("{} {health}%", p("健康", "Health")));
                }
                if let Some(name) = self
                    .os
                    .plan_now
                    .and_then(|now| self.os.plans.iter().find(|(plan, _)| *plan == now))
                    .map(|(_, name)| name.clone())
                {
                    bits.push(name);
                }
            }
            Page::Storage => {
                bits.push(format!(
                    "{} {}",
                    p("临时文件", "Temporary"),
                    crate::ui::text::bytes(self.os.temp as f64)
                ));
                bits.push(format!(
                    "{} {}",
                    p("回收站", "Recycle Bin"),
                    crate::ui::text::bytes(self.os.bin.0 as f64)
                ));
                if crate::sys::storage_sense() {
                    bits.push(p("存储感知开", "Storage sense on"));
                }
            }
            Page::Network => {
                if let Some(adapter) = self.os.adapters.first() {
                    bits.push(adapter.name.clone());
                    if let Some(ip) = &adapter.ipv4 {
                        bits.push(ip.clone());
                    }
                } else {
                    bits.push(p("正在读取…", "Reading…"));
                }
                if let Some(probe) = &self.os.probe {
                    let ms = probe
                        .mean
                        .map(|mean| format!("{mean} ms"))
                        .unwrap_or_else(|| "—".into());
                    bits.push(format!("{} {ms}", p("延迟", "Ping")));
                }
            }
            Page::Wifi => {
                if !self.os.wifi.on {
                    bits.push(p("无线关", "Radio off"));
                } else if let Some(net) = self.os.wifi.nets.iter().find(|net| net.connected) {
                    bits.push(net.ssid.clone());
                } else {
                    bits.push(p("未连接", "Not joined"));
                }
            }
            Page::Update => {
                bits.push(sys::windows_edition());
                bits.push(format!(
                    "{} {}",
                    p("已装更新", "Updates"),
                    self.os.patches.len()
                ));
            }
            Page::Account => {
                let name = std::env::var("USERNAME").unwrap_or_default();
                if !name.is_empty() {
                    bits.push(name);
                }
                bits.push(if crate::elevation::is_elevated() {
                    p("管理员", "Elevated")
                } else {
                    p("标准", "Standard")
                });
            }
            Page::About => {
                // The machine's name and its model, the first two facts read.
                for (_, value) in self.os.about.iter().take(2) {
                    bits.push(value.clone());
                }
            }
            Page::Startup => {
                if self.os.reading_startups.is_some() && self.os.startups.is_empty() {
                    bits.push(p("正在读取启动项…", "Reading the start-up apps…"));
                } else {
                    let on = self.os.startups.iter().filter(|app| app.enabled).count();
                    bits.push(format!(
                        "{} {} · {}",
                        p("允许自启", "Allowed"),
                        on,
                        self.os.startups.len()
                    ));
                    if let Some(app) = self.os.startups.iter().find(|app| app.enabled) {
                        bits.push(app.display.clone());
                    }
                }
            }
            Page::Bluetooth => {
                if self.os.bt_radios.is_empty() {
                    bits.push(p("无蓝牙无线电", "No Bluetooth radio"));
                } else {
                    bits.push(self.os.bt_radios.first().cloned().unwrap_or_default());
                    bits.push(if self.os.bt_discoverable {
                        p("可被发现", "Discoverable")
                    } else {
                        p("不易被发现", "Not discoverable")
                    });
                    if self.os.reading_bt.is_some() && self.os.bt_devices.is_empty() {
                        bits.push(p("正在读取设备…", "Reading devices…"));
                    } else {
                        bits.push(format!(
                            "{} {}",
                            p("已记住", "Remembered"),
                            self.os.bt_devices.len()
                        ));
                    }
                    if self.os.reading_bt_near.is_some() {
                        bits.push(p("正在搜索附近…", "Scanning…"));
                    } else if !self.os.bt_near.is_empty() {
                        bits.push(format!("{} {}", p("附近", "Nearby"), self.os.bt_near.len()));
                    }
                }
            }
            Page::Privacy => {
                let open = ["webcam", "microphone", "location"]
                    .iter()
                    .filter(|cap| crate::ui::system_settings::consent_on(cap))
                    .count();
                bits.push(format!(
                    "{} {} / 3",
                    p("已允许的访问", "Permits open"),
                    open
                ));
            }
            Page::Display => {
                let screens = &self.screens;
                bits.push(format!("{} {}", p("显示器", "Displays"), screens.len()));
                if let Some(primary) = screens.iter().find(|screen| screen.primary) {
                    let device = crate::sys::device_of(&primary.monitor);
                    if let Some(mode) = crate::sys::current(&device) {
                        bits.push(format!("{} × {} · {} Hz", mode.wide, mode.tall, mode.hz));
                    }
                }
                bits.push(format!("{} {}%", p("缩放", "Scale"), crate::sys::scale()));
                if crate::sys::night() > 0 {
                    bits.push(p("夜览开", "Night light"));
                }
            }
            // A page still being brought across, or one read of the system.
            _ => bits.push(p("实时读取", "A reading of the system")),
        }
        let caption = bits.join(" · ");
        frame.icon(
            self.page.icon(),
            Point {
                x: cx,
                y: area.y + area.h * 0.34,
            },
            44.0,
            palette.text,
        );
        let draw_centred = |s: &str, font: Font, color: Color, cy: f32| {
            let w = frame.gfx.measure(s, font).min((area.w - 32.0).max(1.0));
            text_centred(frame, s, font, color, cx - w / 2.0, cy, w, Align::Start);
        };
        draw_centred(
            self.page.name(lang),
            Font::new(Family::SegoeDisplay, 18.0, 600.0),
            palette.text,
            area.y + area.h * 0.58,
        );
        draw_centred(
            &caption,
            Font::new(Family::Segoe, 13.0, 400.0),
            palette.text2,
            area.y + area.h * 0.58 + 30.0,
        );
    }

    /// The machine as it is now: its own shape, the name it goes by, and what its
    /// parts are busy with — drawn as the panel's own modules draw them, the real cards
    /// with their charts. A desktop stands as a tower, a laptop as a book.
    fn paint_about_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
        let lang = self.lang;
        let about = self.os.about.clone();
        let zh = |z: &'static str, e: &'static str| pick(lang, z, e);
        let cx = area.x + area.w / 2.0;
        // The column the cards sit in: the area's width less a side margin, centred.
        let margin = 24.0;
        let col_w = (area.w - 2.0 * margin).max(240.0);
        let col_x = area.x + (area.w - col_w) / 2.0;
        let rx = col_x + col_w;
        let pad = 16.0;
        let line = 20.0;
        let row_gap = 13.0;
        let section_gap = 24.0;
        let label_font = Font::new(Family::Segoe, 12.5, 400.0);
        let value_font = Font::new(Family::Segoe, 12.5, 600.0);
        let title_font = Font::new(Family::Segoe, 13.0, 600.0);
        let btn_font = Font::new(Family::Segoe, 12.5, 600.0);
        let mut targets: Vec<(Rect, Target)> = Vec::new();

        // The preview runs past its pane, so it is clipped to the pane and the whole
        // flow is lifted by how far it has been scrolled down.
        let scroll = self.preview_scroll;
        frame.clip(area.x, area.y, area.w, area.h);

        // The machine drawn as it is, and the name it goes by under the model sold.
        let laptop =
            crate::smbios::system().laptop || crate::ui::system_settings::battery_status().is_some();
        crate::ui::system_settings::device_art(frame, cx - 48.0, area.y + 16.0 - scroll, laptop);
        let centred = |s: &str, font: Font, color: Color, cy: f32| {
            let m = frame.gfx.measure(s, font).min(col_w);
            text_centred(frame, s, font, color, cx - m / 2.0, cy, m, Align::Start);
        };
        let mut y = area.y + 16.0 + 78.0 - scroll;
        if let Some((_, value)) = about.get(1) {
            centred(
                value,
                Font::new(Family::SegoeDisplay, 17.0, 600.0),
                palette.text,
                y,
            );
            y += 24.0;
        }
        if let Some((_, value)) = about.first() {
            centred(
                value,
                Font::new(Family::Segoe, 12.5, 400.0),
                palette.text2,
                y,
            );
            y += 22.0;
        }

        // A grouped card of label/value rows, values wrapped to the column. Tells
        // back the y below the card, and the top of each row to hang a control on.
        let card = |y: f32, rows: &[(String, String)]| -> (f32, Vec<f32>) {
            if rows.is_empty() {
                return (y, Vec::new());
            }
            let lx = col_x + pad;
            let label_w = rows
                .iter()
                .map(|(l, _)| frame.gfx.measure(l, label_font))
                .fold(0.0f32, f32::max);
            let value_x = lx + label_w + 16.0;
            let value_w = (rx - pad - value_x).max(60.0);
            let wrapped: Vec<(String, Vec<String>)> = rows
                .iter()
                .map(|(l, v)| (l.clone(), wrap_text(frame, v, value_font, value_w)))
                .collect();
            let lines: usize = wrapped.iter().map(|(_, ls)| ls.len()).sum();
            let h = 2.0 * pad + lines as f32 * line + (rows.len() as f32 - 1.0) * row_gap;
            fill(frame, palette.card, col_x, y, col_w, h, 10.0);
            stroke_inside(
                frame,
                Rect {
                    x: col_x,
                    y,
                    w: col_w,
                    h,
                },
                10.0,
                palette.card_stroke,
            );
            let mut ry = y + pad;
            let mut tops = Vec::new();
            for (label, ls) in &wrapped {
                tops.push(ry);
                text_centred(
                    frame,
                    label,
                    label_font,
                    palette.text2,
                    lx,
                    ry + line / 2.0,
                    label_w,
                    Align::Start,
                );
                for (k, said) in ls.iter().enumerate() {
                    let vw = frame.gfx.measure(said, value_font).min(value_w);
                    text_centred(
                        frame,
                        said,
                        value_font,
                        palette.text,
                        rx - pad - vw,
                        ry + k as f32 * line + line / 2.0,
                        vw,
                        Align::Start,
                    );
                }
                ry += ls.len() as f32 * line + row_gap;
            }
            (y + h, tops)
        };
        // A section's heading, set above its card.
        let heading = |y: f32, text: &str| {
            text_centred(
                frame,
                text,
                title_font,
                palette.text,
                col_x,
                y + 10.0,
                col_w,
                Align::Start,
            );
            y + 26.0
        };
        let fact = |i: usize| about.get(i).map(|(_, v)| v.clone()).unwrap_or_default();

        // 概览: the machine's own facts, and the coverage Glance cannot look up, told
        // with a way into Windows' full report.
        let overview = vec![
            (zh("名称", "Name").to_string(), fact(0)),
            (zh("型号", "Model").to_string(), fact(1)),
            (zh("处理器", "Processor").to_string(), fact(3)),
            (zh("图形卡", "Graphics").to_string(), fact(6)),
            (zh("内存", "Memory").to_string(), fact(4)),
            (zh("主板", "Motherboard").to_string(), fact(5)),
            (zh("网卡", "Network").to_string(), fact(9)),
            (zh("序列号", "Serial number").to_string(), fact(10)),
            (zh("保修与保障", "Warranty").to_string(), String::new()),
        ];
        let (bottom, tops) = card(y, &overview);
        if let Some(&top) = tops.last() {
            let label = zh("详细信息…", "Details…");
            let bw = frame.gfx.measure(label, btn_font) + 24.0;
            let bh = 22.0;
            let bx = rx - pad - bw;
            let by = top + (line - bh) / 2.0;
            fill(frame, palette.control, bx, by, bw, bh, 6.0);
            stroke_inside(
                frame,
                Rect {
                    x: bx,
                    y: by,
                    w: bw,
                    h: bh,
                },
                6.0,
                palette.card_stroke,
            );
            text_centred(
                frame,
                label,
                btn_font,
                palette.text,
                bx + 12.0,
                by + bh / 2.0,
                bw - 24.0,
                Align::Start,
            );
            targets.push((
                Rect {
                    x: bx,
                    y: by,
                    w: bw,
                    h: bh,
                },
                Target::Jump("msinfo32"),
            ));
        }
        y = bottom + section_gap;

        // The Windows it runs, and the firmware it starts on.
        y = heading(y, zh("系统", "System"));
        let system = vec![
            (zh("版本", "Version").to_string(), fact(2)),
            (zh("BIOS 版本", "BIOS").to_string(), fact(11)),
        ];
        y = card(y, &system).0 + section_gap;

        // Each screen, as it is wired now: its size and the scale it was made for.
        y = heading(y, zh("显示器", "Displays"));
        let screens = crate::ui::system_settings::about_screens();
        let mut displays: Vec<(String, String)> = screens
            .iter()
            .enumerate()
            .map(|(i, (primary, size, scale))| {
                let name = if *primary {
                    zh("主显示器", "Main display").to_string()
                } else {
                    format!("{} {}", zh("显示器", "Display"), i + 1)
                };
                (name, format!("{size} · {scale}"))
            })
            .collect();
        if displays.is_empty() {
            displays.push((zh("显示器", "Display").to_string(), fact(8)));
        }
        y = card(y, &displays).0 + section_gap;

        // Each drive mounted with a letter, and the room it has taken and holds.
        y = heading(y, zh("存储", "Storage"));
        let mut store: Vec<(String, String)> = crate::ui::system_settings::drive_space()
            .into_iter()
            .map(|(name, used, total)| (name, crate::ui::text::usage(used, total)))
            .collect();
        if store.is_empty() {
            store.push((zh("硬盘", "Storage").to_string(), fact(7)));
        }
        y = card(y, &store).0;

        // The ways on, set just under the facts so no empty band is left below them:
        // the screens and the storage lead into this window's own pages, the whole
        // technical report to Windows'.
        y += section_gap;
        let items = [
            (zh("显示器设置", "Displays"), Target::Nav(Page::Display)),
            (zh("储存空间设置", "Storage"), Target::Nav(Page::Storage)),
            (zh("系统报告", "System report"), Target::Jump("msinfo32")),
        ];
        let gap = 10.0;
        let bh = 30.0;
        let widths: Vec<f32> = items
            .iter()
            .map(|(label, _)| frame.gfx.measure(label, btn_font) + 28.0)
            .collect();
        let total: f32 = widths.iter().sum::<f32>() + gap * (items.len() as f32 - 1.0);
        let mut bx = cx - total / 2.0;
        let by = y;
        for ((label, target), bw) in items.into_iter().zip(widths) {
            fill(frame, palette.control, bx, by, bw, bh, 8.0);
            stroke_inside(
                frame,
                Rect {
                    x: bx,
                    y: by,
                    w: bw,
                    h: bh,
                },
                8.0,
                palette.card_stroke,
            );
            text_centred(
                frame,
                label,
                btn_font,
                palette.text,
                bx + 14.0,
                by + bh / 2.0,
                bw - 28.0,
                Align::Start,
            );
            targets.push((
                Rect {
                    x: bx,
                    y: by,
                    w: bw,
                    h: bh,
                },
                target,
            ));
            bx += bw + gap;
        }

        // The whole flow's height, told back so the wheel knows how far down to go.
        // A bottom margin is reserved so the buttons clear the pane's lower edge.
        let bottom_gap = 20.0;
        self.preview_content_h = y + bh + bottom_gap + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }

    /// Windows Update as it stands: the edition it runs, how many fixes are in and
    /// the newest of them, then the list of what went in — read in the background,
    /// so it says so while they come. Drawn as the 关于本机 preview is, a column of
    /// grouped cards that runs past the pane and is scrolled by the wheel.
    fn paint_update_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
        use crate::ui::system_settings as sys;
        let lang = self.lang;
        let p = |zh: &'static str, en: &'static str| pick(lang, zh, en).to_string();
        let cx = area.x + area.w / 2.0;
        let margin = 24.0;
        let col_w = (area.w - 2.0 * margin).max(240.0);
        let col_x = area.x + (area.w - col_w) / 2.0;
        let rx = col_x + col_w;
        let pad = 14.0;
        let line = 18.0;
        let row_gap = 8.0;
        let section_gap = 18.0;
        let label_font = Font::new(Family::Segoe, 12.5, 400.0);
        let value_font = Font::new(Family::Segoe, 12.5, 600.0);
        let title_font = Font::new(Family::Segoe, 13.0, 600.0);
        let btn_font = Font::new(Family::Segoe, 12.5, 600.0);
        let mut targets: Vec<(Rect, Target)> = Vec::new();

        // The list runs past its pane, so it is clipped to the pane and lifted by the
        // scroll.
        let scroll = self.preview_scroll;
        frame.clip(area.x, area.y, area.w, area.h);

        // The mark, its name, and the Windows it runs.
        frame.icon(
            Page::Update.icon(),
            Point {
                x: cx,
                y: area.y + 40.0 - scroll,
            },
            40.0,
            palette.text,
        );
        let centred = |s: &str, font: Font, color: Color, cy: f32| {
            let m = frame.gfx.measure(s, font).min(col_w);
            text_centred(frame, s, font, color, cx - m / 2.0, cy, m, Align::Start);
        };
        let mut y = area.y + 40.0 + 44.0 - scroll;
        centred(
            &p("软件更新", "Software Update"),
            Font::new(Family::SegoeDisplay, 17.0, 600.0),
            palette.text,
            y,
        );
        y += 24.0;
        let edition = sys::windows_edition();
        if !edition.is_empty() {
            centred(&edition, label_font, palette.text2, y);
            y += 22.0;
        }

        // A grouped card of label/value rows, values wrapped to the column.
        let card = |y: f32, rows: &[(String, String)]| -> (f32, Vec<f32>) {
            if rows.is_empty() {
                return (y, Vec::new());
            }
            let lx = col_x + pad;
            let label_w = rows
                .iter()
                .map(|(l, _)| frame.gfx.measure(l, label_font))
                .fold(0.0f32, f32::max);
            let value_x = lx + label_w + 16.0;
            let value_w = (rx - pad - value_x).max(60.0);
            let wrapped: Vec<(String, Vec<String>)> = rows
                .iter()
                .map(|(l, v)| (l.clone(), wrap_text(frame, v, value_font, value_w)))
                .collect();
            let lines: usize = wrapped.iter().map(|(_, ls)| ls.len()).sum();
            let h = 2.0 * pad + lines as f32 * line + (rows.len() as f32 - 1.0) * row_gap;
            fill(frame, palette.card, col_x, y, col_w, h, 10.0);
            stroke_inside(
                frame,
                Rect {
                    x: col_x,
                    y,
                    w: col_w,
                    h,
                },
                10.0,
                palette.card_stroke,
            );
            let mut ry = y + pad;
            let mut tops = Vec::new();
            for (label, ls) in &wrapped {
                tops.push(ry);
                if !label.is_empty() {
                    text_centred(
                        frame,
                        label,
                        label_font,
                        palette.text2,
                        lx,
                        ry + line / 2.0,
                        label_w,
                        Align::Start,
                    );
                }
                for (k, said) in ls.iter().enumerate() {
                    let vw = frame.gfx.measure(said, value_font).min(value_w);
                    text_centred(
                        frame,
                        said,
                        value_font,
                        palette.text,
                        rx - pad - vw,
                        ry + k as f32 * line + line / 2.0,
                        vw,
                        Align::Start,
                    );
                }
                ry += ls.len() as f32 * line + row_gap;
            }
            (y + h, tops)
        };
        let heading = |y: f32, text: &str| {
            text_centred(
                frame,
                text,
                title_font,
                palette.text,
                col_x,
                y + 10.0,
                col_w,
                Align::Start,
            );
            y + 26.0
        };

        // The state of the update: what Windows, how many fixes, and the newest date.
        y = heading(y, &p("Windows 更新", "Windows Update"));
        let latest = self
            .os
            .patches
            .first()
            .map(|x| x.when.clone())
            .unwrap_or_default();
        let facts = vec![
            (p("版本", "Edition"), edition.clone()),
            (
                p("已装更新", "Installed"),
                format!("{}", self.os.patches.len()),
            ),
            (p("最近更新", "Latest"), latest),
        ];
        y = card(y, &facts).0 + section_gap;

        // The fixes read, newest first; saying so while they are still coming in.
        y = heading(y, &p("已安装更新", "Installed updates"));
        let list: Vec<(String, String)> = if self.os.patches.is_empty() {
            vec![(
                String::new(),
                if self.os.reading_patches.is_some() {
                    p("正在读取已安装的更新…", "Reading the updates installed…")
                } else {
                    p("没读到的更新。", "No updates were read.")
                },
            )]
        } else {
            self.os
                .patches
                .iter()
                .take(20)
                .map(|x| {
                    let what = if x.what.is_empty() {
                        x.id.clone()
                    } else {
                        format!("{} · {}", x.id, x.what)
                    };
                    (x.when.clone(), what)
                })
                .collect()
        };
        y = card(y, &list).0;

        // The way into Windows' own page, set under the facts.
        y += section_gap;
        let label = p("Windows 更新…", "Windows Update…");
        let bw = frame.gfx.measure(&label, btn_font) + 28.0;
        let bh = 30.0;
        let bx = cx - bw / 2.0;
        let by = y;
        fill(frame, palette.control, bx, by, bw, bh, 8.0);
        stroke_inside(
            frame,
            Rect {
                x: bx,
                y: by,
                w: bw,
                h: bh,
            },
            8.0,
            palette.card_stroke,
        );
        text_centred(
            frame,
            &label,
            btn_font,
            palette.text,
            bx + 14.0,
            by + bh / 2.0,
            bw - 28.0,
            Align::Start,
        );
        targets.push((
            Rect {
                x: bx,
                y: by,
                w: bw,
                h: bh,
            },
            Target::Jump("ms-settings:windowsupdate"),
        ));

        let bottom_gap = 20.0;
        self.preview_content_h = by + bh + bottom_gap + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }

    /// The room the drives hold and have taken, and what could be cleared: drawn as
    /// the 关于本机 and 软件更新 previews are — a column of grouped cards that runs
    /// past the pane and is scrolled by the wheel. The drives and their use are the
    /// heart of it, which a lone line of words left out.
    fn paint_storage_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
        let lang = self.lang;
        let p = |zh: &'static str, en: &'static str| pick(lang, zh, en).to_string();
        let cx = area.x + area.w / 2.0;
        let margin = 24.0;
        let col_w = (area.w - 2.0 * margin).max(240.0);
        let col_x = area.x + (area.w - col_w) / 2.0;
        let rx = col_x + col_w;
        let pad = 14.0;
        let line = 18.0;
        let row_gap = 8.0;
        let section_gap = 18.0;
        let label_font = Font::new(Family::Segoe, 12.5, 400.0);
        let value_font = Font::new(Family::Segoe, 12.5, 600.0);
        let title_font = Font::new(Family::Segoe, 13.0, 600.0);
        let btn_font = Font::new(Family::Segoe, 12.5, 600.0);
        let mut targets: Vec<(Rect, Target)> = Vec::new();

        let scroll = self.preview_scroll;
        frame.clip(area.x, area.y, area.w, area.h);

        // The mark and its name.
        frame.icon(
            Page::Storage.icon(),
            Point {
                x: cx,
                y: area.y + 40.0 - scroll,
            },
            40.0,
            palette.text,
        );
        let centred = |s: &str, font: Font, color: Color, cy: f32| {
            let m = frame.gfx.measure(s, font).min(col_w);
            text_centred(frame, s, font, color, cx - m / 2.0, cy, m, Align::Start);
        };
        let mut y = area.y + 40.0 + 44.0 - scroll;
        centred(
            &p("存储", "Storage"),
            Font::new(Family::SegoeDisplay, 17.0, 600.0),
            palette.text,
            y,
        );
        y += 34.0;

        // A grouped card of label/value rows, values wrapped to the column.
        let card = |y: f32, rows: &[(String, String)]| -> (f32, Vec<f32>) {
            if rows.is_empty() {
                return (y, Vec::new());
            }
            let lx = col_x + pad;
            let label_w = rows
                .iter()
                .map(|(l, _)| frame.gfx.measure(l, label_font))
                .fold(0.0f32, f32::max);
            let value_x = lx + label_w + 16.0;
            let value_w = (rx - pad - value_x).max(60.0);
            let wrapped: Vec<(String, Vec<String>)> = rows
                .iter()
                .map(|(l, v)| (l.clone(), wrap_text(frame, v, value_font, value_w)))
                .collect();
            let lines: usize = wrapped.iter().map(|(_, ls)| ls.len()).sum();
            let h = 2.0 * pad + lines as f32 * line + (rows.len() as f32 - 1.0) * row_gap;
            fill(frame, palette.card, col_x, y, col_w, h, 10.0);
            stroke_inside(
                frame,
                Rect {
                    x: col_x,
                    y,
                    w: col_w,
                    h,
                },
                10.0,
                palette.card_stroke,
            );
            let mut ry = y + pad;
            let mut tops = Vec::new();
            for (label, ls) in &wrapped {
                tops.push(ry);
                if !label.is_empty() {
                    text_centred(
                        frame,
                        label,
                        label_font,
                        palette.text2,
                        lx,
                        ry + line / 2.0,
                        label_w,
                        Align::Start,
                    );
                }
                for (k, said) in ls.iter().enumerate() {
                    let vw = frame.gfx.measure(said, value_font).min(value_w);
                    text_centred(
                        frame,
                        said,
                        value_font,
                        palette.text,
                        rx - pad - vw,
                        ry + k as f32 * line + line / 2.0,
                        vw,
                        Align::Start,
                    );
                }
                ry += ls.len() as f32 * line + row_gap;
            }
            (y + h, tops)
        };
        let heading = |y: f32, text: &str| {
            text_centred(
                frame,
                text,
                title_font,
                palette.text,
                col_x,
                y + 10.0,
                col_w,
                Align::Start,
            );
            y + 26.0
        };

        // Each drive mounted with a letter, and the room it has taken and holds.
        let room = crate::ui::system_settings::drive_space();
        y = heading(y, &p("磁盘", "Drives"));
        let drives: Vec<(String, String)> = if room.is_empty() {
            vec![(
                String::new(),
                p("没读到可用的磁盘。", "No drive could be read."),
            )]
        } else {
            room.iter()
                .map(|(name, used, total)| (name.clone(), crate::ui::text::usage(*used, *total)))
                .collect()
        };
        y = card(y, &drives).0 + section_gap;

        // The whole across every drive, and what could be cleared from it.
        y = heading(y, &p("占用与清理", "Used and to clear"));
        let sum: (u64, u64) = room
            .iter()
            .fold((0, 0), |(u, t), (_, used, total)| (u + used, t + total));
        let mut clear: Vec<(String, String)> = Vec::new();
        if sum.1 > 0 {
            clear.push((p("总计", "Total"), crate::ui::text::usage(sum.0, sum.1)));
        }
        clear.push((
            p("临时文件", "Temporary"),
            crate::ui::text::bytes(self.os.temp as f64),
        ));
        clear.push((
            p("回收站", "Recycle Bin"),
            format!(
                "{} · {}",
                self.os.bin.1,
                crate::ui::text::bytes(self.os.bin.0 as f64)
            ),
        ));
        clear.push((
            p("存储感知", "Storage sense"),
            if crate::sys::storage_sense() {
                p("开", "On")
            } else {
                p("关", "Off")
            },
        ));
        y = card(y, &clear).0;

        // The way into Windows' own storage page.
        y += section_gap;
        let label = p("存储设置…", "Storage settings…");
        let bw = frame.gfx.measure(&label, btn_font) + 28.0;
        let bh = 30.0;
        let bx = cx - bw / 2.0;
        let by = y;
        fill(frame, palette.control, bx, by, bw, bh, 8.0);
        stroke_inside(
            frame,
            Rect {
                x: bx,
                y: by,
                w: bw,
                h: bh,
            },
            8.0,
            palette.card_stroke,
        );
        text_centred(
            frame,
            &label,
            btn_font,
            palette.text,
            bx + 14.0,
            by + bh / 2.0,
            bw - 28.0,
            Align::Start,
        );
        targets.push((
            Rect {
                x: bx,
                y: by,
                w: bw,
                h: bh,
            },
            Target::Jump("ms-settings:sense"),
        ));

        let bottom_gap = 20.0;
        self.preview_content_h = by + bh + bottom_gap + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }
}
