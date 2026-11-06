//! The stage drawn for the session: what opens when the machine starts, how the clock and the calendar are
//! said, and the language words are written in.

use super::*;

impl Ui {

    /// What starts when a person signs in: how many are let, and each one — its own
    /// switch lives on the full page, so here it is the list and its state that is
    /// shown. Drawn as the other grouped-card previews, scrolled by the wheel.
    pub(super) fn paint_startup_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
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

        // The mark, its name, and how many are let to start.
        frame.icon(
            Page::Startup.icon(),
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
            &p("启动", "Startup"),
            Font::new(Family::SegoeDisplay, 17.0, 600.0),
            palette.text,
            y,
        );
        y += 24.0;
        let on = self.os.startups.iter().filter(|app| app.enabled).count();
        let said = format!(
            "{} {} · {}",
            p("允许自启", "Allowed"),
            on,
            self.os.startups.len()
        );
        centred(&said, label_font, palette.text2, y);
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

        // Each app that starts at sign-in, whose hive it is in, with a switch of its
        // own: pressed here as on the full page, it lets the app run or holds it.
        y = heading(y, &p("登录时启动的应用", "Apps started at sign-in"));
        let rows: Vec<(String, String, bool, Target)> = self
            .os
            .startups
            .iter()
            .map(|app| {
                let hive = pick(
                    lang,
                    if app.machine { "本机" } else { "本用户" },
                    if app.machine { "Machine" } else { "User" },
                )
                .to_string();
                (
                    app.display.clone(),
                    hive,
                    app.enabled,
                    Target::Startup(app.machine, app.name.clone(), !app.enabled),
                )
            })
            .collect();
        if rows.is_empty() {
            let note = if self.os.reading_startups.is_some() {
                p("正在读取启动项…", "Reading the start-up apps…")
            } else {
                p("没找到开机启动项。", "No start-up apps were found.")
            };
            y = card(y, &[(String::new(), note)]).0;
        } else {
            let row_h = 30.0;
            let switch_w = 40.0;
            let h = 2.0 * pad + rows.len() as f32 * row_h;
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
            let lx = col_x + pad;
            let switch_left = rx - pad - switch_w;
            let now = Instant::now();
            for (i, (name, hive, on, target)) in rows.into_iter().enumerate() {
                let cy = y + pad + i as f32 * row_h + row_h / 2.0;
                text_centred(
                    frame,
                    &name,
                    label_font,
                    palette.text,
                    lx,
                    cy,
                    (switch_left - 16.0 - lx).max(40.0),
                    Align::Start,
                );
                // The hive, told just left of its switch.
                let hw = frame.gfx.measure(&hive, label_font);
                text_centred(
                    frame,
                    &hive,
                    label_font,
                    palette.text2,
                    switch_left - 16.0 - hw,
                    cy,
                    hw,
                    Align::Start,
                );
                let pressed = self.pressed == Some(target.clone());
                self.toggle(
                    frame,
                    palette,
                    format!("toggle:{name}"),
                    on,
                    true,
                    switch_left,
                    cy,
                    now,
                    pressed,
                );
                targets.push((
                    Rect {
                        x: col_x,
                        y: cy - row_h / 2.0,
                        w: col_w,
                        h: row_h,
                    },
                    target,
                ));
            }
            y += h;
        }

