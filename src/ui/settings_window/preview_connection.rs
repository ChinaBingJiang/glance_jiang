//! The stage drawn for reaching out: the general wiring of it, the networks near by, and what may be said
//! about this machine while it is being used. The two lines of readings kept for the smaller charts.

use super::*;

impl Ui {

    /// The 通用 hub with none of its sub-items picked: the machine live, as cards
    /// copied from the monitor panel's own — CPU, GPU, memory, network — each with
    /// its big reading, its recent chart, and the lines under it. Drawn at the
    /// pane's size and centred, not shrunk onto a screen's miniature; the pane
    /// scrolls what does not fit.
    pub(super) fn paint_general_preview(
        &mut self,
        frame: &Frame,
        palette: &Palette,
        area: Rect,
    ) -> Option<f32> {
        let lang = self.lang;
        let p = |zh: &'static str, en: &'static str| pick(lang, zh, en).to_string();
        let col_w = (area.w - 48.0).min(560.0);
        let col_x = area.x + (area.w - col_w) / 2.0;
        let rx = col_x + col_w;
        let pad = 14.0;
        let line = 18.0;
        let row_gap = 7.0;
        let section_gap: f32 = 18.0;
        let chart_h: f32 = 46.0;
        let label_font = Font::new(Family::Segoe, 12.5, 400.0);
        let value_font = Font::new(Family::Segoe, 12.5, 600.0);
        let big_font = Font::new(Family::SegoeDisplay, 30.0, 600.0);
        let name_font = Font::new(Family::Segoe, 11.5, 400.0);
        let scroll = self.preview_scroll;
        frame.clip(area.x, area.y, area.w, area.h);

        // One label and value within a card, the row's middle at `ry`; the value
        // kept short of `vend`, where a control may stand instead.
        let row_line = |ry: f32, label: &str, value: &str, vend: f32| {
            text_centred(
                frame,
                label,
                label_font,
                palette.text2,
                col_x + pad,
                ry,
                150.0,
                Align::Start,
            );
            let w = frame
                .gfx
                .measure(value, value_font)
                .min((vend - col_x - pad - 150.0 - 16.0).max(40.0));
            text_centred(
                frame,
                value,
                value_font,
                palette.text,
                vend - w,
                ry,
                w,
                Align::Start,
            );
        };
        // A card: a title row — its bold name at the left, the device's own at the
        // right — under it the big reading and its chart, and under that the rows.
        // `cores`, where given, the per-core bars under the chart; `ceiling`, where
        // given, the chart's fixed top, so a percent reads as a level rather than
        // against whatever the busiest moment was. Returns the y under the card.
        let card = |y: f32,
                    name: &str,
                    title: &str,
                    big: &str,
                    chart: Option<&[f32]>,
                    ceiling: Option<f32>,
                    cores: Option<&[f32]>,
                    rows: &[(String, String)]| {
            let title_h = 24.0;
            let rows_h = rows.len() as f32 * (line + row_gap) - row_gap;
            let bars_h = if cores.is_some() { 16.0 } else { 0.0 };
            let body_h = (chart_h + bars_h).max(frame.gfx.baseline(big_font).0 + 4.0);
            let h = pad
                + title_h
                + body_h
                + 12.0
                + if rows.is_empty() { 0.0 } else { rows_h + 12.0 }
                + pad;
            fill(frame, palette.card, col_x, y, col_w, h, 12.0);
            stroke_inside(
                frame,
                Rect {
                    x: col_x,
                    y,
                    w: col_w,
                    h,
                },
                12.0,
                palette.card_stroke,
            );
            // The title row: the bold name at the left, the device it is at the
            // right, kept short of the name rather than running into it.
            let head_font = Font::new(Family::Segoe, 13.5, 700.0);
            text_centred(
                frame,
                name,
                head_font,
                palette.text,
                col_x + pad,
                y + pad + title_h / 2.0,
                220.0,
                Align::Start,
            );
            if !title.is_empty() {
                let room =
                    (col_w - 2.0 * pad - frame.gfx.measure(name, head_font) - 24.0).max(60.0);
                let tw = frame.gfx.measure(title, name_font).min(room);
                text_centred(
                    frame,
                    title,
                    name_font,
                    palette.text2,
                    rx - pad - tw,
                    y + pad + title_h / 2.0,
                    tw,
                    Align::End,
                );
            }
            // The reading, and its chart and the cores' bars, in the body under it.
            let body_y = y + pad + title_h;
            text_centred(
                frame,
                big,
                big_font,
                palette.text,
                col_x + pad,
                body_y + body_h / 2.0,
                150.0,
                Align::Start,
            );
            let sx = col_x + 170.0;
            let sw = (rx - pad - sx).max(40.0);
            if let Some(points) = chart {
                Self::sparkline(frame, points, palette, sx, body_y, sw, chart_h, ceiling);
            }
            if let Some(shares) = cores {
                let n = shares.len().max(1) as f32;
                let side = ((sw - 8.0) / n - 4.0).clamp(2.0, 14.0);
                let gap = ((sw - n * side) / (n - 1.0).max(1.0)).max(2.0);
                for (i, share) in shares.iter().enumerate() {
                    let lit = share.clamp(0.0, 1.0);
                    let bx = sx + i as f32 * (side + gap);
                    fill(
                        frame,
                        palette.control,
                        bx,
                        body_y + chart_h + 6.0,
                        side,
                        8.0,
                        2.0,
                    );
                    if lit > 0.01 {
                        fill(
                            frame,
                            palette.switch_on,
                            bx,
                            body_y + chart_h + 6.0 + 8.0 * (1.0 - lit),
                            side,
                            8.0 * lit,
                            2.0,
                        );
                    }
                }
            }
            let mut ry = body_y + body_h + 12.0 + line / 2.0;
            for (label, value) in rows {
                row_line(ry, label, value, rx - pad);
                ry += line + row_gap;
            }
            y + h
        };

        let info = &crate::app().info;
        let history = crate::app().controller.history.read().unwrap();
        let all = crate::panel::contiguous(&history);
        if all.is_empty() {
            fill(
                frame,
                palette.card,
                col_x,
                area.y - scroll,
                col_w,
                80.0,
                12.0,
            );
            stroke_inside(
                frame,
                Rect {
                    x: col_x,
                    y: area.y - scroll,
                    w: col_w,
                    h: 80.0,
                },
                12.0,
                palette.card_stroke,
            );
            text_centred(
                frame,
                &p(
                    "还没有读数：监控启动后这里会活起来。",
                    "Nothing read yet — it fills in once the monitor is sampling.",
                ),
                label_font,
                palette.text2,
                col_x + pad,
                area.y - scroll + 40.0,
                col_w - 2.0 * pad,
                Align::Start,
            );
            self.preview_content_h = area.h;
            frame.unclip();
            return None;
        }
        let sample = all.last().unwrap();
        let recent = |get: fn(&crate::reading::Sample) -> Option<f32>| -> Vec<f32> {
            let mut out: Vec<f32> = all.iter().rev().take(90).rev().filter_map(get).collect();
            if out.len() < 2 {
                out.extend(
                    std::iter::repeat(out.first().copied().unwrap_or(0.0)).take(2 - out.len()),
                );
            }
            out
        };

        let mut y = area.y - scroll + 8.0;
        let cpu_pc = sample.cpu.unwrap_or(0.0);
        let pc_str = format!("{:.0}%", cpu_pc);
        let mut rows: Vec<(String, String)> = Vec::new();
        if let Some(temp) = sample.cpu_sensors.as_ref().and_then(|s| s.temp) {
            rows.push((p("温度", "Temperature"), format!("{temp:.0} °C")));
        }
        let kinds = &sample.kinds_ghz;
        if kinds.len() == 1 && kinds[0].is_some() {
            rows.push((p("频率", "Clock"), format!("{:.2} GHz", kinds[0].unwrap())));
        } else if kinds.len() >= 2 {
            if let Some(big) = kinds[0] {
                rows.push((p("大核频率", "P-core clock"), format!("{big:.2} GHz")));
            }
            if let Some(little) = kinds.last().copied().flatten() {
                rows.push((p("小核频率", "E-core clock"), format!("{little:.2} GHz")));
            }
        } else if let Some(ghz) = sample.ghz {
            rows.push((p("频率", "Clock"), format!("{ghz:.2} GHz")));
        }
        if let Some(power) = sample.cpu_sensors.as_ref().and_then(|s| s.power) {
            rows.push((p("功耗", "Power"), format!("{power:.1} W")));
        }
        let cores: Vec<f32> = sample
            .threads
            .iter()
            .map(|t| t.unwrap_or(0.0) / 100.0)
            .collect();
        y = card(
            y,
            "CPU",
            &info.cpu_name,
            &pc_str,
            Some(&recent(|s| s.cpu)),
            Some(100.0),
            Some(&cores),
            &rows,
        ) + section_gap;

        if let Some((gpu, first)) = info.gpus.first().zip(sample.gpus.first()) {
            let gpu_pc = first.usage.unwrap_or(0.0);
            let mut rows: Vec<(String, String)> = Vec::new();
            if let Some(used) = first.mem_used {
                rows.push((
                    p("显存", "Memory"),
                    format!(
                        "{} / {}",
                        crate::ui::text::bytes(used as f64),
                        crate::ui::text::bytes(gpu.mem_total as f64)
                    ),
                ));
            }
            if let Some(mhz) = first.clock_mhz {
                rows.push((p("频率", "Clock"), format!("{mhz:.0} MHz")));
            }
            if let Some(watt) = first.power {
                rows.push((p("功耗", "Power"), format!("{watt:.1} W")));
            }
            if let Some(temp) = first.temp {
                rows.push((p("温度", "Temperature"), format!("{temp:.0} °C")));
            }
            let gpu_chart = recent(|s| s.gpus.first().and_then(|g| g.usage));
            y = card(
                y,
                "GPU",
                &gpu.name,
                &format!("{gpu_pc:.0}%"),
                Some(&gpu_chart),
                Some(100.0),
                None,
                &rows,
            ) + section_gap;
        } else {
            text_centred(
                frame,
                &p("没有可读取的显卡。", "No graphics card could be read."),
                label_font,
                palette.text2,
                col_x,
                y + 10.0,
                col_w,
                Align::Start,
            );
            y += 26.0;
        }

        let mem_pc = if info.mem_total > 0 {
            sample.memory.used as f64 / info.mem_total as f64 * 100.0
        } else {
            0.0
        };
        let mut rows = vec![(
            p("已用", "In use"),
            format!(
                "{} / {}",
                crate::ui::text::bytes(sample.memory.used as f64),
                crate::ui::text::bytes(info.mem_total as f64)
            ),
        )];
        if let Some((now, rated)) = &info.memory_speed {
            let rated = rated.filter(|rated| *now != *rated);
            rows.push((
                p("频率", "Speed"),
                match rated {
                    // The tongue picks, not `pick`: the lines are made here.
                    Some(rated) if lang == Lang::Zh => {
                        format!("{now} MT/s（额定 {rated}）")
                    }
                    Some(rated) => format!("{now} MT/s (rated {rated})"),
                    None => format!("{now} MT/s"),
                },
            ));
        }
        let mem_chart: Vec<f32> = all
            .iter()
            .rev()
            .take(90)
            .rev()
            .map(|s| (s.memory.used as f64 / info.mem_total.max(1) as f64 * 100.0) as f32)
            .collect();
        // The modules, as the card's own name: "8 × 4 GB LPDDR5-8400".
        let modules = info.memory_modules.clone().unwrap_or_default();
        y = card(
            y,
            &p("内存", "Memory"),
            &modules,
            &format!("{mem_pc:.0}%"),
            Some(&mem_chart),
            Some(100.0),
            None,
            &rows,
        ) + section_gap;

        let net_title = sample
            .network
            .as_ref()
            .map(|n| n.name.clone())
            .or_else(|| info.network_adapter.clone())
            .unwrap_or_default();
        let down = sample.net_down.unwrap_or(0.0);
        let up = sample.net_up.unwrap_or(0.0);
        let mut rows = vec![
            (p("下载", "Down"), format!("{}/s", crate::ui::text::bytes(down))),
            (p("上传", "Up"), format!("{}/s", crate::ui::text::bytes(up))),
        ];
        if let Some(net) = sample.network.as_ref().filter(|n| n.link_bps > 0) {
            rows.push((
                p("链路", "Link"),
                format!("{:.0} Mbps", net.link_bps as f64 / 1e6),
            ));
        }
        let view = area.h;
        // The network's chart keeps both ways, and has no single big reading to
        // stand beside; it is drawn across the card, over the rows.
        {
            let title_h = 24.0;
            let rows_h = rows.len() as f32 * (line + row_gap) - row_gap;
            let h = pad + title_h + chart_h + 14.0 + rows_h + 12.0 + pad;
            fill(frame, palette.card, col_x, y, col_w, h, 12.0);
            stroke_inside(
                frame,
                Rect {
                    x: col_x,
                    y,
                    w: col_w,
                    h,
                },
                12.0,
                palette.card_stroke,
            );
            let head_font = Font::new(Family::Segoe, 13.5, 700.0);
            let name = p("网络", "Network");
            text_centred(
                frame,
                &name,
                head_font,
                palette.text,
                col_x + pad,
                y + pad + title_h / 2.0,
                220.0,
                Align::Start,
            );
            if !net_title.is_empty() {
                let room =
                    (col_w - 2.0 * pad - frame.gfx.measure(&name, head_font) - 24.0).max(60.0);
                let tw = frame.gfx.measure(&net_title, name_font).min(room);
                text_centred(
                    frame,
                    &net_title,
                    name_font,
                    palette.text2,
                    rx - pad - tw,
                    y + pad + title_h / 2.0,
                    tw,
                    Align::End,
                );
            }
            let sx = col_x + pad;
            let sw = (rx - pad - sx).max(40.0);
            let downs: Vec<f32> = all
                .iter()
                .rev()
                .take(90)
                .rev()
                .map(|s| s.net_down.unwrap_or(0.0) as f32)
                .collect();
            let ups: Vec<f32> = all
                .iter()
                .rev()
                .take(90)
                .rev()
                .map(|s| s.net_up.unwrap_or(0.0) as f32)
                .collect();
            Self::sparkline_two(
                frame,
                &downs,
                &ups,
                palette,
                sx,
                y + pad + title_h,
                sw,
                chart_h,
            );
            let mut ry = y + pad + title_h + chart_h + 14.0 + line / 2.0;
            for (label, value) in &rows {
                row_line(ry, label, value, rx - pad);
                ry += line + row_gap;
            }
            y += h + section_gap;
        }

        self.preview_content_h = (y + 20.0 + scroll - area.y).max(view);
        frame.unclip();
        // The charts creep: drawn again as the sampler's next readings land.
        Some(2.0)
    }

