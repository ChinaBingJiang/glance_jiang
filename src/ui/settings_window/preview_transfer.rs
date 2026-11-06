//! The stage drawn for what leaves the machine and what comes back: sharing, what is near, copies kept
//! away, the state gone back to, and the account all this belongs to.

use super::*;

impl Ui {

    /// The name this PC goes by, the workgroup it belongs to, and the adapters
    /// that reach somewhere — the sharing centre told as it truly is. Sharing
    /// itself is the system's to manage, so the way on opens its page.
    pub(super) fn paint_sharing_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
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
            Page::Sharing.icon(),
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
            &p("共享", "Sharing"),
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
        let group = ss::workgroup();
        let name = ss::computer_name();
        y = heading(y, &p("本机", "This PC"));
        let mine = vec![
            (
                p("计算机名", "Name"),
                if name.is_empty() {
                    p("未知", "Unknown")
                } else {
                    name
                },
            ),
            (
                p("工作组或域", "Workgroup"),
                if group.is_empty() {
                    p("未加入", "Not joined")
                } else {
                    group
                },
            ),
        ];
        y = card(y, &mine) + section_gap;

        y = heading(y, &p("网络适配器", "Adapters"));
        let rows: Vec<(String, String)> = if self.os.adapters.is_empty() {
            let note = if self.os.reading_adapters.is_some() {
                p("正在读取网络…", "Reading the network…")
            } else {
                p("没读到可用的网络。", "No network could be read.")
            };
            vec![(String::new(), note)]
        } else {
            self.os
                .adapters
                .iter()
                .map(|a| {
                    let at = a.ipv4.clone().unwrap_or_else(|| p("未编号", "Unnumbered"));
                    let via = a
                        .gateway
                        .clone()
                        .unwrap_or_else(|| p("无网关", "No gateway"));
                    (a.name.clone(), format!("{at} · {via}"))
                })
                .collect()
        };
        y = card(y, &rows) + section_gap;

