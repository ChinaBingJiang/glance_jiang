//! The stage at the right of the window: the screen a page is shown on, where it stands and how large, and
//! what is drawn on it - the panel, the dock, a picture of the real screen taken a moment ago.

use super::*;

impl Ui {

    /// The 通用 hub: pick which sub-item's preview the right pane shows, without
    /// turning to its page — its readings asked just as if it were come to.
    pub(super) fn pick_preview(&mut self, page: Page) {
        self.preview = Some(page);
        self.preview_scroll = 0.0;
        self.preview_scroll_target = 0.0;
        self.read_for(page);
        self.next_frame = Instant::now();
    }

    /// The preview shows the screen named `screen`: its desktop, at its
    /// size.
    pub(super) fn show_screen(&mut self, screen: &str) {
        let Some(m) = self
            .screens
            .iter()
            .find(|known| known.id == screen)
            .map(|known| known.monitor)
        else {
            return;
        };
        let Some(contact) = crate::mouse::monitor_at(POINT {
            x: (m.left + m.right) / 2,
            y: (m.top + m.bottom) / 2,
        }) else {
            return;
        };
        let work = contact.work;
        let size = (
            ((work.right - work.left) as f32 / contact.scale).round(),
            ((work.bottom - work.top) as f32 / contact.scale).round(),
        );
        // Shown already, and as it is now (its size, scale and place the same).
        let is = |stage: &Stage| {
            stage.screen.as_deref() == Some(screen) && stage.work == work && stage.size == size
        };
        if is(&self.stage) {
            return;
        }
        // Shown before, as it is now: taken as it was kept; else read from the wallpaper on another thread (shown
        // without it meanwhile).
        let next = match self.kept_stages.iter().position(is) {
            Some(at) => self.kept_stages.swap_remove(at),
            None => {
                self.read_stage(screen.to_string(), work, size);
                Stage::new(size, Some(screen.to_string()), work, None)
            }
        };
        let before = std::mem::replace(&mut self.stage, next);
        // One kept for each screen, as it last was.
        if let Some(of) = before.screen.clone() {
            self.kept_stages
                .retain(|kept| kept.screen.as_deref() != Some(of.as_str()));
            self.kept_stages.push(before);
        }
        let known: Vec<&str> = self.screens.iter().map(|known| known.id.as_str()).collect();
        self.kept_stages
            .retain(|kept| kept.screen.as_deref().is_some_and(|id| known.contains(&id)));
    }

    /// Reads every screen's desktop as the preview shows it (its wallpaper, at its work area's size), on another
    /// thread: each kept as it comes (see `draw`).
    pub(super) fn read_stages(&mut self) {
        let wanted = stages_wanted(&self.screens);
        // As they were last read, at once (read again below, and taken if changed).
        for (screen, work, size) in &wanted {
            if let Some(desktop) = kept_desktop(screen, *work, *size) {
                self.kept_stages
                    .retain(|kept| kept.screen.as_deref() != Some(screen.as_str()));
                self.kept_stages.push(Stage::new(
                    *size,
                    Some(screen.clone()),
                    *work,
                    Some(desktop),
                ));
            }
        }
        let send = self.read.0.clone();
        std::thread::spawn(move || {
            for (screen, work, size) in wanted {
                let desktop = read_and_keep(&screen, work, size);
                let stage = Stage::new(size, Some(screen), work, Some(desktop));
                if send.send(Read::Stage(stage)).is_err() {
                    break;
                }
            }
        });
    }

    /// Reads screen `screen`'s desktop for the preview (its work area `work`, `size` DIPs) on another thread.
    fn read_stage(&self, screen: String, work: RECT, size: (f32, f32)) {
        let send = self.read.0.clone();
        std::thread::spawn(move || {
            let desktop = read_and_keep(&screen, work, size);
            let _ = send.send(Read::Stage(Stage::new(
                size,
                Some(screen),
                work,
                Some(desktop),
            )));
        });
    }