    /// The recent history's chart: a filled area under a lit line, the last
    /// reading marked with a dot — the monitor panel's own shape, drawn at the
    /// pane's size rather than shrunk onto a screen. `ceiling`, where given, is
    /// the fixed top of the scale (a percent's 100); else the busiest moment is.
    fn sparkline(
        frame: &Frame,
        points: &[f32],
        palette: &Palette,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        ceiling: Option<f32>,
    ) {
        if points.len() < 2 {
            return;
        }
        let most = ceiling.unwrap_or_else(|| points.iter().fold(1.0f32, |m, v| m.max(*v)));
        let most = most.max(1.0);
        let step = w / (points.len() - 1) as f32;
        let line: Vec<Point> = points
            .iter()
            .enumerate()
            .map(|(i, v)| Point {
                x: x + i as f32 * step,
                y: y + h - h * (v / most).clamp(0.0, 1.0),
            })
            .collect();
        let mut under = line.clone();
        under.push(Point { x: x + w, y: y + h });
        under.push(Point { x, y: y + h });
        Canvas::fill_shape(frame, &under, crate::ui::canvas::Fill::Solid(palette.control));
        Canvas::stroke(frame, &line, palette.switch_on, 1.5);
        if let Some(end) = line.last() {
            Canvas::fill_circle(frame, palette.switch_on, *end, 2.5);
        }
    }

