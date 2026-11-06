//! The stage drawn for the doors: the dock along an edge, the launchpad's grid of programs, the overlay of
//! what was seen last, and what plays over a whole screen.

use super::*;

impl Ui {

    /// The dock's own preview, as the other pages' are: the desktop the stage shows, and the dock standing along its
    /// edge at the size the settings say — with the shell's own pictures of what is kept on it, where it has any.
    pub(super) fn paint_dock_preview(&mut self, frame: &Frame, area: Rect) {
        fill(
            frame,
            self.palette.card,
            area.x,
            area.y,
            area.w,
            area.h,
            8.0,
        );
        stroke_inside(frame, area, 8.0, self.palette.card_stroke);
        let lang = self.lang;
        let dock = &self.settings.dock;
        use crate::ui::dock::{Act, Entry};
        use crate::settings::{DockDoor, DockItem};
        let mut entries: Vec<Entry> = dock
            .items
            .iter()
            .map(|item| match item {
                DockItem::Keep { target, name } => Entry {
                    act: Act::Launch(target.clone()),
                    name: name.clone(),
                    picture: None,
                    running: false,
                },
                DockItem::Door { door } => Entry {
                    act: Act::Door(*door),
                    name: crate::ui::dock::door_says(*door, lang).to_string(),
                    picture: None,
                    // (This window is Glance's settings, and the one thing here known to be running.)
                    running: dock.running && *door == DockDoor::Settings,
                },
                DockItem::Trash => Entry {
                    act: Act::Trash,
                    name: crate::ui::dock::trash_says(lang).to_string(),
                    picture: None,
                    running: false,
                },
                DockItem::Gap => Entry {
                    act: Act::Gap,
                    name: String::new(),
                    picture: None,
                    running: false,
                },
            })
            .collect();
        let (whole, spacing) = dock.measures();
        let peak = dock.peak();
        let at_top = dock.edge == DockEdge::Top;
        let frost = 1.0 - 0.7 * dock.clear.clamp(0.0, 1.0);
        let room = |entries: &[Entry], size: f32, spacing: f32| {
            crate::ui::dock::row(entries, None, size, spacing, peak)
        };
        // (Read at the tile's full side, whatever the stage shrinks it to: one reading, kept, drawn as small as
        // need be.)
        let ask = ((whole * self.scale).round() as u32).clamp(32, 256);
        for entry in entries.iter_mut() {
            if let Act::Launch(target) = &entry.act {
                entry.picture = self.dock_pic(&target.clone(), ask);
            }
        }
        let glass = crate::ui::overlay::Glass {
            tone: if self.dark {
                crate::ui::overlay::Tone::Dark
            } else {
                crate::ui::overlay::Tone::Light
            },
            opacity: frost,
            presence: 1.0,
        };
        let (sw, sh) = self.stage.size;
        if sw > 0.0 && sh > 0.0 {
            // (The stage as the other pages show it: the whole screen fitted to the area, the dock along its edge —
            // too wide for the screen it stands on, too wide here as well.)
            let fit = (area.w / sw).min(area.h / sh).min(1.0);
            let (ox, oy) = (
                area.x + (area.w - sw * fit) / 2.0,
                area.y + (area.h - sh * fit) / 2.0,
            );
            self.stage_picture(frame, area, (ox, oy), fit, 1.0);
            let (size, spacing) = (whole * fit, spacing * fit);
            let row = room(&entries, size, spacing);
            let x = ox + (sw * fit - row.length) / 2.0;
            let y = if at_top {
                oy + crate::ui::dock::MARGIN * fit
            } else {
                (oy + sh * fit - row.thickness - crate::ui::dock::MARGIN * fit)
                    .max(oy + crate::ui::dock::MARGIN * fit)
            };
            frame.origin(x, y);
            crate::ui::dock::shadow(frame, &row, glass, false);
            crate::ui::dock::paint(
                frame, &entries, &row, glass, size, at_top, None, false, self.scale,
            );
            frame.origin(0.0, 0.0);
            return;
        }
        // (No stage read yet: the row on its own, as it was before the desktop came under it.)
        let rest = room(&entries, whole, spacing);
        let span = (rest.length - 2.0 * crate::ui::dock::PAD).max(1.0);
        let fit =
            ((area.w - 2.0 * crate::ui::dock::MARGIN - 2.0 * crate::ui::dock::PAD) / span).clamp(0.25, 1.0);
        let (size, spacing) = (whole * fit, spacing * fit);
        let row = room(&entries, size, spacing);
        let inset = 2.0 * crate::ui::dock::MARGIN;
        let x = area.x + (area.w - row.length) / 2.0;
        let y = if at_top {
            area.y + inset
        } else {
            (area.y + area.h - row.thickness - inset).max(area.y + inset)
        };
        frame.origin(x, y);
        crate::ui::dock::shadow(frame, &row, glass, false);
        crate::ui::dock::paint(
            frame, &entries, &row, glass, size, at_top, None, false, self.scale,
        );
        frame.origin(0.0, 0.0);
    }