        let label = p("网络和共享中心…", "Network and sharing…");
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
            Target::Jump("shell:networkandsharing"),
        ));

        self.preview_content_h = by + bh + 20.0 + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }

    /// Nearby sharing — AirDrop and Handoff's Windows kin — as the switch
    /// Windows keeps it: worn here, it is let on to this user's own devices or
    /// off again; the sending itself stays the system's own doing.
    pub(super) fn paint_nearby_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
        let lang = self.lang;
        let p = |zh: &'static str, en: &'static str| pick(lang, zh, en).to_string();
        let cx = area.x + area.w / 2.0;
        let margin = 24.0;
        let col_w = (area.w - 2.0 * margin).max(240.0);
        let col_x = area.x + (area.w - col_w) / 2.0;
        let rx = col_x + col_w;
        let pad = 14.0;
        let section_gap = 18.0;
        let label_font = Font::new(Family::Segoe, 12.5, 400.0);
        let value_font = Font::new(Family::Segoe, 12.5, 600.0);
        let title_font = Font::new(Family::Segoe, 13.0, 600.0);
        let btn_font = Font::new(Family::Segoe, 12.5, 600.0);
        let mut targets: Vec<(Rect, Target)> = Vec::new();

        let scroll = self.preview_scroll;
        frame.clip(area.x, area.y, area.w, area.h);

        frame.icon(
            Page::Nearby.icon(),
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
            &p("就近共享", "Nearby Sharing"),
            Font::new(Family::SegoeDisplay, 17.0, 600.0),
            palette.text,
            y,
        );
        y += 34.0;

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

        let on = crate::ui::system_settings::nearby_sharing();
        y = heading(y, &p("向附近的设备收发", "Send and receive nearby"));
        let row_h = 30.0;
        let switch_w = 40.0;
        let h = 2.0 * pad + 2.0 * row_h;
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
        let wide_w = (rx - pad - lx).max(40.0);
        let now = Instant::now();

        let cy = y + pad + row_h / 2.0;
        let word = if on { p("开", "On") } else { p("关", "Off") };
        text_centred(
            frame,
            &p("就近共享", "Nearby sharing"),
            label_font,
            palette.text2,
            lx,
            cy,
            (switch_left - 16.0 - lx).max(40.0),
            Align::Start,
        );
        let hw = frame.gfx.measure(&word, label_font);
        text_centred(
            frame,
            &word,
            label_font,
            palette.text,
            switch_left - 16.0 - hw,
            cy,
            hw,
            Align::Start,
        );
        let target = Target::Nearby(!on);
        let pressed = self.pressed == Some(target.clone());
        self.toggle(
            frame,
            palette,
            "toggle:nearby".to_string(),
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

        let cy = cy + row_h;
        let said = if on {
            p("需蓝牙与 Wi-Fi", "Wants Bluetooth & Wi-Fi")
        } else {
            p("关闭中", "Held off")
        };
        text_centred(
            frame,
            &p("条件", "Needs"),
            label_font,
            palette.text2,
            lx,
            cy,
            wide_w,
            Align::Start,
        );
        let vw = frame.gfx.measure(&said, value_font);
        text_centred(
            frame,
            &said,
            value_font,
            palette.text,
            rx - pad - vw,
            cy,
            vw,
            Align::Start,
        );
        y += h + section_gap;

        let label = p("就近共享设置…", "Sharing settings…");
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
            Target::Jump("ms-settings:sharedexperienc"),
        ));

        self.preview_content_h = by + bh + 20.0 + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }

    /// Backup as it can honestly be read: whether file history has ever been
    /// set up, and the room the system drive still holds. The backing up and
    /// restoring is Windows' own, so the way on opens its page.
    pub(super) fn paint_backup_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
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
            Page::Backup.icon(),
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
            &p("备份", "Backup"),
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

        let room = crate::ui::system_settings::drive_space();
        let system = room
            .iter()
            .find(|(name, _, _)| name.starts_with('C'))
            .cloned();
        let mut rows = vec![(
            p("文件历史记录", "File history"),
            if crate::ui::system_settings::file_history_configured() {
                p("已配置", "Set up")
            } else {
                p("未见配置", "Not set up")
            },
        )];
        if let Some((_, used, total)) = system {
            rows.push((
                p("系统盘可用", "Free on C"),
                crate::ui::text::usage(total - used, total),
            ));
        }
        y = heading(y, &p("备份", "Backup"));
        y = card(y, &rows) + section_gap;

        let label = p("备份设置…", "Backup settings…");
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
            Target::Jump("ms-settings:backup"),
        ));

        self.preview_content_h = by + bh + 20.0 + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }

    /// The way the machine starts: the firmware it boots by and whether secure
    /// boot is worn, then the two real restarts — into the recovery environment,
    /// and (on UEFI) into the firmware's own setup. Each asks for the privilege.
    pub(super) fn paint_recovery_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
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
            Page::Recovery.icon(),
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
            &p("启动磁盘", "Startup Disk"),
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

        let uefi = crate::ui::system_settings::firmware_uefi();
        let boot = crate::ui::system_settings::secure_boot();
        y = heading(y, &p("启动", "Starts by"));
        let rows = vec![
            (
                p("固件类型", "Firmware"),
                match uefi {
                    Some(true) => "UEFI".to_string(),
                    Some(false) => p("传统 BIOS", "Legacy BIOS"),
                    None => p("未知", "Unknown"),
                },
            ),
            (
                p("安全启动", "Secure boot"),
                match boot {
                    Some(true) => p("开", "On"),
                    Some(false) => p("关", "Off"),
                    None => p("未知", "Unknown"),
                },
            ),
        ];
        y = card(y, &rows) + section_gap;

        let mut items: Vec<(String, Target)> = vec![(
            p("重启到恢复环境", "Restart to recovery"),
            Target::RebootTo(true),
        )];
        if uefi == Some(true) {
            items.push((
                p("重启进入固件设置", "Restart to firmware"),
                Target::RebootTo(false),
            ));
        }
        items.push((
            p("恢复设置…", "Recovery…"),
            Target::Jump("ms-settings:recovery"),
        ));
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

        self.preview_content_h = by + bh + 20.0 + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }

    /// The account as it is: its owner's own picture where Windows keeps one, their
    /// name, the kind of account it is, and the machine and folder it belongs to.
    /// Kept in a column of its own — the picture and the name centred, the facts in
    /// one card under them — rather than scattered across the pane's whole width.
    pub(super) fn paint_account_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
        let lang = self.lang;
        let p = |zh, en| pick(lang, zh, en).to_string();
        let margin = 24.0;
        let col_w = (area.w - 2.0 * margin).min(420.0).max(240.0);
        let col_x = area.x + (area.w - col_w) / 2.0;
        let cx = col_x + col_w / 2.0;
        let rx = col_x + col_w;
        let pad = 14.0;
        let line = 18.0;
        let row_gap = 8.0;
        let label_font = Font::new(Family::Segoe, 12.5, 400.0);
        let value_font = Font::new(Family::Segoe, 12.5, 600.0);
        let scroll = self.preview_scroll;
        frame.clip(area.x, area.y, area.w, area.h);
        // The picture is read once, from the newest file Windows leaves in its folder.
        if !self.os.avatar_read {
            self.os.avatar_read = true;
            self.os.avatar_pic = account_picture()
                .as_ref()
                .and_then(|path| crate::cover::avatar(path))
                .map(|cover| crate::ui::canvas::Picture {
                    key: cover.key,
                    side: crate::cover::SIDE,
                    pixels: cover.pixels,
                });
        }
        let side = 84.0f32;
        let ay = area.y + 22.0 - scroll;
        let ax = cx - side / 2.0;
        match &self.os.avatar_pic {
            Some(pic) => frame.picture(pic, ax, ay, side, side, side / 2.0, 1.0),
            None => {
                fill(frame, palette.control, ax, ay, side, side, side / 2.0);
                frame.icon(
                    Icon::User,
                    Point {
                        x: cx,
                        y: ay + side / 2.0,
                    },
                    40.0,
                    palette.text2,
                );
            }
        }
        let name = std::env::var("USERNAME").unwrap_or_default();
        let kind = if crate::elevation::is_elevated() {
            p("管理员", "Administrator")
        } else {
            p("标准用户", "Standard user")
        };
        let big = Font::new(Family::SegoeDisplay, 18.0, 600.0);
        let small = Font::new(Family::Segoe, 12.5, 400.0);
        let draw = |s: &str, font: Font, color: Color, cy: f32| {
            let m = frame.gfx.measure(s, font).min(col_w);
            text_centred(frame, s, font, color, cx - m / 2.0, cy, m, Align::Start);
        };
        let mut y = ay + side + 16.0;
        draw(
            if name.is_empty() { "—" } else { &name },
            big,
            palette.text,
            y,
        );
        y += 24.0;
        draw(&kind, small, palette.text2, y);
        y += 30.0;
        // The facts the page tells, in one card: the label's column as wide as its
        // widest label, the values kept to the card's right, wrapped where they run.
        let computer = std::env::var("COMPUTERNAME").unwrap_or_default();
        let profile = std::env::var("USERPROFILE").unwrap_or_default();
        let home = profile.rsplit(['\\', '/']).next().unwrap_or("").to_string();
        let rows = vec![
            (
                p("计算机", "Computer"),
                if computer.is_empty() {
                    "—".into()
                } else {
                    computer
                },
            ),
            (
                p("用户", "User"),
                if home.is_empty() { "—".into() } else { home },
            ),
            (
                p("用户目录", "Home folder"),
                if profile.is_empty() {
                    "—".into()
                } else {
                    profile
                },
            ),
            (p("账户类型", "Account"), kind),
        ];
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
        self.preview_content_h = y + h + 22.0 + scroll - area.y;
        frame.unclip();
    }
}