    /// The screen the preview shows, by its name: on the layout's page, the
    /// one its editor shows large; on the overlay's, the one
    /// the overlay was put on; and the one this window is on, for the
    /// other pages and until either was.
    pub(super) fn stage_screen(&self) -> Option<String> {
        let at = |point: POINT| {
            let here = crate::mouse::monitor_at(point)?.monitor;
            self.screens
                .iter()
                .find(|screen| screen.monitor == here)
                .map(|screen| screen.id.clone())
        };
        let chosen = match self.page {
            Page::Layout => self.editor_screen(),
            Page::Monitor if self.tab == Tab::Overlay => self
                .settings
                .overlay
                .screen
                .and_then(|(x, y)| at(POINT { x, y })),
            _ => None,
        };
        chosen.or_else(|| {
            let mut window = RECT::default();
            let _ = unsafe {
                windows::Win32::UI::WindowsAndMessaging::GetWindowRect(self.hwnd, &mut window)
            };
            at(POINT {
                x: (window.left + window.right) / 2,
                y: (window.top + window.bottom) / 2,
            })
        })
    }

    /// The component the preview shows: the workbench on its page, what plays on the music's, else the readings.
    fn previewed(&self) -> Part {
        match self.page {
            Page::Workbench => Part::Workbench,
            Page::Music => Part::Music,
            _ => Part::Monitor,
        }
    }

    /// The edge the preview's panel opens from, on the screen it shows: the first of that screen's that opens what it
    /// shows, else the first that opens anything; where none does, the one a panel opened by hand opens from.
    pub(super) fn preview_edge(&self) -> Edge {
        let places = self.settings.places();
        let screen = self.stage.screen.as_deref().unwrap_or_default();
        let lit = places.lit(screen);
        let part = self.previewed();
        lit.each()
            .find(|edge| places.part(screen, *edge) == Some(part))
            .or(lit.each().next())
            .unwrap_or(places.by_hand().0)
    }

    /// The desktop of the screen named `id` at `monitor`, `size` physical
    /// pixels large, for the layout's editor: taken once, and again when its
    /// size changes.
    pub(super) fn map_picture(
        &mut self,
        frame: &Frame,
        id: &str,
        monitor: RECT,
        size: (u32, u32),
    ) -> Option<ID2D1Bitmap1> {
        // The picture held is drawn smaller or larger to the size wanted. Read again (on another thread, the one held
        // drawn meanwhile) only where it is smaller than that, or more than twice as large; and not while the window
        // is sized by hand (each size of it would read the desktop again): once it is let go.
        let holds = |asked: (u32, u32)| {
            asked.0 >= size.0 && asked.1 >= size.1 && asked.0 <= size.0 * 2 && asked.1 <= size.1 * 2
        };
        let found = self
            .map_art
            .iter()
            .position(|art| art.id == id && art.monitor == monitor);
        let at = match found {
            Some(at) if holds(self.map_art[at].asked) || self.by_hand => at,
            _ => {
                let send = self.read.0.clone();
                let of = id.to_string();
                std::thread::spawn(move || {
                    let desktop = read_desktop(monitor, size.0, size.1);
                    let _ = send.send(Read::Map {
                        id: of,
                        monitor,
                        size,
                        desktop,
                    });
                });
                match found {
                    Some(at) => {
                        self.map_art[at].asked = size;
                        at
                    }
                    None => {
                        self.map_art.retain(|art| art.id != id);
                        self.map_art.push(MapArt {
                            id: id.to_string(),
                            monitor,
                            asked: size,
                            desktop: None,
                            bitmap: None,
                        });
                        self.map_art.len() - 1
                    }
                }
            }
        };
        let art = &mut self.map_art[at];
        if art.bitmap.is_none() {
            art.bitmap = art
                .desktop
                .as_ref()
                .and_then(|desktop| desktop.bitmap(&frame.dc, 1.0).ok());
        }
        art.bitmap.clone()
    }

