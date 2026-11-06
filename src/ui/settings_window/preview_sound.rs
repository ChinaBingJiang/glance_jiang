//! The stage drawn for sound, for the devices the radio holds, and for the traffic over the wire.

use super::*;

impl Ui {
    /// The Sound page's preview: the device in hand — a picked one, else the
    /// default — with its own volume slid and its mute let or held. The slider is
    /// hit through `self.targets` like any other, and writes that device alone.
    /// The pane scrolls.
    pub(super) fn paint_sound_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
        let lang = self.lang;
        let p = |zh: &'static str, en: &'static str| pick(lang, zh, en).to_string();
        let margin = 24.0;
        let col_w = (area.w - 2.0 * margin).max(240.0);
        let col_x = area.x + (area.w - col_w) / 2.0;
        let rx = col_x + col_w;
        let cx = area.x + area.w / 2.0;
        let pad = 14.0;
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

        // The device in hand: a picked one, or the default output if none is.
        let picked = self.os.sound_pick.clone().or_else(|| {
            self.os
                .outs
                .iter()
                .find(|out| out.given)
                .cloned()
                .or_else(|| self.os.outs.first().cloned())
        });
        let mut y = area.y + 20.0 - scroll;
        let head = Font::new(Family::SegoeDisplay, 17.0, 600.0);
        let title = picked
            .as_ref()
            .map(|out| out.name.clone())
            .unwrap_or_else(|| p("声音", "Sound"));
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
        y += 30.0;
        // A small word under the title: whether this is the default, or one picked aside.
        let said = match (&self.os.sound_pick, picked.as_ref()) {
            (Some(out), _) if out.given => p("默认输出", "Default output"),
            (Some(_), _) => p("已单独选中", "Picked out"),
            (None, Some(_)) => p("默认设备", "The default device"),
            (None, None) => String::new(),
        };
        if !said.is_empty() {
            let m = frame.gfx.measure(&said, label_font).min(col_w);
            text_centred(
                frame,
                &said,
                label_font,
                palette.text2,
                cx - m / 2.0,
                y,
                m,
                Align::Start,
            );
        }
        y += 26.0;

        // The volume card: its word and value on top, the track the width below.
        let vol_h = 76.0;
        fill(frame, palette.card, col_x, y, col_w, vol_h, 10.0);
        stroke_inside(
            frame,
            Rect {
                x: col_x,
                y,
                w: col_w,
                h: vol_h,
            },
            10.0,
            palette.card_stroke,
        );
        text_centred(
            frame,
            &p("音量", "Volume"),
            label_font,
            palette.text,
            col_x + pad,
            y + 20.0,
            col_w - 2.0 * pad - 56.0,
            Align::Start,
        );
        let shown = self.slider_shown(Field::Volume, self.os.volume.0 as f32);
        text_centred(
            frame,
            &shown,
            title_font,
            palette.text2,
            rx - pad,
            y + 20.0,
            56.0,
            Align::End,
        );
        let (_, min, max, _, value) = self.slider(Field::Volume);
        let track = Rect {
            x: col_x + pad,
            y: y + 34.0,
            w: col_w - 2.0 * pad,
            h: 30.0,
        };
        let cy = track.y + track.h / 2.0;
        let start = track.x + SLIDER_THUMB;
        let end = track.x + track.w - SLIDER_THUMB;
        let at = start
            + if max > min {
                (value - min) / (max - min) * (end - start)
            } else {
                0.0
            };
        fill(
            frame,
            palette.switch_stroke.alpha(0.45),
            start,
            cy - 2.0,
            end - start,
            4.0,
            2.0,
        );
        fill(
            frame,
            palette.switch_on,
            start,
            cy - 2.0,
            at - start,
            4.0,
            2.0,
        );
        crate::ui::thumb::draw(
            frame,
            Point { x: at, y: cy },
            SLIDER_THUMB,
            self.slider_dots.of(Field::Volume),
            palette.switch_knob_ring,
            palette.card_stroke,
            palette.switch_on,
        );
        targets.push((track, Target::Slider(Field::Volume)));
        y += vol_h + 16.0;