    /// The launchpad's page as its settings ask it: the screen fitted to the card, its tiles lying in the columns and
    /// rows asked for — each as big as the page lets it be (see `crate::ui::launchpad::Grid::asked`).
    pub(super) fn paint_launchpad_preview(&mut self, frame: &Frame, area: Rect) {
        fill(
            frame,
            self.palette.card,
            area.x,
            area.y,
            area.w,
            area.h,
            8.0,
        );
        stroke_inside(frame, area, 8.0, self.palette.card_stroke);
        let dock = &self.settings.dock;
        // (The screen in its DIPs, as the launchpad lays a page on it; where the stage has not been read yet, one
        // of every day.)
        let (sw, sh) = self.stage.size;
        let (wide, tall) = if sw > 1.0 && sh > 1.0 {
            (sw / self.scale, sh / self.scale)
        } else {
            (1920.0, 1080.0)
        };
        // (The dots' line as the launchpad itself works it out: at the screen's feet, or lifted clear of the
        // dock's window where the dock stands at the foot — glass, swell room and all — and the grid gives that
        // band up just as it does there.)
        let foot = crate::ui::launchpad::feet(tall);
        let dots_at = if dock.on && dock.edge == DockEdge::Bottom && !dock.auto_hide {
            let (size, _) = dock.measures();
            let thickness = size + 2.0 * crate::ui::dock::PAD;
            let top = tall - crate::ui::dock::INSET - thickness;
            (top - crate::ui::dock::overhang(size, dock.peak()).1 - 10.0).clamp(foot - 320.0, foot)
        } else {
            foot
        };
        let grid = crate::ui::launchpad::Grid::asked(
            wide,
            crate::ui::launchpad::holds(tall, dots_at),
            35,
            dock.pad_cols,
            dock.pad_rows,
        );
        // (The whole page shrunk into the card, as the other pages fit the screen under them.)
        let inset = 20.0 * self.scale;
        let fit = ((area.w - 2.0 * inset) / wide).min((area.h - 2.0 * inset) / tall);
        let (ox, oy) = (
            area.x + (area.w - wide * fit) / 2.0,
            area.y + (area.h - tall * fit) / 2.0,
        );
        let (ww, hh) = (wide * fit, tall * fit);
        fill(frame, self.palette.window, ox, oy, ww, hh, 6.0);
        stroke_inside(
            frame,
            Rect {
                x: ox,
                y: oy,
                w: ww,
                h: hh,
            },
            6.0,
            self.palette.card_stroke,
        );
        // (The search at the page's head, the tiles between, and the pages' dots at its foot.)
        let (pw, ph) = (200.0 * fit, 26.0 * fit);
        fill(
            frame,
            self.palette.control,
            ox + (ww - pw) / 2.0,
            oy + 46.0 * fit,
            pw,
            ph,
            ph / 2.0,
        );
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let side = grid.side * fit;
                fill(
                    frame,
                    self.palette.control,
                    ox + (grid.from.0 + col as f32 * grid.pitch.0) * fit,
                    oy + (grid.from.1 + row as f32 * grid.pitch.1) * fit,
                    side,
                    side,
                    side * 0.225,
                );
            }
        }
        let (d, gap) = (5.0 * fit, 12.0 * fit);
        for i in 0..3 {
            fill(
                frame,
                if i == 0 {
                    self.palette.text
                } else {
                    self.palette.text3
                },
                ox + (ww - (3.0 * d + 2.0 * gap)) / 2.0 + i as f32 * (d + gap),
                oy + dots_at * fit,
                d,
                d,
                d / 2.0,
            );
        }
    }

    /// The shell's picture of one kept program, read once and kept from then on (see `dock_pics`).
    fn dock_pic(&mut self, target: &str, side: u32) -> Option<crate::ui::canvas::Picture> {
        if let Some((_, _, found)) = self
            .dock_pics
            .iter()
            .find(|(here, at, _)| here == target && *at == side)
        {
            return found.clone();
        }
        let made = crate::dock::pictured(target, side);
        self.dock_pics
            .push((target.to_string(), side, made.clone()));
        made
    }

    pub(super) fn paint_overlay_preview(&mut self, frame: &Frame, area: Rect) {
        let overlay = &self.settings.overlay;
        let readings = {
            let history = crate::app().controller.history.read().unwrap();
            let Some(sample) = history.back() else {
                fill(
                    frame,
                    self.palette.card,
                    area.x,
                    area.y,
                    area.w,
                    area.h,
                    8.0,
                );
                stroke_inside(frame, area, 8.0, self.palette.card_stroke);
                return;
            };
            crate::ui::overlay::preview(sample, &overlay.chosen(), self.lang)
        };
        let frames = crate::ui::overlay::preview_frames();
        let (sw, sh) = self.stage.size;
        let size = overlay.size.clamp(SIZES.0, SIZES.1);
        let width = |text: &str, font| frame.gfx.measure(text, font);
        let line = |font| frame.gfx.baseline(font);
        let metrics = crate::ui::overlay::Metrics {
            width: &width,
            line: &line,
        };
        let shape = overlay.shape((sw - 2.0 * crate::ui::overlay::INSET) / size);
        let glass = |k: f32| {
            let px = self.scale * k * size;
            (
                px,
                crate::ui::overlay::size(&readings, shape, px, &metrics),
                crate::ui::overlay::radius(&readings, shape, px, &metrics),
            )
        };
        // At its own size, as on screen: the part of the screen around it
        // that the preview holds (all of a screen smaller than that); smaller
        // only where the overlay, shadow and all, is wider or taller than
        // the preview.
        let k = (area.w / sw).min(area.h / sh).max(1.0);
        let (_, (gw, gh), _) = glass(k);
        let room = 2.0 * crate::ui::overlay::MARGIN;
        let k = k
            .min(area.w / (gw * size + room))
            .min(area.h / (gh * size + room));
        let (px, (gw, gh), _) = glass(k);
        let (w, h) = (gw * size, gh * size);
        let inset = crate::ui::overlay::INSET;
        let x = inset + overlay.at.0.clamp(0.0, 1.0) * (sw - 2.0 * inset - w).max(0.0);
        let y = inset + overlay.at.1.clamp(0.0, 1.0) * (sh - 2.0 * inset - h).max(0.0);
        let view = |at: f32, length: f32, room: f32, extent: f32| {
            (at + length / 2.0 - room / 2.0 / k).clamp(0.0, (extent - room / k).max(0.0))
        };
        let (vx, vy) = (view(x, w, area.w, sw), view(y, h, area.h, sh));
        let (ox, oy) = (
            area.x - vx * k + ((area.w - sw * k) / 2.0).max(0.0),
            area.y - vy * k + ((area.h - sh * k) / 2.0).max(0.0),
        );
        // Framed round what of the screen shows, as the panel's preview is (see `shown_of`): a screen drawn smaller
        // than the room left no plain card round it, its corners square inside the card's rounded ones.
        let area = self.shown_of(area, k, ox - area.x, oy - area.y);
        fill(
            frame,
            self.palette.card,
            area.x,
            area.y,
            area.w,
            area.h,
            8.0,
        );
        // (Inside the preview's rounded corners.)
        let Ok(mask) = (unsafe {
            frame
                .gfx
                .factory
                .CreateRoundedRectangleGeometry(&rounded(area, 8.0))
        }) else {
            return;
        };
        let layer = D2D1_LAYER_PARAMETERS1 {
            contentBounds: rect(area.x, area.y, area.w, area.h),
            geometricMask: std::mem::ManuallyDrop::new(windows::core::Interface::cast(&mask).ok()),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: 1.0,
            ..Default::default()
        };
        unsafe { frame.dc.PushLayer(&layer, None) };
        if self
            .stage
            .bitmap
            .as_ref()
            .is_none_or(|(key, _)| *key != 1.0f32.to_bits())
        {
            self.stage.bitmap = self
                .stage
                .desktop
                .as_ref()
                .and_then(|desktop| desktop.bitmap(&frame.dc, 1.0).ok())
                .map(|b| (1.0f32.to_bits(), b));
        }
        if let Some((_, bitmap)) = &self.stage.bitmap {
            unsafe {
                frame.dc.DrawBitmap(
                    bitmap,
                    Some(&rect(ox, oy, sw * k, sh * k)),
                    1.0,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    None,
                    None,
                );
            }
        }
        // The glass, tinted for the screen around it, over the screen behind
        // it blurred as the compositor blurs it.
        let screen = Matrix3x2::scale(k, k) * Matrix3x2::translation(ox, oy);
        let placed = Matrix3x2::scale(size, size) * Matrix3x2::translation(x, y) * screen;
        let around = crate::ui::overlay::MARGIN + 16.0;
        let whole = |v: f32| v.round() as i32;
        let hole = RECT {
            left: whole(x),
            top: whole(y),
            right: whole(x + w),
            bottom: whole(y + h),
        };
        let ring = RECT {
            left: whole(x - around),
            top: whole(y - around),
            right: whole(x + w + around),
            bottom: whole(y + h + around),
        };
        let frost = 1.0 - overlay.clear.clamp(0.0, 1.0);
        let glass = crate::ui::overlay::Glass {
            presence: frost,
            ..self.stage.glass(ring, hole)
        };
        // (Its tint alone over the screen as it is: the overlay does not blur what is behind it.)
        frame.place(placed);
        frame.crisp_text();
        crate::ui::overlay::shadow(frame, &readings, shape, glass, false, px);
        crate::ui::overlay::paint(
            frame,
            &readings,
            &frames,
            shape,
            glass,
            crate::ui::overlay::Hand::Free,
            px,
        );
        frame.origin(0.0, 0.0);
        unsafe { frame.dc.PopLayer() };
        drop(std::mem::ManuallyDrop::into_inner(layer.geometricMask));
        stroke_inside(frame, area, 8.0, self.palette.card_stroke);
    }

    /// What plays, with the elements chosen, slid out from the edge that opens it on the screen the window opened on
    /// (as large as it was made there: see `MusicRooms`), as the readings' and the workbench's previews show theirs;
    /// the example while no music plays. Returns how soon it is to be drawn again (as the preview's charts' speed: see
    /// `paint`), while something plays.
    pub(super) fn paint_music_preview(&mut self, frame: &Frame, area: Rect) -> Option<f32> {
        let edge = self.preview_edge();
        let (sw, sh) = self.stage.size;
        let shows = crate::ui::music::Shows::from(&self.settings.view);
        let size = crate::ui::music::rests_at(self.settings.music_rooms.at(edge == Edge::Top), &shows);
        let zoom = self.settings.panel_size.clamp(SIZES.0, SIZES.1);
        let (pw, ph) = (size.0 * zoom, size.1 * zoom);
        let inset = Theme::new(Skin::named(&self.settings.skin), false).inset * zoom;
        let along = |at: f32, length: f32, extent: f32| {
            (at - length / 2.0).min(extent - GAP - length).max(GAP)
        };
        let pos = match edge {
            Edge::Left => (inset, along(sh / 2.0, ph, sh)),
            Edge::Right => (sw - inset - pw, along(sh / 2.0, ph, sh)),
            Edge::Top => (along(sw / 2.0, pw, sw), inset),
        };
        let (k, stage_x, stage_y) = self.staged(area, edge, pw, false);
        let (ox, oy) = (area.x + stage_x, area.y + stage_y);
        let area = self.shown_of(area, k, stage_x, stage_y);
        let palette = self.palette.clone();
        fill(frame, palette.card, area.x, area.y, area.w, area.h, 8.0);
        self.stage_picture(frame, area, (ox, oy), k, zoom);
        unsafe {
            frame.dc.PushAxisAlignedClip(
                &rect(area.x, area.y, area.w, area.h),
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            )
        };
        let (at, scale) = ((ox + pos.0 * k, oy + pos.1 * k), zoom * k);
        let pointer = self
            .pointer
            .map(|(x, y)| ((x - at.0) / scale, (y - at.1) / scale))
            .filter(|(x, y)| *x >= 0.0 && *y >= 0.0 && *x < size.0 && *y < size.1);
        self.follows_pointer.push(Rect {
            x: at.0,
            y: at.1,
            w: size.0 * scale,
            h: size.1 * scale,
        });
        frame.place(Matrix3x2::scale(scale, scale) * Matrix3x2::translation(at.0, at.1));
        let ground = crate::ui::music::Ground::of(&shows);
        let now = Instant::now();
        let painted = if crate::ui::music::heard() {
            self.music.paint(
                frame,
                size,
                &shows,
                ground,
                self.scale * scale,
                pointer,
                now,
            )
        } else {
            self.music.paint_example(
                frame,
                size,
                &shows,
                ground,
                self.scale * scale,
                pointer,
                now,
                self.example_from.elapsed().as_secs_f64(),
            )
        };
        frame.origin(0.0, 0.0);
        unsafe { frame.dc.PopAxisAlignedClip() };
        stroke_inside(frame, area, 8.0, palette.card_stroke);
        (painted.busy || !crate::ui::music::heard()).then_some(MUSIC_PREVIEW_PACE)
    }
}