    /// The preview: the panel as it will look over the desktop, running live.
    pub(super) fn paint_preview(
        &mut self,
        frame: &Frame,
        palette: &Palette,
        width: f32,
        height: f32,
    ) -> Option<f32> {
        let lang = self.lang;
        let x0 = NAV + self.pane();
        let head = Font::new(Family::SegoeDisplay, 20.0, 600.0);
        let title = if self.page == Page::Layout {
            pick(lang, "屏幕", "Screens")
        } else if self.page == Page::System {
            // The 通用 hub takes the picked sub-item's own name; till one is picked
            // it is the hub's own readings pane, named for the page.
            self.preview
                .map(|page| page.name(lang))
                .unwrap_or(pick(lang, "通用", "General"))
        } else {
            pick(lang, "预览", "Preview")
        };
        text_centred(
            frame,
            title,
            head,
            palette.text,
            x0,
            PAD_TOP + 18.0,
            200.0,
            Align::Start,
        );
        let detail = match self.page {
            Page::Layout => pick(
                lang,
                "屏幕的排列与 Windows 设置一致，桌面为当前壁纸",
                "The screens as Windows arranges them, over your wallpaper",
            ),
            Page::Phone => pick(lang, "手机上的 Glance", "Glance on the phone"),
            Page::Music => pick(lang, "正在播放的内容", "What plays now"),
            Page::Dock => pick(lang, "屏幕边缘的图标栏", "Icons along the screen's edge"),
            Page::Launchpad => pick(lang, "每页程序的排布", "How a page of programs lies"),
            Page::Account => pick(lang, "这台电脑上的账户", "The account on this PC"),
            _ => pick(lang, "实时数据", "Live readings"),
        };
        let title_w = frame.gfx.measure(title, head);
        let area = Rect {
            x: x0,
            y: PAD_TOP + 40.0,
            w: width - x0 - PAD_SIDE,
            h: height - PAD_TOP - 40.0 - PAD_TOP,
        };
        // (No wider than the preview under it: where it is narrow, cut short with an ellipsis.)
        let detail_x = x0 + title_w + 12.0;
        text_centred(
            frame,
            detail,
            Font::new(Family::Segoe, 12.0, 400.0),
            palette.text2,
            detail_x,
            PAD_TOP + 20.0,
            (area.x + area.w - detail_x).max(0.0),
            Align::Start,
        );
        if area.w <= 0.0 || area.h <= 0.0 {
            return None;
        }
        // The 通用 hub shows whichever sub-item is picked, drawn as that page's own
        // preview without turning to its page; with none picked it shows its own
        // readings — cards copied from the monitor panel's, drawn at the pane's size
        // and centred — not the monitor's preview shrunk to a screen's miniature.
        // The page is set only for the drawing, then put back.
        if self.page == Page::System {
            return match self.preview {
                Some(page) => {
                    let saved = self.page;
                    self.page = page;
                    let out = self.paint_preview_shown(frame, palette, area);
                    self.page = saved;
                    out
                }
                None => self.paint_general_preview(frame, palette, area),
            };
        }
        self.paint_preview_shown(frame, palette, area)
    }

    /// The preview for the page now in `self.page`, once the hub has resolved
    /// whose preview to show.
    fn paint_preview_shown(&mut self, frame: &Frame, palette: &Palette, area: Rect) -> Option<f32> {
        // Each of the PC's own categories will carry a preview of its own as it
        // is brought across; until then its room stays a plain card.
        if !matches!(
            self.page,
            Page::Layout
                | Page::Look
                | Page::Monitor
                | Page::Workbench
                | Page::Dock
                | Page::Launchpad
                | Page::Music
                | Page::Phone
                | Page::General
                | Page::Reports
        ) {
            self.paint_os_preview(frame, palette, area);
            return None;
        }
        // (The panel's and the music's previews frame what of the screen they show: see `shown_of`.)
        let overlay = self.page == Page::Monitor && self.tab == Tab::Overlay;
        if matches!(self.page, Page::Layout | Page::Phone) {
            fill(frame, palette.card, area.x, area.y, area.w, area.h, 8.0);
            stroke_inside(frame, area, 8.0, palette.card_stroke);
        }

        if overlay {
            self.paint_overlay_preview(frame, area);
            return None;
        }
        if self.page == Page::Dock {
            self.paint_dock_preview(frame, area);
            return None;
        }
        if self.page == Page::Launchpad {
            self.paint_launchpad_preview(frame, area);
            return None;
        }
        if self.page == Page::Music {
            return self.paint_music_preview(frame, area);
        }
        if self.page == Page::Layout {
            return self.paint_layout(frame, palette, area, Instant::now());
        }
        // The phone's page: the phone app as a phone shows it, in a phone's outline.
        if self.page == Page::Phone {
            // (Its fans turn: drawn again some thirty times a second while they do.)
            return self.paint_phone(frame, area).then_some(7.5);
        }

        // The readings (or the workbench), laid out for the screen the window opened on, centred along the edge they
        // open from.
        let prefs = self.prefs.clone().holding(self.previewed());
        let edge = self.preview_edge();
        self.panel_on_stage(frame, area, Some(edge), Some(&prefs), false)
            .0
    }