        // The mute switch, whole pressed.
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
            &p("静音", "Mute"),
            label_font,
            palette.text,
            col_x + pad,
            y + h / 2.0,
            col_w - 2.0 * pad - 56.0,
            Align::Start,
        );
        switch(rx - pad - 40.0, y + h / 2.0, self.os.volume.1);
        targets.push((
            Rect {
                x: col_x,
                y,
                w: col_w,
                h,
            },
            Target::Switch(Switch::Muted),
        ));
        y += h + 16.0;

        // A word on what the left holds: every device, each adjustable of its own.
        y = heading(y, &p("这台电脑的设备", "This PC's devices"));
        let line = 18.0;
        if self.os.reading_outs.is_some() {
            text_centred(
                frame,
                &p("正在读取设备…", "Reading the devices…"),
                label_font,
                palette.text2,
                col_x + pad,
                y + line / 2.0,
                col_w - 2.0 * pad,
                Align::Start,
            );
            y += line;
        } else {
            let mut lines = 0usize;
            for out in &self.os.outs {
                let label = if out.given {
                    format!("{} · {}", out.name, p("默认", "Default"))
                } else {
                    out.name.clone()
                };
                text_centred(
                    frame,
                    &format!("{}  {label}", p("输出", "Out")),
                    label_font,
                    palette.text,
                    col_x + pad,
                    y + line / 2.0,
                    col_w - 2.0 * pad,
                    Align::Start,
                );
                y += line;
                lines += 1;
            }
            for input in &self.os.ins {
                let label = if input.given {
                    format!("{} · {}", input.name, p("默认", "Default"))
                } else {
                    input.name.clone()
                };
                text_centred(
                    frame,
                    &format!("{}  {label}", p("输入", "In")),
                    label_font,
                    palette.text,
                    col_x + pad,
                    y + line / 2.0,
                    col_w - 2.0 * pad,
                    Align::Start,
                );
                y += line;
                lines += 1;
            }
            if lines == 0 {
                text_centred(
                    frame,
                    &p("声卡没报出可用的设备。", "No working device was reported."),
                    label_font,
                    palette.text2,
                    col_x + pad,
                    y + line / 2.0,
                    col_w - 2.0 * pad,
                    Align::Start,
                );
                y += line;
            }
        }
        self.preview_content_h = y + 20.0 + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }

    /// The Bluetooth page's preview: with none picked, the adapter as it stands —
    /// whether it is on, open to discovery, and how many devices it remembers; with
    /// a device picked, its details and a button to pair and connect it through the
    /// system's own wizard. The pane scrolls; its buttons are hit through `self.targets`.
    pub(super) fn paint_bt_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
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

        let chosen = self.os.bt_pick.clone();
        let powered = self.os.bt_powered;
        let has = self.os.bt_has;
        let discoverable = self.os.bt_discoverable;
        let radios = self.os.bt_radios.clone();
        let mine = self.os.bt_devices.len();
        let mut y = area.y + 20.0 - scroll;

        match &chosen {
            // Nothing picked: the adapter as it stands, and a word to pick a device.
            None => {
                frame.icon(
                    Page::Bluetooth.icon(),
                    Point { x: cx, y: y + 24.0 },
                    40.0,
                    palette.text,
                );
                y += 24.0 + 44.0;
                let title = if !has {
                    p("没有蓝牙适配器", "No Bluetooth adapter")
                } else if !powered {
                    p("蓝牙已关闭", "Bluetooth is off")
                } else {
                    radios
                        .first()
                        .cloned()
                        .unwrap_or_else(|| p("蓝牙", "Bluetooth"))
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
                if powered {
                    y = heading(y, &p("状态", "Status"));
                    let lines = vec![
                        (p("适配器", "Adapter"), p("已开启", "On")),
                        (
                            p("可被发现", "Discoverable"),
                            if discoverable {
                                p("开", "Open")
                            } else {
                                p("关", "Hidden")
                            },
                        ),
                        (p("我的设备", "My devices"), format!("{mine}")),
                    ];
                    y = plain_card(y, &lines) + section_gap;
                }
                let hint = if powered {
                    p(
                        "在左侧点击一个设备，查看它的详情并连接。",
                        "Pick a device on the left for its details and to connect.",
                    )
                } else {
                    p(
                        "打开蓝牙后才能搜索和连接设备。",
                        "Turn Bluetooth on to seek and connect devices.",
                    )
                };
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
                    cx - 70.0,
                    y,
                    &p("蓝牙设置…", "Bluetooth settings…"),
                    Target::Jump("ms-settings:bluetooth"),
                );
                y += 30.0 + section_gap;
            }
            // A device picked: its details, and the way to pair and connect it.
            Some(address) => {
                let device = self.bt_device(address);
                let name = device
                    .as_ref()
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| p("未知设备", "Unknown device"));
                let connected = device.as_ref().map(|d| d.connected).unwrap_or(false);
                let paired = device.as_ref().map(|d| d.paired).unwrap_or(false);
                let head = Font::new(Family::SegoeDisplay, 17.0, 600.0);
                let m = frame.gfx.measure(&name, head).min(col_w);
                text_centred(
                    frame,
                    &name,
                    head,
                    palette.text,
                    cx - m / 2.0,
                    y,
                    m,
                    Align::Start,
                );
                y += 34.0;

                y = heading(y, &p("详细信息", "Details"));
                let state = if connected {
                    p("已连接", "Connected")
                } else if paired {
                    p("已配对，未连接", "Paired, not connected")
                } else {
                    p("未配对", "Not paired")
                };
                let lines = vec![
                    (p("名称", "Name"), name.clone()),
                    (p("地址", "Address"), address.clone()),
                    (p("状态", "Status"), state),
                ];
                y = plain_card(y, &lines) + section_gap;

                // The word a last pairing or switch left here.
                if let Some(said) = &self.os.bt_said {
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

                // Pair and connect through the system's own wizard; if it is connected
                // already the button still re-opens that wizard.
                y = heading(y, &p("操作", "Actions"));
                let connect = if connected {
                    p("重新连接", "Reconnect")
                } else {
                    p("连接", "Connect")
                };
                let _ = pill(
                    &mut targets,
                    col_x,
                    y,
                    &connect,
                    Target::BtPair(address.clone(), name.clone()),
                );
                y += 30.0 + section_gap;

                // And a way back to the whole page, and into Windows' own page.
                let _ = pill(
                    &mut targets,
                    col_x,
                    y,
                    &p("返回", "Back"),
                    Target::BtPick(String::new()),
                );
                let bw = frame.gfx.measure(&p("返回", "Back"), btn_font) + 28.0;
                let _ = pill(
                    &mut targets,
                    col_x + bw + 10.0,
                    y,
                    &p("蓝牙设置…", "Bluetooth settings…"),
                    Target::Jump("ms-settings:bluetooth"),
                );
                y += 30.0 + section_gap;
            }
        }

        self.preview_content_h = y + 20.0 + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }

    /// One adapter's whole story on the right: above, the network as it is now —
    /// its link, its speed, and the traffic over it as a live line; below, the
    /// numbers it was given, each of the settable ones with a button to change it
    /// (raised, so it may take). Or, when they are asked for, the connections open
    /// now. The pane scrolls; its buttons are hit through `self.targets`.
    pub(super) fn paint_network_preview(&mut self, frame: &Frame, palette: &Palette, area: Rect) {
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

        // The adapter shown: the one picked on the left, else the one carrying
        // traffic now, else the first read. Its name is the alias `netsh` takes.
        let live_name = crate::app().controller.history.read().ok().and_then(|h| {
            h.back()
                .and_then(|s| s.network.as_ref().map(|n| n.name.clone()))
        });
        let chosen = self
            .os
            .net_pick
            .clone()
            .or_else(|| live_name.clone())
            .or_else(|| self.os.adapters.first().map(|a| a.name.clone()));
        let card = chosen
            .as_ref()
            .and_then(|name| self.os.adapters.iter().find(|a| &a.name == name).cloned());

        // Its name leads, but with room above it: unlike the wireless and the
        // Bluetooth, this page shows no icon over the title, so it starts lower.
        let mut y = area.y + 34.0 - scroll;
        let title = card
            .as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_else(|| p("无连接", "No link"));
        {
            let head = Font::new(Family::SegoeDisplay, 16.0, 600.0);
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
        }
        y += 34.0;

        if self.os.net_conns {
            // The lines `netstat` read: its protocol and state, then its two ends
            // and the process holding it.
            y = heading(y, &p("活跃连接", "Active connections"));
            let rows: Vec<(String, String)> = if self.os.reading_conns.is_some()
                && self.os.conns.is_empty()
            {
                vec![(String::new(), p("正在读取…", "Reading…"))]
            } else if self.os.conns.is_empty() {
                vec![(
                    String::new(),
                    p("没有活动的连接。", "No active connections."),
                )]
            } else {
                self.os
                    .conns
                    .iter()
                    .take(60)
                    .map(|c| {
                        let left = format!("{} {}", c.proto, c.state).trim().to_string();
                        let right = format!("{} → {} · {} {}", c.local, c.foreign, "PID", c.pid);
                        (left, right)
                    })
                    .collect()
            };
            y = plain_card(y, &rows) + section_gap;
        } else if let Some(card) = &card {
            // The truth of it now: the link, the speed, and the traffic this moment.
            let sample = crate::app()
                .controller
                .history
                .read()
                .unwrap()
                .back()
                .cloned();
            let mut status: Vec<(String, String)> = Vec::new();
            status.push((
                p("状态", "Status"),
                if card.up {
                    p("已连接", "Connected")
                } else {
                    p("未连接", "Not connected")
                },
            ));
            if !card.desc.is_empty() {
                status.push((p("设备", "Device"), card.desc.clone()));
            }
            status.push((
                p("链路", "Link"),
                if card.link_bps > 0 {
                    crate::ui::text::link_speed(card.link_bps)
                } else {
                    "—".into()
                },
            ));
            if let Some(s) = &sample {
                status.push((
                    p("下载", "Down"),
                    crate::ui::text::rate(s.net_down.unwrap_or(0.0), false),
                ));
                status.push((
                    p("上传", "Up"),
                    crate::ui::text::rate(s.net_up.unwrap_or(0.0), false),
                ));
                status.push((
                    p("累计", "Total"),
                    format!(
                        "{} · {}",
                        crate::ui::text::bytes(s.net_total_down as f64),
                        crate::ui::text::bytes(s.net_total_up as f64)
                    ),
                ));
            }
            y = heading(y, &p("网络状况", "Now"));
            y = plain_card(y, &status) + section_gap;

            // Its traffic as a live line: down solid in the accent, up dashed in
            // grey, over the last few dozen readings.
            {
                let history = crate::app().controller.history.read().unwrap();
                let all = crate::panel::contiguous(&history);
                let n = all.len().min(90);
                let hist_h = 68.0;
                let ch = hist_h + 2.0 * pad;
                fill(frame, palette.card, col_x, y, col_w, ch, 10.0);
                stroke_inside(
                    frame,
                    Rect {
                        x: col_x,
                        y,
                        w: col_w,
                        h: ch,
                    },
                    10.0,
                    palette.card_stroke,
                );
                let sx = col_x + pad;
                let sw = col_w - 2.0 * pad;
                let sy = y + pad;
                if n >= 2 {
                    let view = &all[all.len() - n..];
                    let step = sw / (n as f32 - 1.0);
                    let most = view
                        .iter()
                        .flat_map(|s| [s.net_down.unwrap_or(0.0), s.net_up.unwrap_or(0.0)])
                        .fold(0.0f64, f64::max)
                        .max(1.0);
                    let at = |which: bool| {
                        view.iter()
                            .enumerate()
                            .map(|(i, s)| {
                                let v = if which {
                                    s.net_down.unwrap_or(0.0)
                                } else {
                                    s.net_up.unwrap_or(0.0)
                                };
                                Point {
                                    x: sx + i as f32 * step,
                                    y: sy + hist_h - hist_h * (v / most) as f32,
                                }
                            })
                            .collect::<Vec<Point>>()
                    };
                    let down = at(true);
                    let up = at(false);
                    let mut area_fill = down.clone();
                    area_fill.push(Point {
                        x: sx + sw,
                        y: sy + hist_h,
                    });
                    area_fill.push(Point {
                        x: sx,
                        y: sy + hist_h,
                    });
                    Canvas::fill_shape(
                        frame,
                        &area_fill,
                        crate::ui::canvas::Fill::Solid(palette.control),
                    );
                    Canvas::stroke(frame, &down, palette.switch_on, 1.5);
                    Canvas::stroke_dashed(frame, &up, palette.text3, 1.2);
                }
                y += ch + section_gap;
            }

            // The numbers it was given, the settable ones with a button of their own.
            let servers: Vec<String> = card.dns.split_whitespace().map(str::to_string).collect();
            let alias = card.name.clone();
            let edit = p("编辑", "Edit");
            let dash = |o: Option<String>| o.unwrap_or_else(|| "—".into());
            let detail: Vec<(String, String, Option<Target>)> = vec![
                (
                    p("IP 地址", "IP address"),
                    dash(card.ipv4.clone()),
                    Some(Target::NetEdit(alias.clone(), NetField::Ip)),
                ),
                (
                    p("子网掩码", "Subnet mask"),
                    if card.mask.is_empty() {
                        "—".into()
                    } else {
                        card.mask.clone()
                    },
                    Some(Target::NetEdit(alias.clone(), NetField::Mask)),
                ),
                (
                    p("默认网关", "Gateway"),
                    dash(card.gateway.clone()),
                    Some(Target::NetEdit(alias.clone(), NetField::Gateway)),
                ),
                (
                    p("首选 DNS", "Preferred DNS"),
                    servers.first().cloned().unwrap_or_else(|| "—".into()),
                    Some(Target::NetEdit(alias.clone(), NetField::Dns1)),
                ),
                (
                    p("备用 DNS", "Alternate DNS"),
                    servers.get(1).cloned().unwrap_or_else(|| "—".into()),
                    Some(Target::NetEdit(alias.clone(), NetField::Dns2)),
                ),
                (
                    p("物理地址", "MAC"),
                    if card.mac.is_empty() {
                        "—".into()
                    } else {
                        card.mac.clone()
                    },
                    None,
                ),
                (
                    p("获取方式", "Configured"),
                    if card.dhcp {
                        p("自动 (DHCP)", "Automatic (DHCP)")
                    } else {
                        p("手动", "Manual")
                    },
                    None,
                ),
                (p("接口索引", "Index"), card.index.to_string(), None),
            ];
            y = heading(y, &p("详细信息", "Details"));
            let hh = 2.0 * pad + detail.len() as f32 * (line + row_gap) - row_gap;
            fill(frame, palette.card, col_x, y, col_w, hh, 10.0);
            stroke_inside(
                frame,
                Rect {
                    x: col_x,
                    y,
                    w: col_w,
                    h: hh,
                },
                10.0,
                palette.card_stroke,
            );
            let label_w = detail
                .iter()
                .map(|(l, _, _)| frame.gfx.measure(l, label_font))
                .fold(0.0f32, f32::max);
            let value_x = col_x + pad + label_w + 16.0;
            let mut ry = y + pad;
            for (label, value, action) in &detail {
                text_centred(
                    frame,
                    label,
                    label_font,
                    palette.text2,
                    col_x + pad,
                    ry + line / 2.0,
                    label_w,
                    Align::Start,
                );
                let mut vend = rx - pad;
                if let Some(target) = action {
                    let bw = frame.gfx.measure(&edit, btn_font) + 24.0;
                    let bh = 24.0;
                    let bx = rx - pad - bw;
                    let by = ry - (bh - line) / 2.0;
                    fill(frame, palette.control, bx, by, bw, bh, 7.0);
                    stroke_inside(
                        frame,
                        Rect {
                            x: bx,
                            y: by,
                            w: bw,
                            h: bh,
                        },
                        7.0,
                        palette.card_stroke,
                    );
                    text_centred(
                        frame,
                        &edit,
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
                        target.clone(),
                    ));
                    vend = bx - 10.0;
                }
                let vw = (vend - value_x).max(40.0);
                let mw = frame.gfx.measure(value, value_font).min(vw);
                text_centred(
                    frame,
                    value,
                    value_font,
                    palette.text,
                    vend - mw,
                    ry + line / 2.0,
                    mw,
                    Align::Start,
                );
                ry += line + row_gap;
            }
            y += hh + section_gap;

            // What the page can do to this adapter as a whole.
            y = heading(y, &p("操作", "Actions"));
            let mut ax = col_x;
            let acts = [
                (
                    p("改为自动 (DHCP)", "Go automatic"),
                    Target::NetDhcp(alias.clone()),
                ),
                (
                    p("释放并续租", "Renew lease"),
                    Target::NetRenew(alias.clone()),
                ),
            ];
            for (label, target) in acts {
                let bw = frame.gfx.measure(&label, btn_font) + 28.0;
                let bh = 30.0;
                fill(frame, palette.control, ax, y, bw, bh, 8.0);
                stroke_inside(
                    frame,
                    Rect {
                        x: ax,
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
                    ax + 14.0,
                    y + bh / 2.0,
                    bw - 28.0,
                    Align::Start,
                );
                targets.push((
                    Rect {
                        x: ax,
                        y,
                        w: bw,
                        h: bh,
                    },
                    target,
                ));
                ax += bw + 10.0;
            }
            y += 30.0 + section_gap;
        } else {
            y = heading(y, &p("网络状况", "Now"));
            y = plain_card(
                y,
                &[(
                    String::new(),
                    p("没找到可用的网络适配器。", "No working adapter was found."),
                )],
            ) + section_gap;
        }

        self.preview_content_h = y + 20.0 + scroll - area.y;
        frame.unclip();
        self.targets.extend(targets);
    }
}