    /// The same for traffic, which has two ways: the down line lit and filled,
    /// the up one dashed above the same scale.
    fn sparkline_two(
        frame: &Frame,
        downs: &[f32],
        ups: &[f32],
        palette: &Palette,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        if downs.len() < 2 {
            return;
        }
        let most = downs
            .iter()
            .chain(ups)
            .fold(1.0f32, |m, v| m.max(*v).max(1.0));
        let step = w / (downs.len() - 1) as f32;
        let at = |values: &[f32]| -> Vec<Point> {
            values
                .iter()
                .enumerate()
                .map(|(i, v)| Point {
                    x: x + i as f32 * step,
                    y: y + h - h * (v / most).clamp(0.0, 1.0),
                })
                .collect()
        };
        let down = at(downs);
        let mut under = down.clone();
        under.push(Point { x: x + w, y: y + h });
        under.push(Point { x, y: y + h });
        Canvas::fill_shape(frame, &under, crate::ui::canvas::Fill::Solid(palette.control));
        Canvas::stroke(frame, &down, palette.switch_on, 1.5);
        Canvas::stroke_dashed(frame, &at(ups), palette.text3, 1.2);
    }

    /// The wireless network as it is now: the name it joined, what the card reports
    /// of the link, the numbers it was given, and the traffic over it this moment.
    /// One wireless network's whole story on the right, once picked on the left:
    /// its details — and, for a saved one, its kept password, shown on a press;
    /// for a new one, a button to type its password and join. Then what the page
    /// can do to it: let go of the link, join again, leave it of its own accord, or
    /// forget it. With none picked it shows the card as it stands now. The pane
    /// scrolls; its buttons are hit through `self.targets`.
    pub(super) fn paint_wifi_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
        let lang = self.lang;
        let p = |zh: &'static str, en: &'static str| pick(lang, zh, en).to_string();
        let margin = 24.0;
        let col_w = (area.w - 2.0 * margin).max(240.0);
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
        // A pill button at (x, y) firing `target`; its width back, to step along.
        let pill = |targets: &mut Vec<(Rect, Target)>,
                    x: f32,
                    y: f32,
                    label: &str,
                    target: Target|
         -> f32 {
            let bw = frame.gfx.measure(label, btn_font) + 28.0;
            let bh = 30.0;
            fill(frame, palette.control, x, y, bw, bh, 8.0);
            stroke_inside(frame, Rect { x, y, w: bw, h: bh }, 8.0, palette.card_stroke);
            text_centred(
                frame,
                label,
                btn_font,
                palette.text,
                x + 14.0,
                y + bh / 2.0,
                bw - 28.0,
                Align::Start,
            );
            targets.push((Rect { x, y, w: bw, h: bh }, target));
            bw
        };
        // A row of pill buttons, wrapped to the column's width; the y under them.
        let pill_row =
            |targets: &mut Vec<(Rect, Target)>, y: f32, acts: &[(String, Target)]| -> f32 {
                let (mut ax, mut ay) = (col_x, y);
                for (label, target) in acts {
                    let bw = frame.gfx.measure(label, btn_font) + 28.0;
                    if ax + bw > rx && ax > col_x {
                        ax = col_x;
                        ay += 30.0 + 10.0;
                    }
                    let took = target.clone();
                    let w = pill(targets, ax, ay, label, took);
                    ax += w + 10.0;
                }
                ay + 30.0
            };