    /// How the stage is shown in `area` with a panel `pw` wide (stage DIPs) slid out from `edge`: how large a stage
    /// DIP is drawn, and where the stage's corner is in the area. The whole screen (`whole`), else the whole height of
    /// it with the whole panel and a strip of desktop beside it (along the top, either side of it).
    pub(super) fn staged(&self, area: Rect, edge: Edge, pw: f32, whole: bool) -> (f32, f32, f32) {
        let (sw, sh) = self.stage.size;
        let fit = (area.w / sw).min(area.h / sh).min(1.0);
        match (whole, edge) {
            (true, _) => (fit, (area.w - sw * fit) / 2.0, (area.h - sh * fit) / 2.0),
            (false, Edge::Left | Edge::Right) => {
                let k = (area.h / sh)
                    .min(area.w / (pw + PREVIEW_MARGIN).min(sw))
                    .min(1.0);
                // A screen that fits whole, at the area's left; one that does not, its side the panel is on.
                let x = if sw * k <= area.w || edge == Edge::Left {
                    0.0
                } else {
                    area.w - sw * k
                };
                (k, x, 0.0)
            }
            // The panel in the middle of the top, with as much desktop either side of it as the sides' previews have
            // beside theirs, and the whole height of the screen under it. (Its whole width shown, a wide screen made
            // the panel small.)
            (false, Edge::Top) => {
                let k = (area.h / sh)
                    .min(area.w / (pw + 2.0 * PREVIEW_MARGIN).min(sw))
                    .min(1.0);
                (
                    k,
                    if sw * k <= area.w {
                        0.0
                    } else {
                        (area.w - sw * k) / 2.0
                    },
                    0.0,
                )
            }
        }
    }

    /// Where the stage's whole screen is drawn in `area` (as `panel_on_stage` draws it whole).
    pub(super) fn whole_stage(&self, area: Rect) -> Rect {
        let (sw, sh) = self.stage.size;
        let (k, x, y) = self.staged(area, Edge::Left, 0.0, true);
        Rect {
            x: area.x + x,
            y: area.y + y,
            w: sw * k,
            h: sh * k,
        }
    }

    /// What of the stage shows in `area`, drawn `k` DIPs to one of its own with its corner at `(x, y)` in the area
    /// (as `staged` places it): the preview's frame is drawn round that, with no empty room in it.
    pub(super) fn shown_of(&self, area: Rect, k: f32, x: f32, y: f32) -> Rect {
        let (sw, sh) = self.stage.size;
        let (left, top) = (area.x + x.max(0.0), area.y + y.max(0.0));
        let (right, bottom) = (
            (area.x + x + sw * k).min(area.x + area.w),
            (area.y + y + sh * k).min(area.y + area.h),
        );
        Rect {
            x: left,
            y: top,
            w: right - left,
            h: bottom - top,
        }
    }