        // The way into Windows' own startup page.
        y += section_gap;
        let label = p("启动应用设置…", "Startup apps settings…");
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
            Target::Jump("ms-settings:startupapps"),
        ));

        let bottom_gap = 20.0;
        self.preview_content_h = by + bh + bottom_gap + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }

    /// The clock and the tongue: how the hour is told, whether seconds show, the zone
    /// it keeps, and the word the machine is read in. The hour and seconds are switches
    /// you can throw here; the zone and the last sync are told as they stand. Drawn as
    /// the other grouped-card previews, with the ways to correct the clock and into
    /// Windows' own pages.
    pub(super) fn paint_datetime_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
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

        let scroll = self.preview_scroll;
        frame.clip(area.x, area.y, area.w, area.h);

        // The mark and its name.
        frame.icon(
            Page::DateTime.icon(),
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
            &p("时间与语言", "Time & Language"),
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

        // The clock as it is kept: what can be changed here is a switch, the zone and
        // the last sync are told as they stand.
        y = heading(y, &p("日期与时间", "Date & Time"));
        let zone = sys::current_zone();
        let said_zone = if zone.is_empty() {
            p("未知", "Unknown")
        } else {
            zone
        };
        let synced = match self.os.clock_said {
            Some(true) => Some(p("已完成", "Done")),
            Some(false) => Some(p("未成功", "Failed")),
            None => None,
        };
        let row_h = 30.0;
        let switch_w = 40.0;
        let rows = 3 + usize::from(synced.is_some());
        let h = 2.0 * pad + rows as f32 * row_h;
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
        let lx = col_x + pad;
        let switch_left = rx - pad - switch_w;
        let label_w = (switch_left - 16.0 - lx).max(40.0);
        let wide_w = (rx - pad - lx).max(40.0);
        let now = Instant::now();

        // 时间制: a switch between the 12- and the 24-hour clock.
        let cy = y + pad + row_h / 2.0;
        let word24 = if sys::clock_24() {
            p("24 小时制", "24-hour")
        } else {
            p("12 小时制", "12-hour")
        };
        text_centred(
            frame,
            &p("时间制", "Hour"),
            label_font,
            palette.text2,
            lx,
            cy,
            label_w,
            Align::Start,
        );
        let hw = frame.gfx.measure(&word24, label_font);
        text_centred(
            frame,
            &word24,
            label_font,
            palette.text,
            switch_left - 16.0 - hw,
            cy,
            hw,
            Align::Start,
        );
        let t24 = Target::Switch(Switch::Clock24);
        let pressed = self.pressed == Some(t24.clone());
        self.toggle(
            frame,
            palette,
            "toggle:clock24".to_string(),
            sys::clock_24(),
            true,
            switch_left,
            cy,
            now,
            pressed,
        );
        targets.push((
            Rect {
                x: col_x,
                y: cy - row_h / 2.0,
                w: col_w,
                h: row_h,
            },
            t24,
        ));

        // 秒显示: a switch for the seconds on the clock.
        let cy = cy + row_h;
        let wordsec = if sys::clock_seconds() {
            p("开", "On")
        } else {
            p("关", "Off")
        };
        text_centred(
            frame,
            &p("秒显示", "Seconds"),
            label_font,
            palette.text2,
            lx,
            cy,
            label_w,
            Align::Start,
        );
        let hw = frame.gfx.measure(&wordsec, label_font);
        text_centred(
            frame,
            &wordsec,
            label_font,
            palette.text,
            switch_left - 16.0 - hw,
            cy,
            hw,
            Align::Start,
        );
        let tsec = Target::Switch(Switch::Seconds);
        let pressed = self.pressed == Some(tsec.clone());
        self.toggle(
            frame,
            palette,
            "toggle:seconds".to_string(),
            sys::clock_seconds(),
            true,
            switch_left,
            cy,
            now,
            pressed,
        );
        targets.push((
            Rect {
                x: col_x,
                y: cy - row_h / 2.0,
                w: col_w,
                h: row_h,
            },
            tsec,
        ));

        // 当前时区: told as it stands, nothing to press.
        let cy = cy + row_h;
        text_centred(
            frame,
            &p("当前时区", "Zone"),
            label_font,
            palette.text2,
            lx,
            cy,
            wide_w,
            Align::Start,
        );
        let vw = frame.gfx.measure(&said_zone, value_font);
        text_centred(
            frame,
            &said_zone,
            value_font,
            palette.text,
            rx - pad - vw,
            cy,
            vw,
            Align::Start,
        );

        // The last word on correcting the clock, where there was one.
        if let Some(said) = &synced {
            let cy = cy + row_h;
            text_centred(
                frame,
                &p("校时", "Synced"),
                label_font,
                palette.text2,
                lx,
                cy,
                wide_w,
                Align::Start,
            );
            let vw = frame.gfx.measure(said, value_font);
            text_centred(
                frame,
                said,
                value_font,
                palette.text,
                rx - pad - vw,
                cy,
                vw,
                Align::Start,
            );
        }
        y += h;

        // The ways on: correct the clock, and Windows' own pages for either half.
        y += section_gap;
        let items = [
            (p("立即校时", "Sync now"), Target::SyncClock),
            (
                p("日期和时间设置…", "Date & time…"),
                Target::Jump("ms-settings:dateandtime"),
            ),
            (
                p("语言设置…", "Language…"),
                Target::Jump("ms-settings:regionlanguage"),
            ),
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
                target,
            ));
            bx += bw + gap;
        }

        let bottom_gap = 20.0;
        self.preview_content_h = by + bh + bottom_gap + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }

    /// The languages, the region, and the formats they carry — told as Windows
    /// keeps them now. Nothing here is changed in place: a display language is
    /// only worn again after signing out, so the way on is Windows' own page.
    pub(super) fn paint_language_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
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

        frame.icon(
            Page::Language.icon(),
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
            &p("语言与地区", "Language & Region"),
            Font::new(Family::SegoeDisplay, 17.0, 600.0),
            palette.text,
            y,
        );
        y += 34.0;

        let card = |y: f32, rows: &[(String, String)]| -> f32 {
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

        use crate::ui::system_settings as ss;
        let langs = ss::preferred_languages();
        let display = if langs.is_empty() {
            ss::locale_name()
        } else {
            langs.join("、")
        };
        let geo = ss::geo_name();
        let zone = ss::current_zone();
        let fmt = ss::time_format();
        let sdate = ss::short_date();
        let lname = ss::locale_name();
        y = heading(y, &p("语言与地区", "Language & Region"));
        let rows = vec![
            (
                p("显示语言", "Display"),
                if display.is_empty() {
                    p("未知", "Unknown")
                } else {
                    display
                },
            ),
            (
                p("区域格式", "Format"),
                if lname.is_empty() {
                    p("未知", "Unknown")
                } else {
                    lname
                },
            ),
            (
                p("国家或地区", "Country"),
                if geo.is_empty() {
                    p("未知", "Unknown")
                } else {
                    ss::country_name(&geo, lang)
                },
            ),
            (
                p("短日期", "Short date"),
                if sdate.is_empty() {
                    p("未知", "Unknown")
                } else {
                    sdate
                },
            ),
            (
                p("时间格式", "Time"),
                if fmt.is_empty() {
                    p("未知", "Unknown")
                } else {
                    fmt
                },
            ),
            (
                p("时区", "Zone"),
                if zone.is_empty() {
                    p("未知", "Unknown")
                } else {
                    zone
                },
            ),
        ];
        y = card(y, &rows) + section_gap;

        let label = p("语言设置…", "Language settings…");
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
            Target::Jump("ms-settings:regionlanguage"),
        ));

        self.preview_content_h = by + bh + 20.0 + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }
}