        let snap = self.os.wifi.clone();
        let chosen = self.os.wifi_pick.clone();
        let mut y = area.y + 20.0 - scroll;

        match &chosen {
            // Nothing picked: the card as it stands, and a word to pick one.
            None => {
                frame.icon(
                    Page::Wifi.icon(),
                    Point { x: cx, y: y + 24.0 },
                    40.0,
                    palette.text,
                );
                y += 24.0 + 44.0;
                let title = if !snap.on {
                    p("无线已关闭", "The radio is off")
                } else if let Some(net) = snap.current() {
                    net.ssid.clone()
                } else {
                    p("未连接", "Not joined")
                };
                let head = Font::new(Family::SegoeDisplay, 17.0, 600.0);
                let m = frame.gfx.measure(&title, head).min(col_w);
                text_centred(
                    frame,
                    &title,
                    head,
                    palette.text,
                    cx - m / 2.0,
                    y,
                    m,
                    Align::Start,
                );
                y += 34.0;
                if snap.on {
                    if let Some(net) = snap.current() {
                        let mut lines = vec![
                            (p("状态", "Status"), p("已连接", "Joined")),
                            (p("信号", "Signal"), format!("{}%", net.signal)),
                            (
                                p("安全", "Security"),
                                wifi_security(lang, net.auth, net.secure),
                            ),
                        ];
                        if let Some(conn) = &snap.connection {
                            if !conn.band.is_empty() {
                                lines.push((p("频段", "Band"), conn.band.clone()));
                            }
                            if conn.channel > 0 {
                                lines.push((p("信道", "Channel"), conn.channel.to_string()));
                            }
                            if !conn.phy.is_empty() {
                                lines.push((p("标准", "Standard"), conn.phy.clone()));
                            }
                            let rate = crate::ui::wifi::link(conn.rx.max(conn.tx));
                            if !rate.is_empty() {
                                lines.push((p("链路", "Link"), rate));
                            }
                        }
                        if let Some(addr) = &snap.addr {
                            if !addr.ip.is_empty() {
                                lines.push((p("IP 地址", "IP address"), addr.ip.clone()));
                            }
                            if !addr.router.is_empty() {
                                lines.push((p("网关", "Router"), addr.router.clone()));
                            }
                            if !addr.dns.is_empty() {
                                lines.push((p("DNS", "DNS"), addr.dns.clone()));
                            }
                        }
                        y = heading(y, &p("当前连接", "Now joined"));
                        y = plain_card(y, &lines) + section_gap;
                    }
                }
                let hint = p(
                    "在左侧点击一个网络，查看它的详情。",
                    "Pick a network on the left to see its details.",
                );
                let m = frame.gfx.measure(&hint, label_font).min(col_w);
                text_centred(
                    frame,
                    &hint,
                    label_font,
                    palette.text2,
                    cx - m / 2.0,
                    y + line / 2.0,
                    m,
                    Align::Start,
                );
                y += line + section_gap;
                let _ = pill(
                    &mut targets,
                    cx - 60.0,
                    y,
                    &p("Wi-Fi 设置…", "Wi-Fi settings…"),
                    Target::Jump("ms-settings:network-wifi"),
                );
                y += 30.0 + section_gap;
            }
            // A network picked: its details, its password or join, its actions.
            Some(ssid) => {
                let net = snap.nets.iter().find(|n| n.ssid == *ssid);
                let is_connected = net.map(|n| n.connected).unwrap_or(false);
                let is_known =
                    net.map(|n| n.known).unwrap_or(false) || snap.saved.iter().any(|s| s == ssid);
                let head = Font::new(Family::SegoeDisplay, 17.0, 600.0);
                let m = frame.gfx.measure(ssid, head).min(col_w);
                text_centred(
                    frame,
                    ssid,
                    head,
                    palette.text,
                    cx - m / 2.0,
                    y,
                    m,
                    Align::Start,
                );
                y += 34.0;

                // The truth of it: how it lets you in, and where it stands now.
                let mut lines: Vec<(String, String)> = Vec::new();
                if let Some(net) = net {
                    lines.push((
                        p("安全", "Security"),
                        wifi_security(lang, net.auth, net.secure),
                    ));
                    lines.push((p("信号", "Signal"), format!("{}%", net.signal)));
                    if is_connected {
                        if let Some(conn) = &snap.connection {
                            if !conn.band.is_empty() {
                                lines.push((p("频段", "Band"), conn.band.clone()));
                            }
                            if conn.channel > 0 {
                                lines.push((p("信道", "Channel"), conn.channel.to_string()));
                            }
                            if !conn.phy.is_empty() {
                                lines.push((p("标准", "Standard"), conn.phy.clone()));
                            }
                            let rate = crate::ui::wifi::link(conn.rx.max(conn.tx));
                            if !rate.is_empty() {
                                lines.push((p("链路", "Link"), rate));
                            }
                        }
                        if let Some(addr) = &snap.addr {
                            if !addr.ip.is_empty() {
                                lines.push((p("IP 地址", "IP address"), addr.ip.clone()));
                            }
                            if !addr.mask.is_empty() {
                                lines.push((p("子网掩码", "Subnet mask"), addr.mask.clone()));
                            }
                            if !addr.router.is_empty() {
                                lines.push((p("网关", "Router"), addr.router.clone()));
                            }
                            if !addr.dns.is_empty() {
                                lines.push((p("DNS", "DNS"), addr.dns.clone()));
                            }
                        }
                    }
                } else {
                    lines.push((
                        p("状态", "Status"),
                        p("已保存，不在范围内", "Saved, out of range"),
                    ));
                }
                y = heading(y, &p("详细信息", "Details"));
                y = plain_card(y, &lines) + section_gap;

                // Its password, when one is kept; a button to join, when none is.
                if is_known {
                    y = heading(y, &p("密码", "Password"));
                    match &self.os.wifi_key {
                        Some((for_ssid, key)) if *for_ssid == *ssid => {
                            y = plain_card(y, &[(p("密码", "Password"), key.clone())])
                                + section_gap;
                        }
                        _ => {
                            let _ = pill(
                                &mut targets,
                                col_x,
                                y,
                                &p("显示密码", "Show password"),
                                Target::WifiKey(ssid.clone()),
                            );
                            y += 30.0 + section_gap;
                        }
                    }
                } else if let Some(net) = net {
                    if crate::ui::wifi::page_joinable(net.auth, net.secure) {
                        y = heading(y, &p("加入", "Join"));
                        let _ = pill(
                            &mut targets,
                            col_x,
                            y,
                            &p("加入网络", "Join network"),
                            Target::WifiConnect(ssid.clone()),
                        );
                        y += 30.0 + section_gap;
                    } else {
                        let note = p(
                            "企业网络需要证书或域账户，请在系统的网络页里加入。",
                            "This enterprise network wants a certificate or a domain account; join it on Windows' own page.",
                        );
                        for said in wrap_text(frame, &note, label_font, col_w) {
                            text_centred(
                                frame,
                                &said,
                                label_font,
                                palette.text2,
                                col_x,
                                y + line / 2.0,
                                col_w,
                                Align::Start,
                            );
                            y += line;
                        }
                        y += section_gap;
                    }
                }

                // What the page can do to it as a whole.
                let mut acts: Vec<(String, Target)> = Vec::new();
                if is_connected {
                    acts.push((p("断开连接", "Disconnect"), Target::WifiDisconnect));
                    if let Some(conn) = &snap.connection {
                        let auto = conn.auto;
                        acts.push((
                            if auto {
                                p("自动加入：开", "Auto-join: on")
                            } else {
                                p("自动加入：关", "Auto-join: off")
                            },
                            Target::WifiAuto(ssid.clone(), !auto),
                        ));
                    }
                } else if is_known {
                    acts.push((p("连接", "Join"), Target::WifiJoin(ssid.clone())));
                }
                if is_known {
                    acts.push((p("忘记网络", "Forget"), Target::WifiForget(ssid.clone())));
                }
                if !acts.is_empty() {
                    y = heading(y, &p("操作", "Actions"));
                    y = pill_row(&mut targets, y, &acts) + section_gap;
                }

                // The word a last action left here.
                if let Some(said) = &self.os.wifi_said {
                    for said in wrap_text(frame, said, label_font, col_w) {
                        text_centred(
                            frame,
                            &said,
                            label_font,
                            palette.text2,
                            col_x,
                            y + line / 2.0,
                            col_w,
                            Align::Start,
                        );
                        y += line;
                    }
                    y += section_gap;
                }

                // And a way back to the whole card, and into Windows' own page.
                let _ = pill(
                    &mut targets,
                    col_x,
                    y,
                    &p("返回", "Back"),
                    Target::WifiPick(String::new()),
                );
                let bw = frame.gfx.measure(&p("返回", "Back"), btn_font) + 28.0;
                let _ = pill(
                    &mut targets,
                    col_x + bw + 10.0,
                    y,
                    &p("Wi-Fi 设置…", "Wi-Fi settings…"),
                    Target::Jump("ms-settings:network-wifi"),
                );
                y += 30.0 + section_gap;
            }
        }