    /// The stage's desktop in `area` and, where `edge` is one, the panel showing `prefs` slid out from it, running
    /// live: the whole screen (`whole`), else the whole height of it
    /// with the whole panel and a strip of desktop beside it. Returns how fast the panel's charts move (physical pixels
    /// a second), and where the screen was drawn.
    pub(super) fn panel_on_stage(
        &mut self,
        frame: &Frame,
        area: Rect,
        edge: Option<Edge>,
        prefs: Option<&Prefs>,
        whole: bool,
    ) -> (Option<f32>, Rect) {
        let lang = self.lang;
        let (sw, sh) = self.stage.size;
        let fit = (area.w / sw).min(area.h / sh).min(1.0);
        let (Some(edge), Some(prefs)) = (edge, prefs) else {
            let (ox, oy) = (
                area.x + (area.w - sw * fit) / 2.0,
                area.y + (area.h - sh * fit) / 2.0,
            );
            self.stage_picture(frame, area, (ox, oy), fit, 1.0);
            return (
                None,
                Rect {
                    x: ox,
                    y: oy,
                    w: sw * fit,
                    h: sh * fit,
                },
            );
        };
        let app = crate::app();
        let skin = Skin::named(&self.settings.skin);
        let interval = self.settings.interval_ms as f64;
        // Held while the preview is drawn; the sampler waits that long.
        let history = app.controller.history.read().unwrap();
        let samples = crate::panel::contiguous(&history);
        if samples.is_empty() {
            if !whole {
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
            }
            return (
                None,
                Rect {
                    x: area.x,
                    y: area.y,
                    w: area.w,
                    h: area.h,
                },
            );
        }
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            * 1000.0;
        let pen = wall - interval - PEN_LAG_MS;
        let measure = Theme::new(skin, false);
        // The preview shows what the panel would hold if it opened now.
        let seen = app.controller.seen.lock().unwrap().clone();
        let probe = Scene {
            music_apart: false,
            thumbs: &crate::ui::view::RESTING,
            info: &app.info,
            prefs,
            theme: &measure,
            lang,
            history: samples,
            seen: &seen,
            pen_ms: pen,
            scrolls: crate::ui::view::Scrolls::default(),
            tab: crate::ui::view::Tab::Shelf,
            hover: None,
            pinned: false,
            overlay: crate::app().controller.overlay_wanted(),
            buttons: true,
            newer: update::available().is_some(),
            settings_lit: None,
        };
        let lanes = view::lanes(&probe);
        let heights: Vec<f32> = lanes.iter().map(|lane| lane.height(&measure)).collect();
        // (Quick launch as its rail, as the panel lays it out.)
        let rail = lanes.iter().position(|lane| lane.id() == "apps");
        self.largest = arrange::largest(
            &measure,
            edge,
            &heights,
            (sw, sh),
            self.settings.columns,
            SIZES.1,
            rail,
        );
        let size = self.settings.panel_size.clamp(SIZES.0, SIZES.1);
        let (layout, zoom) = arrange::arrange(
            &measure,
            edge,
            heights,
            (sw, sh),
            self.settings.columns,
            size,
            rail,
        );
        let (pw, ph) = (layout.width() * zoom, layout.height() * zoom);
        let inset = measure.inset * zoom;
        let along = |at: f32, length: f32, extent: f32| {
            (at - length / 2.0).min(extent - GAP - length).max(GAP)
        };
        let pos = match edge {
            Edge::Left => (inset, along(sh / 2.0, ph, sh)),
            Edge::Right => (sw - inset - pw, along(sh / 2.0, ph, sh)),
            Edge::Top => (along(sw / 2.0, pw, sw), inset),
        };
        // The desktop behind the panel decides its theme when it follows the
        // backdrop, and how frosted glass is.
        let behind = RECT {
            left: pos.0 as i32,
            top: pos.1 as i32,
            right: (pos.0 + pw) as i32,
            bottom: (pos.1 + ph) as i32,
        };
        let tone = self.stage.tone(behind);
        let dark = theme::is_dark(prefs.theme, Some(tone.0).filter(|_| skin.sees_backdrop()));
        let theme = Theme::new(skin, dark);
        let frost = if skin == Skin::Glass {
            skins::frost(tone.0, tone.1, dark)
        } else {
            0.0
        };
        let scene = Scene {
            music_apart: false,
            thumbs: &crate::ui::view::RESTING,
            info: &app.info,
            prefs,
            theme: &theme,
            lang,
            history: samples,
            seen: &seen,
            pen_ms: pen,
            scrolls: crate::ui::view::Scrolls::default(),
            tab: crate::ui::view::Tab::Shelf,
            hover: None,
            pinned: false,
            overlay: crate::app().controller.overlay_wanted(),
            buttons: true,
            newer: update::available().is_some(),
            settings_lit: None,
        };
        let lanes = view::lanes(&scene);

        let (k, stage_x, stage_y) = self.staged(area, edge, pw, whole);
        // The pane scrolls: the screen stands as far above the top as the wheel has
        // gone down, and its whole drawn height is told so the wheel knows how far
        // down to go. Where it fits the pane whole, there is nothing to scroll.
        let stage_y = stage_y - if whole { 0.0 } else { self.preview_scroll };
        self.preview_content_h = (sh * k).max(area.h);
        let (ox, oy) = (area.x + stage_x, area.y + stage_y);
        let area = if whole {
            area
        } else {
            self.shown_of(area, k, stage_x, stage_y)
        };
        if !whole {
            fill(
                frame,
                self.palette.card,
                area.x,
                area.y,
                area.w,
                area.h,
                8.0,
            );
        }
        let mask = unsafe {
            frame
                .gfx
                .factory
                .CreateRoundedRectangleGeometry(&D2D1_ROUNDED_RECT {
                    rect: rect(area.x, area.y, area.w, area.h),
                    radiusX: 8.0,
                    radiusY: 8.0,
                })
        };
        let layer = D2D1_LAYER_PARAMETERS1 {
            contentBounds: rect(area.x, area.y, area.w, area.h),
            geometricMask: std::mem::ManuallyDrop::new(
                mask.ok()
                    .map(|m| windows::core::Interface::cast(&m).unwrap()),
            ),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: 1.0,
            ..Default::default()
        };
        unsafe { frame.dc.PushLayer(&layer, None) };
        // The desktop in the panel's units, which a zoomed panel shrinks.
        self.stage_picture(frame, area, (ox, oy), k, zoom);
        let local = Matrix3x2::scale(zoom, zoom)
            * Matrix3x2::translation(pos.0, pos.1)
            * Matrix3x2::scale(k, k)
            * Matrix3x2::translation(ox, oy);
        // The stage's desktop is a pixel a DIP: a panel DIP covers `zoom` of them.
        let backdrop = self.stage.bitmap.as_ref().map(|(_, bitmap)| {
            (
                bitmap,
                Vector2 {
                    X: pos.0 / zoom,
                    Y: pos.1 / zoom,
                },
                self.stage
                    .desktop
                    .as_ref()
                    .map_or(0, |desktop| desktop.digest),
                zoom,
            )
        });
        let picture = render::Picture {
            scene: &scene,
            lanes: &lanes,
            layout: &layout,
            small: None,
            size: (layout.width(), layout.height()),
            at: (0.0, 0.0),
            edge: Some(edge),
            backdrop,
            frost,
            rough: false,
        };
        let drawn = self
            .layers
            .draw(frame, &picture, local, self.scale * zoom * k);
        unsafe { frame.dc.PopLayer() };
        drop(std::mem::ManuallyDrop::into_inner(layer.geometricMask));
        if !whole {
            stroke_inside(frame, area, 8.0, self.palette.card_stroke);
        }
        match drawn {
            Ok(drawn) if drawn.grounded => self.gfx.clear_caches(),
            Ok(_) => {}
            // (Not shown, the frame: see `Frame::fail`.)
            Err(error) => frame.fail(error),
        }
        (
            layout
                .plot_width(&theme, &lanes)
                .map(|width| width * self.scale * zoom * k / prefs.chart_seconds as f32),
            Rect {
                x: ox,
                y: oy,
                w: sw * k,
                h: sh * k,
            },
        )
    }

