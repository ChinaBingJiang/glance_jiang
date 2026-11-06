//! The stage drawn for the screens themselves, and for the time spent looking at them.

use super::*;

impl Ui {
    /// The displays as macOS shows one: the screen drawn to its own shape on a
    /// little stand with its name under it, the resolutions it offers as a row of
    /// cards you choose between (the one worn now marked), and a short card of the
    /// scale, the refresh and the night light. Brightness, the colour profile,
    /// rotation and HDR are Windows' own to set, so this says so and links there
    /// rather than dressing up a control it cannot honour.
    pub(super) fn paint_display_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
        let lang = self.lang;
        let p = |zh: &'static str, en: &'static str| pick(lang, zh, en).to_string();
        let margin = 24.0;
        let col_w = (area.w - 2.0 * margin).max(300.0);
        let col_x = area.x + (area.w - col_w) / 2.0;
        let rx = col_x + col_w;
        let cx = area.x + area.w / 2.0;
        let pad = 14.0;
        let line = 18.0;
        let row_gap = 8.0;
        let section_gap = 18.0;
        let label_font = Font::new(Family::Segoe, 12.5, 400.0);
        let value_font = Font::new(Family::Segoe, 12.5, 600.0);
        let title_font = Font::new(Family::Segoe, 13.0, 600.0);
        let btn_font = Font::new(Family::Segoe, 12.0, 600.0);
        let scroll = self.preview_scroll;
        let mut targets: Vec<(Rect, Target)> = Vec::new();
        frame.clip(area.x, area.y, area.w, area.h);

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
        // A card of label/value lines; a blank label lets a line stand alone.
        let plain_card = |y: f32, rows: &[(String, String)]| -> f32 {
            if rows.is_empty() {
                return y;
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
            for (label, ls) in &wrapped {
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
            y + h
        };

        let screens = self.screens.clone();
        if screens.is_empty() {
            let mut y = area.y + 34.0 - scroll;
            y = heading(y, &p("显示器", "Displays"));
            y = plain_card(
                y,
                &[(String::new(), p("没有读到显示器。", "No display was read."))],
            );
            self.preview_content_h = y + 20.0 + scroll - area.y;
            frame.unclip();
            self.targets.extend(targets);
            return;
        }

        // The one shown: the picked index, else the main, else the first.
        let idx = self
            .os
            .disp
            .filter(|i| *i < screens.len())
            .unwrap_or_else(|| screens.iter().position(|s| s.primary).unwrap_or(0));
        let screen = &screens[idx];
        let device = crate::sys::device_of(&screen.monitor);
        let here = crate::sys::current(&device);
        let modes = crate::sys::modes(&device);

        let mut y = area.y + 34.0 - scroll;

        // The screen drawn to its own shape, in a bezel on a small stand.
        let (aw, ah) = here
            .map(|m| (m.wide as f32, m.tall as f32))
            .unwrap_or_else(|| {
                (
                    (screen.monitor.right - screen.monitor.left).max(1) as f32,
                    (screen.monitor.bottom - screen.monitor.top).max(1) as f32,
                )
            });
        let mon_w = col_w.min(300.0);
        let mon_h = mon_w * ah / aw;
        let bezel = 8.0;
        let box_w = mon_w + 2.0 * bezel;
        let box_h = mon_h + 2.0 * bezel;
        let bx = cx - box_w / 2.0;
        fill(frame, palette.control, bx, y, box_w, box_h, 10.0);
        stroke_inside(
            frame,
            Rect {
                x: bx,
                y,
                w: box_w,
                h: box_h,
            },
            10.0,
            palette.card_stroke,
        );
        // The glass: the screen's own desktop, read as the layout's editor reads it,
        // so the preview shows the background it really wears; a plain field with a
        // hairline along its top until that desktop comes.
        let sx = bx + bezel;
        let sy = y + bezel;
        let glass = Rect {
            x: sx,
            y: sy,
            w: mon_w,
            h: mon_h,
        };
        let pixels = (
            (mon_w * self.scale).round().max(1.0) as u32,
            (mon_h * self.scale).round().max(1.0) as u32,
        );
        match self.map_picture(frame, &screen.id, screen.monitor, pixels) {
            Some(picture) => picture_rounded(frame, &picture, glass, 4.0, 1.0),
            None => {
                fill(frame, palette.card, sx, sy, mon_w, mon_h, 4.0);
                fill(frame, palette.switch_on, sx, sy, mon_w, 2.0, 1.0);
            }
        }
        // The stand.
        let neck_w = 44.0;
        let neck_h = 14.0;
        fill(
            frame,
            palette.control,
            cx - neck_w / 2.0,
            y + box_h,
            neck_w,
            neck_h,
            0.0,
        );
        let foot_w = 96.0;
        let foot_h = 6.0;
        fill(
            frame,
            palette.control,
            cx - foot_w / 2.0,
            y + box_h + neck_h,
            foot_w,
            foot_h,
            3.0,
        );
        y += box_h + neck_h + foot_h + 16.0;

        // Its name, then the mode it wears now.
        let name = format!(
            "{} {}{}",
            p("显示器", "Display"),
            idx + 1,
            if screen.primary {
                p(" · 主", " · Main")
            } else {
                String::new()
            }
        );
        let head = Font::new(Family::SegoeDisplay, 16.0, 600.0);
        let nm = frame.gfx.measure(&name, head).min(col_w);
        text_centred(
            frame,
            &name,
            head,
            palette.text,
            cx - nm / 2.0,
            y,
            nm,
            Align::Start,
        );
        y += 22.0;
        if let Some(m) = here {
            let said = format!("{} × {} · {} Hz", m.wide, m.tall, m.hz);
            let sw = frame.gfx.measure(&said, label_font).min(col_w);
            text_centred(
                frame,
                &said,
                label_font,
                palette.text2,
                cx - sw / 2.0,
                y,
                sw,
                Align::Start,
            );
            y += 20.0;
        }
        y += 6.0;

        // More than one: a chip each to bring the other forward.
        if screens.len() > 1 {
            let chip = 30.0;
            let gap = 10.0;
            let total = screens.len() as f32 * chip + (screens.len() as f32 - 1.0) * gap;
            let mut chx = cx - total / 2.0;
            for (i, _) in screens.iter().enumerate() {
                let lit = i == idx;
                fill(
                    frame,
                    if lit {
                        palette.selection
                    } else {
                        palette.control
                    },
                    chx,
                    y,
                    chip,
                    chip,
                    8.0,
                );
                stroke_inside(
                    frame,
                    Rect {
                        x: chx,
                        y,
                        w: chip,
                        h: chip,
                    },
                    8.0,
                    palette.card_stroke,
                );
                let lab = format!("{}", i + 1);
                let lw = frame.gfx.measure(&lab, value_font);
                text_centred(
                    frame,
                    &lab,
                    value_font,
                    if lit { palette.text } else { palette.text2 },
                    chx + (chip - lw) / 2.0,
                    y + chip / 2.0,
                    lw,
                    Align::Start,
                );
                if !lit {
                    targets.push((
                        Rect {
                            x: chx,
                            y,
                            w: chip,
                            h: chip,
                        },
                        Target::DispPick(i),
                    ));
                }
                chx += chip + gap;
            }
            y += chip + 16.0;
        }

        // The resolutions it offers, as cards you choose between, current marked.
        if !modes.is_empty() {
            y = heading(y, &p("分辨率", "Resolution"));
            let per = 3usize;
            let gap = 12.0;
            let card_w = (col_w - (per as f32 - 1.0) * gap) / per as f32;
            let card_h = 84.0;
            let count = modes.len().min(9);
            for (n, mode) in modes.iter().take(count).enumerate() {
                let col = n % per;
                let row = n / per;
                let cx0 = col_x + col as f32 * (card_w + gap);
                let cy0 = y + row as f32 * (card_h + gap);
                let on = here == Some(*mode);
                fill(frame, palette.card, cx0, cy0, card_w, card_h, 10.0);
                stroke_inside(
                    frame,
                    Rect {
                        x: cx0,
                        y: cy0,
                        w: card_w,
                        h: card_h,
                    },
                    10.0,
                    if on {
                        palette.selection
                    } else {
                        palette.card_stroke
                    },
                );
                // A miniature of the screen to this mode's shape.
                let tw = card_w - 20.0;
                let th = (tw * mode.tall as f32 / mode.wide.max(1) as f32).min(34.0);
                let tx = cx0 + (card_w - tw) / 2.0;
                let ty = cy0 + 10.0;
                fill(frame, palette.control, tx, ty, tw, th, 3.0);
                fill(
                    frame,
                    if on { palette.selection } else { palette.text3 },
                    tx,
                    ty,
                    tw,
                    2.0,
                    1.0,
                );
                let lab = format!("{} × {}", mode.wide, mode.tall);
                let lw = frame.gfx.measure(&lab, value_font).min(card_w - 8.0);
                text_centred(
                    frame,
                    &lab,
                    value_font,
                    if on { palette.selection } else { palette.text },
                    cx0 + (card_w - lw) / 2.0,
                    ty + th + 14.0,
                    lw,
                    Align::Start,
                );
                let hz = format!("{} Hz", mode.hz);
                let hw = frame.gfx.measure(&hz, label_font).min(card_w - 8.0);
                text_centred(
                    frame,
                    &hz,
                    label_font,
                    palette.text2,
                    cx0 + (card_w - hw) / 2.0,
                    ty + th + 30.0,
                    hw,
                    Align::Start,
                );
                targets.push((
                    Rect {
                        x: cx0,
                        y: cy0,
                        w: card_w,
                        h: card_h,
                    },
                    Target::SetMode(device.clone(), *mode),
                ));
            }
            let rows = (count + per - 1) / per;
            y += rows as f32 * (card_h + gap) - gap + section_gap;
        }

        // The scale, the refresh and the night light, as they stand now.
        y = heading(y, &p("设置", "Settings"));
        let scale = crate::sys::scale();
        let refresh = here
            .map(|m| format!("{} Hz", m.hz))
            .unwrap_or_else(|| "—".into());
        let night = if crate::sys::night() > 0 {
            p("开", "On")
        } else {
            p("关", "Off")
        };
        y = plain_card(
            y,
            &[
                (p("缩放", "Scale"), format!("{scale}%")),
                (p("刷新率", "Refresh"), refresh),
                (p("夜览", "Night light"), night),
            ],
        ) + section_gap;

        // What Windows keeps: brightness, colour, rotation, HDR.
        y = heading(y, &p("亮度与颜色", "Brightness and colour"));
        y = plain_card(
            y,
            &[(
                String::new(),
                p(
                    "亮度、色彩描述文件、旋转与高动态范围仍由 Windows 管理。",
                    "Brightness, the colour profile, rotation and HDR stay Windows' own to set.",
                ),
            )],
        ) + 14.0;
        // A button into Windows' own display page.
        {
            let label = p("显示设置…", "Display settings…");
            let bw = frame.gfx.measure(&label, btn_font) + 28.0;
            let bh = 30.0;
            fill(frame, palette.control, col_x, y, bw, bh, 8.0);
            stroke_inside(
                frame,
                Rect {
                    x: col_x,
                    y,
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
                col_x + 14.0,
                y + bh / 2.0,
                bw - 28.0,
                Align::Start,
            );
            targets.push((
                Rect {
                    x: col_x,
                    y,
                    w: bw,
                    h: bh,
                },
                Target::Jump("ms-settings:display"),
            ));
            y += bh;
        }

        self.preview_content_h = y + 20.0 + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }

    /// 屏幕使用时间 as Glance has gathered it: today told large, the last seven days
    /// as bars, and the apps that kept the screen — read from its own tally, not
    /// Windows'. Drawn as the other rich previews are, a column that runs past the
    /// pane and is scrolled by the wheel.
    pub(super) fn paint_screen_time_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
        let lang = self.lang;
        let zh = lang == Lang::Zh;
        let p = |a: &'static str, b: &'static str| pick(lang, a, b).to_string();
        let margin = 24.0;
        let col_w = (area.w - 2.0 * margin).max(300.0);
        let col_x = area.x + (area.w - col_w) / 2.0;
        let rx = col_x + col_w;
        let cx = area.x + area.w / 2.0;
        let pad = 14.0;
        let line = 18.0;
        let row_gap = 8.0;
        let section_gap = 18.0;
        let label_font = Font::new(Family::Segoe, 12.5, 400.0);
        let value_font = Font::new(Family::Segoe, 12.5, 600.0);
        let title_font = Font::new(Family::Segoe, 13.0, 600.0);
        let big_font = Font::new(Family::SegoeDisplay, 30.0, 600.0);
        let scroll = self.preview_scroll;
        frame.clip(area.x, area.y, area.w, area.h);

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
        // A card of label/value lines; a blank label lets a line stand alone.
        let plain_card = |y: f32, rows: &[(String, String)]| -> f32 {
            if rows.is_empty() {
                return y;
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
            for (label, ls) in &wrapped {
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
            y + h
        };

        let (total, apps) = crate::ui::screen_time::today_usage();
        let week = crate::ui::screen_time::week();
        let front = crate::ui::screen_time::foreground();
        let yest = crate::ui::screen_time::yesterday_usage();

        let mut y = area.y + 34.0 - scroll;

        // Today told large, with yesterday and what is in front just under it.
        y = heading(y, &p("今天", "Today"));
        let said = crate::ui::screen_time::say(total, zh);
        let sw = frame.gfx.measure(&said, big_font).min(col_w);
        text_centred(
            frame,
            &said,
            big_font,
            palette.text,
            cx - sw / 2.0,
            y + 20.0,
            sw,
            Align::Start,
        );
        y += 46.0;
        let under = format!(
            "{} {} · {} {} · {} {}",
            p("昨天", "yesterday"),
            crate::ui::screen_time::say(yest, zh),
            p("当前", "now"),
            if front.is_empty() {
                p("无", "none")
            } else {
                front.clone()
            },
            apps.len(),
            p("款应用", "apps")
        );
        for l in wrap_text(frame, &under, label_font, col_w) {
            let lw = frame.gfx.measure(&l, label_font).min(col_w);
            text_centred(
                frame,
                &l,
                label_font,
                palette.text2,
                cx - lw / 2.0,
                y + line / 2.0,
                lw,
                Align::Start,
            );
            y += line;
        }
        y += section_gap;

        // The last seven days as bars, today's lit in the accent.
        y = heading(y, &p("过去 7 天", "The last seven days"));
        let chart_h = 150.0;
        fill(frame, palette.card, col_x, y, col_w, chart_h, 10.0);
        stroke_inside(
            frame,
            Rect {
                x: col_x,
                y,
                w: col_w,
                h: chart_h,
            },
            10.0,
            palette.card_stroke,
        );
        let inner_x = col_x + pad;
        let inner_w = col_w - 2.0 * pad;
        let n = week.len();
        if n > 0 {
            let slot = inner_w / n as f32;
            let bar_w = (slot * 0.46).min(30.0);
            let top = y + pad;
            let base = y + chart_h - pad - 18.0;
            let area_h = (base - top).max(1.0);
            let max = week.iter().map(|(_, s)| *s).max().unwrap_or(0).max(1) as f32;
            for (i, (day, secs)) in week.iter().enumerate() {
                let today = i + 1 == n;
                let h = (*secs as f32 / max * area_h).max(if *secs > 0 { 3.0 } else { 0.0 });
                let lx = inner_x + i as f32 * slot;
                let bx = lx + (slot - bar_w) / 2.0;
                fill(
                    frame,
                    if today {
                        palette.selection
                    } else {
                        palette.control
                    },
                    bx,
                    base - h,
                    bar_w,
                    h,
                    3.0,
                );
                let lab = crate::ui::screen_time::label(*day);
                let lw = frame.gfx.measure(&lab, label_font);
                text_centred(
                    frame,
                    &lab,
                    label_font,
                    if today { palette.text } else { palette.text2 },
                    lx + (slot - lw) / 2.0,
                    base + 12.0,
                    lw,
                    Align::Start,
                );
            }
        }
        y += chart_h + section_gap;

        // The apps that kept the screen today, each with a bar to its share.
        y = heading(y, &p("最常用的应用", "Most used"));
        let shown = apps.iter().take(6).count();
        if shown == 0 {
            y = plain_card(
                y,
                &[(
                    String::new(),
                    p(
                        "还没有记录到使用；用一会儿再来。",
                        "Nothing gathered yet; use the PC a while.",
                    ),
                )],
            ) + section_gap;
        } else {
            let pitch = 30.0;
            let card_h = 2.0 * pad + shown as f32 * pitch;
            fill(frame, palette.card, col_x, y, col_w, card_h, 10.0);
            stroke_inside(
                frame,
                Rect {
                    x: col_x,
                    y,
                    w: col_w,
                    h: card_h,
                },
                10.0,
                palette.card_stroke,
            );
            let name_w = 120.0;
            let mut ry = y + pad;
            for (app, secs) in apps.iter().take(6) {
                let t = crate::ui::screen_time::say(*secs, zh);
                let tw = frame.gfx.measure(&t, value_font);
                text_centred(
                    frame,
                    app,
                    label_font,
                    palette.text,
                    col_x + pad,
                    ry + line / 2.0,
                    name_w,
                    Align::Start,
                );
                let track_x = col_x + pad + name_w + 10.0;
                let track_w = (rx - pad - tw - 12.0 - track_x).max(20.0);
                let frac = (*secs as f32 / total.max(1) as f32).clamp(0.0, 1.0);
                fill(
                    frame,
                    palette.control,
                    track_x,
                    ry + line / 2.0 - 3.0,
                    track_w,
                    6.0,
                    3.0,
                );
                fill(
                    frame,
                    palette.selection,
                    track_x,
                    ry + line / 2.0 - 3.0,
                    track_w * frac,
                    6.0,
                    3.0,
                );
                text_centred(
                    frame,
                    &t,
                    value_font,
                    palette.text,
                    rx - pad - tw,
                    ry + line / 2.0,
                    tw,
                    Align::Start,
                );
                ry += pitch;
            }
            y += card_h + section_gap;
        }

        // The day split across its 24 hours, so when the screen was kept busy
        // shows as clearly as how long it was.
        y = heading(y, &p("按小时", "By hour"));
        let hours = crate::ui::screen_time::hours_today();
        let hour_h = 96.0;
        fill(frame, palette.card, col_x, y, col_w, hour_h, 10.0);
        stroke_inside(
            frame,
            Rect {
                x: col_x,
                y,
                w: col_w,
                h: hour_h,
            },
            10.0,
            palette.card_stroke,
        );
        let hx = col_x + pad;
        let hw = col_w - 2.0 * pad;
        let slot = hw / 24.0;
        let bar_w = (slot * 0.6).max(2.0).min(slot);
        let htop = y + pad;
        let hbase = y + hour_h - pad - 14.0;
        let hareal = (hbase - htop).max(1.0);
        let hmax = hours.iter().copied().max().unwrap_or(0).max(1) as f32;
        for (i, secs) in hours.iter().enumerate() {
            let lx = hx + i as f32 * slot;
            let bx = lx + (slot - bar_w) / 2.0;
            let h = if *secs > 0 {
                (*secs as f32 / hmax * hareal).max(3.0)
            } else {
                2.0
            };
            fill(
                frame,
                if *secs > 0 {
                    palette.selection
                } else {
                    palette.control
                },
                bx,
                hbase - h,
                bar_w,
                h,
                2.0,
            );
        }
        for &i in [0usize, 6, 12, 18, 23].iter() {
            let lab = format!("{i:02}");
            let lw = frame.gfx.measure(&lab, label_font);
            let lx = hx + i as f32 * slot + (slot - lw) / 2.0;
            text_centred(
                frame,
                &lab,
                label_font,
                palette.text2,
                lx.clamp(col_x + pad, rx - pad - lw),
                hbase + 10.0,
                lw,
                Align::Start,
            );
        }
        y += hour_h + section_gap;

        // The day sorted into the kinds of app it was spent on.
        y = heading(y, &p("类别", "Categories"));
        let cats = crate::ui::screen_time::categories();
        if cats.is_empty() {
            y = plain_card(
                y,
                &[(
                    String::new(),
                    p("还没有可分类的记录。", "Nothing to sort yet."),
                )],
            ) + section_gap;
        } else {
            let pitch = 30.0;
            let card_h = 2.0 * pad + cats.len() as f32 * pitch;
            fill(frame, palette.card, col_x, y, col_w, card_h, 10.0);
            stroke_inside(
                frame,
                Rect {
                    x: col_x,
                    y,
                    w: col_w,
                    h: card_h,
                },
                10.0,
                palette.card_stroke,
            );
            let cat_total: u64 = cats.iter().map(|(_, _, s)| *s).sum::<u64>().max(1);
            let name_w = 120.0;
            let mut ry = y + pad;
            for (zh_name, en_name, secs) in &cats {
                let name = if zh {
                    zh_name.to_string()
                } else {
                    en_name.to_string()
                };
                let t = crate::ui::screen_time::say(*secs, zh);
                let tw = frame.gfx.measure(&t, value_font);
                text_centred(
                    frame,
                    &name,
                    label_font,
                    palette.text,
                    col_x + pad,
                    ry + line / 2.0,
                    name_w,
                    Align::Start,
                );
                let track_x = col_x + pad + name_w + 10.0;
                let track_w = (rx - pad - tw - 12.0 - track_x).max(20.0);
                let frac = (*secs as f32 / cat_total as f32).clamp(0.0, 1.0);
                fill(
                    frame,
                    palette.control,
                    track_x,
                    ry + line / 2.0 - 3.0,
                    track_w,
                    6.0,
                    3.0,
                );
                fill(
                    frame,
                    palette.selection,
                    track_x,
                    ry + line / 2.0 - 3.0,
                    track_w * frac,
                    6.0,
                    3.0,
                );
                text_centred(
                    frame,
                    &t,
                    value_font,
                    palette.text,
                    rx - pad - tw,
                    ry + line / 2.0,
                    tw,
                    Align::Start,
                );
                ry += pitch;
            }
            y += card_h + section_gap;
        }

        // How the tally is kept, told once at the foot.
        y = plain_card(
            y,
            &[(
                String::new(),
                p(
                    "这些数据由 Glance 自己记录，只存在本机，保留最近三周。",
                    "Glance gathers these itself, kept on this PC only, for the last three weeks.",
                ),
            )],
        );

        self.preview_content_h = y + 20.0 + scroll - area.y;
        frame.unclip();
    }
}