        self.preview_content_h = y + 20.0 + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }

    /// The privacy page's preview: with none picked, the shield and a word to
    /// pick one; with a capability picked, the machine's own switch for it at the
    /// top and the apps that have asked below, each let or held. The pane scrolls;
    /// its switches are hit through `self.targets`.
    pub(super) fn paint_privacy_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
        let lang = self.lang;
        let p = |zh: &'static str, en: &'static str| pick(lang, zh, en).to_string();
        let margin = 24.0;
        let col_w = (area.w - 2.0 * margin).max(240.0);
        let col_x = area.x + (area.w - col_w) / 2.0;
        let rx = col_x + col_w;
        let cx = area.x + area.w / 2.0;
        let pad = 14.0;
        let line = 18.0;
        let section_gap = 18.0;
        let label_font = Font::new(Family::Segoe, 12.5, 400.0);
        let title_font = Font::new(Family::Segoe, 13.0, 600.0);
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
        // A switch drawn still, at the track's left edge `x` centred on `cy`.
        let switch = |x: f32, cy: f32, on: bool| {
            let track = Rect {
                x,
                y: cy - 11.0,
                w: 40.0,
                h: 22.0,
            };
            if on {
                fill(frame, palette.switch_on, x, cy - 11.0, 40.0, 22.0, 11.0);
            } else {
                stroke_inside(frame, track, 11.0, palette.switch_stroke);
            }
            let size = if on { 16.0 } else { 14.0 };
            let kx = if on { x + 40.0 - 4.0 - size } else { x + 4.0 };
            let knob = if on {
                Color::hex(0xFFFFFF, 1.0)
            } else {
                palette.switch_knob
            };
            fill(frame, knob, kx, cy - size / 2.0, size, size, size / 2.0);
        };
        // A card with a word on the left and a switch on the right, whole pressed.
        let switch_card = |targets: &mut Vec<(Rect, Target)>,
                           y: f32,
                           label: &str,
                           on: bool,
                           target: Target|
         -> f32 {
            let h = 52.0;
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
            text_centred(
                frame,
                label,
                label_font,
                palette.text,
                col_x + pad,
                y + h / 2.0,
                col_w - 2.0 * pad - 56.0,
                Align::Start,
            );
            switch(rx - pad - 40.0, y + h / 2.0, on);
            targets.push((
                Rect {
                    x: col_x,
                    y,
                    w: col_w,
                    h,
                },
                target,
            ));
            y + h
        };

        match self.os.priv_pick {
            // Nothing picked: the shield, and a word to pick a capability.
            None => {
                frame.icon(
                    Page::Privacy.icon(),
                    Point {
                        x: cx,
                        y: area.y + area.h * 0.34,
                    },
                    44.0,
                    palette.text,
                );
                let head = Font::new(Family::SegoeDisplay, 18.0, 600.0);
                let title = p("隐私与安全性", "Privacy & Security");
                let m = frame.gfx.measure(&title, head).min(col_w);
                text_centred(
                    frame,
                    &title,
                    head,
                    palette.text,
                    cx - m / 2.0,
                    area.y + area.h * 0.58,
                    m,
                    Align::Start,
                );
                let hint = p(
                    "点击左侧一项，查看哪些应用在用它。",
                    "Pick one on the left to see the apps that use it.",
                );
                let m = frame.gfx.measure(&hint, label_font).min(col_w);
                text_centred(
                    frame,
                    &hint,
                    label_font,
                    palette.text2,
                    cx - m / 2.0,
                    area.y + area.h * 0.58 + 30.0,
                    m,
                    Align::Start,
                );
                self.preview_content_h = area.h;
            }
            // A capability picked: its own switch on top, its apps below.
            Some(cap) => {
                let (capzh, capen, hintzh, hinten) = match cap {
                    "webcam" => (
                        "相机访问",
                        "Camera access",
                        "允许应用使用摄像头",
                        "Apps may use the camera",
                    ),
                    "microphone" => (
                        "麦克风访问",
                        "Microphone access",
                        "允许应用使用麦克风",
                        "Apps may use the microphone",
                    ),
                    _ => (
                        "位置服务",
                        "Location services",
                        "允许应用获取当前位置",
                        "Apps may ask where you are",
                    ),
                };
                let mut y = area.y + 20.0 - scroll;
                let head = Font::new(Family::SegoeDisplay, 17.0, 600.0);
                let title = p(capzh, capen);
                let m = frame.gfx.measure(&title, head).min(col_w);
                text_centred(
                    frame,
                    &title,
                    head,
                    palette.text,
                    cx - m / 2.0,
                    y,
                    m,
                    Align::Start,
                );
                y += 34.0;
                let on = crate::ui::system_settings::consent_on(cap);
                y = switch_card(
                    &mut targets,
                    y,
                    &p(hintzh, hinten),
                    on,
                    Target::ConsentGlobal(cap),
                ) + section_gap;
                y = heading(y, &p("使用此权限的应用", "Apps that use it"));
                if self.os.reading_consent.is_some() {
                    let said = p("正在读取应用…", "Reading the apps…");
                    text_centred(
                        frame,
                        &said,
                        label_font,
                        palette.text2,
                        col_x + pad,
                        y + line / 2.0,
                        col_w - 2.0 * pad,
                        Align::Start,
                    );
                    y += line;
                } else if self.os.consent_apps.is_empty() {
                    let said = p(
                        "还没有应用申请过这项权限。",
                        "No app has asked for this yet.",
                    );
                    text_centred(
                        frame,
                        &said,
                        label_font,
                        palette.text2,
                        col_x + pad,
                        y + line / 2.0,
                        col_w - 2.0 * pad,
                        Align::Start,
                    );
                    y += line;
                } else {
                    let row_h = 48.0;
                    let count = self.os.consent_apps.len();
                    let h = count as f32 * row_h;
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
                    for (i, app) in self.os.consent_apps.iter().enumerate() {
                        let ry = y + i as f32 * row_h;
                        if i > 0 {
                            fill(
                                frame,
                                palette.rule,
                                col_x + pad,
                                ry,
                                col_w - 2.0 * pad,
                                1.0,
                                0.0,
                            );
                        }
                        text_centred(
                            frame,
                            &app.display,
                            label_font,
                            palette.text,
                            col_x + pad,
                            ry + row_h / 2.0,
                            col_w - 2.0 * pad - 56.0,
                            Align::Start,
                        );
                        switch(rx - pad - 40.0, ry + row_h / 2.0, app.allowed);
                        targets.push((
                            Rect {
                                x: col_x,
                                y: ry,
                                w: col_w,
                                h: row_h,
                            },
                            Target::Consent(cap, app.keys.clone(), app.packaged, !app.allowed),
                        ));
                    }
                    y += h;
                }
                self.preview_content_h = y + 20.0 + scroll - area.y;
            }
        }
        frame.unclip();
        self.targets.extend(targets);
    }
}