    /// The stage's desktop at `at`, `k` DIPs to one of its own, clipped to `area`: made a bitmap for the panel's
    /// `zoom` (a zoomed panel's units shrink it), as it is drawn under it.
    pub(super) fn stage_picture(&mut self, frame: &Frame, area: Rect, at: (f32, f32), k: f32, zoom: f32) {
        let (sw, sh) = self.stage.size;
        let zoom_key = zoom.to_bits();
        if self
            .stage
            .bitmap
            .as_ref()
            .is_none_or(|(key, _)| *key != zoom_key)
        {
            self.stage.bitmap = self
                .stage
                .desktop
                .as_ref()
                .and_then(|desktop| desktop.bitmap(&frame.dc, zoom).ok())
                .map(|b| (zoom_key, b));
        }
        if let Some((_, bitmap)) = &self.stage.bitmap {
            // (Inside the area's rounded corners.)
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
                geometricMask: std::mem::ManuallyDrop::new(
                    windows::core::Interface::cast(&mask).ok(),
                ),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: Matrix3x2::identity(),
                opacity: 1.0,
                ..Default::default()
            };
            unsafe {
                frame.dc.PushLayer(&layer, None);
                frame.dc.DrawBitmap(
                    bitmap,
                    Some(&rect(at.0, at.1, sw * k, sh * k)),
                    1.0,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    None,
                    None,
                );
                frame.dc.PopLayer();
            }
        }
    }
}
