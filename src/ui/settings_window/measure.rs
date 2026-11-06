//! How big everything is: how tall a row's words are, how much room the choices beside them take, how tall
//! the whole page comes to; and which of its sections are folded open.

use super::*;

impl Ui {

    /// Whether module `id` is listed in the fold rather than with the rest.
    pub(super) fn folded(&self, id: &str) -> bool {
        MORE.contains(&id) && !self.raised.contains(id)
    }

    /// Each row with its top and height, from the top of the list (DIPs).
    pub(super) fn layout(&self) -> Vec<(Row, f32, f32)> {
        let mut y = PAD_TOP;
        let mut after_heading = false;
        let mut placed = Vec::new();
        // The card whose rows are being placed: its module, where its rows
        // start, and how open it is. Its rows keep their places inside it;
        // what follows it moves up by the part of them still hidden.
        let mut card: Option<(String, f32, f32)> = None;
        for row in self.rows() {
            let inside = match &row {
                Row::Item(id, _) | Row::ModuleChoice(id, _) => Some(id.clone()),
                _ => None,
            };
            if card
                .as_ref()
                .is_some_and(|(id, ..)| Some(id) != inside.as_ref())
            {
                let (_, start, open) = card.take().unwrap();
                y -= (1.0 - open) * (y - start);
            }
            if let Some(id) = inside.filter(|_| card.is_none()) {
                card = Some((id.clone(), y, self.opened(&id)));
            }
            // A heading's space above it takes in the title's below; the
            // first row of a group sits 8 under its heading, the rest 4 apart.
            let (before, height) = match &row {
                Row::Title => (0.0, TITLE),
                Row::Tabs => (0.0, TABS_ROW),
                Row::Note(words) => (
                    12.0,
                    self.gfx.wrapped_height(
                        words,
                        Font::new(Family::Segoe, HINT, 400.0),
                        self.cards() - 8.0,
                    ) + 8.0,
                ),
                // A button of its own: as tall as the button, with no card round it.
                Row::Do(..) => (if after_heading { 8.0 } else { ROW_GAP }, 34.0),
                // A line read from the PC, wrapped as a note is.
                Row::Info(words) => (
                    12.0,
                    self.gfx.wrapped_height(
                        words,
                        Font::new(Family::Segoe, HINT, 400.0),
                        self.cards() - 8.0,
                    ) + 8.0,
                ),
                Row::Slider(_) => (if after_heading { 8.0 } else { ROW_GAP }, ROW),
                Row::Skins => (if after_heading { 8.0 } else { ROW_GAP }, SKINS_ROW),
                Row::Library => (
                    if after_heading { 8.0 } else { ROW_GAP },
                    self.library_height(),
                ),
                // A choice with a word on it: the word above, the choices on
                // a line of their own.
                Row::Choice(field) if self.choice_hint(*field).is_some() => {
                    let options = self.choices(*field).1;
                    let words = self.words_height(&row).max(ROW);
                    let lines = self.choice_lines(&options, self.choices_room()).len() as f32;
                    (
                        if after_heading { 8.0 } else { ROW_GAP },
                        if self.stacked(&options) {
                            words + CHOICE_LINE * lines
                        } else {
                            words
                        },
                    )
                }
                // Inside its module's card, right under the row above.
                // (With a word under its name, as tall as a card's row with one: as much room above and below them.)
                Row::Item(..) => {
                    let words = self.words_height(&row);
                    (
                        0.0,
                        if words > 0.0 {
                            words.max(ROW)
                        } else {
                            ITEM_ROW
                        },
                    )
                }
                // Its choices under its name where they do not fit beside it.
                Row::ModuleChoice(_, field) => {
                    let (name, options, _) = self.choices(*field);
                    let room = self.module_choices_room();
                    let under = if self.beside_name(name, &options, room) {
                        0.0
                    } else {
                        CHOICE_LINE * self.choice_lines(&options, room).len() as f32
                    };
                    (0.0, ITEM_ROW + under)
                }
                Row::Choice(field)
                    if self.choice_hint(*field).is_none()
                        && !self.beside_name(
                            self.choices(*field).0,
                            &self.choices(*field).1,
                            self.choices_room(),
                        ) =>
                {
                    let lines = self
                        .choice_lines(&self.choices(*field).1, self.choices_room())
                        .len() as f32;
                    (
                        if after_heading { 8.0 } else { ROW_GAP },
                        ROW + CHOICE_LINE * lines,
                    )
                }
                // As tall as the code, or as the words beside it where those are taller.
                Row::Code => {
                    let words = self.code.as_ref().map_or(0.0, |(_, what)| {
                        22.0 + self.gfx.wrapped_height(
                            &code_words(self.lang, *what, self.web_address.as_deref()).1,
                            Font::new(Family::Segoe, HINT, 400.0),
                            self.code_words_room(),
                        )
                    });
                    (ROW_GAP, CODE.max(words) + 2.0 * ROW_SIDE)
                }
                Row::Group(_) => (20.0, GROUP),
                _ => (
                    if after_heading { 8.0 } else { ROW_GAP },
                    self.words_height(&row).max(ROW),
                ),
            };
            y += before;
            after_heading = matches!(row, Row::Title);
            placed.push((row, y, height));
            y += height;
        }
        if let Some((_, start, open)) = card {
            y -= (1.0 - open) * (y - start);
        }
        let _ = y;
        placed
    }

    /// How tall a row has to be for its name and its word, the word broken
    /// into lines in the room it has beside the row's controls; none taller
    /// than a row for a row without a word.
    fn words_height(&self, row: &Row) -> f32 {
        let width = self.cards();
        let font = Font::new(Family::Segoe, 14.0, 400.0);
        let button = |text: &str| self.gfx.measure(text, font) + 32.0;
        let (word, room): (Cow<'static, str>, f32) = match row {
            Row::Switch(switch) => match self.switch(*switch).1 {
                Some(word) => (word, width - 2.0 * ROW_SIDE - 40.0 - 16.0),
                None => return 0.0,
            },
            Row::Choice(field) => {
                let Some(word) = self.choice_hint(*field) else {
                    return 0.0;
                };
                let options = self.choices(*field).1;
                let room = if self.stacked(&options) {
                    width - 2.0 * ROW_SIDE
                } else {
                    width - 2.0 * ROW_SIDE - self.segmented_width(&options) - 16.0
                };
                (Cow::Borrowed(word), room)
            }
            Row::Drive(volume) => match self.drive_row(volume).1 {
                Some(word) => (Cow::Borrowed(word), width - 2.0 * ROW_SIDE - 40.0 - 16.0),
                None => return 0.0,
            },
            Row::Element(name) => match self.item_label("lyrics", name).1 {
                Some(word) => (Cow::Borrowed(word), width - 2.0 * ROW_SIDE - 40.0 - 16.0),
                None => return 0.0,
            },
            Row::Item(id, name) => match self.item_label(id, name).1 {
                Some(word) => (
                    Cow::Borrowed(word),
                    width - ROW_SIDE - CHEVRON_ROOM - 40.0 - 12.0 - ITEM_INSET,
                ),
                None => return 0.0,
            },
            // (Its word beside the grip, left of its switch and chevron: see `module_row`.)
            Row::Module(id) => match self.module(id).1 {
                Some(word) => (
                    Cow::Owned(word),
                    width - ROW_SIDE - CHEVRON_ROOM - 40.0 - 12.0 - ITEM_INSET,
                ),
                None => return 0.0,
            },
            // Its word beside its switch, measured as a switch row's is.
            Row::Toggle(_, word, ..) => match word {
                Some(word) => (
                    Cow::Owned(word.clone()),
                    width - 2.0 * ROW_SIDE - 40.0 - 16.0,
                ),
                None => return 0.0,
            },
            // A row that leads another page: its word under its name, with the
            // chevron's room kept off its right.
            Row::Nav(_, _, hint_zh, hint_en, _) => (
                Cow::Borrowed(pick(self.lang, hint_zh, hint_en)),
                width - 2.0 * ROW_SIDE - CHEVRON_ROOM - 16.0,
            ),
            // The same, its name and word read from the machine.
            Row::Pick(_, word, _) => (
                Cow::Owned(word.clone()),
                width - 2.0 * ROW_SIDE - CHEVRON_ROOM - 16.0,
            ),
            _ => match self.button_row(row, Instant::now()) {
                Some((_, word, buttons)) => {
                    let taken: f32 = buttons.iter().map(|(text, ..)| button(text) + 8.0).sum();
                    (Cow::Owned(word), width - 2.0 * ROW_SIDE - taken - 8.0)
                }
                None => return 0.0,
            },
        };
        // A 20 DIP line for the name, 2 apart, the word's lines, and room
        // above and below.
        22.0 + self
            .gfx
            .wrapped_height(&word, Font::new(Family::Segoe, HINT, 400.0), room.max(40.0))
            + 18.0
    }

    /// How wide each of a row's choices is.
    pub(super) fn choice_widths(&self, options: &[String]) -> Vec<f32> {
        let font = Font::new(Family::Segoe, 13.0, 400.0);
        options
            .iter()
            .map(|o| (self.gfx.measure(o, font) + 24.0).max(44.0))
            .collect()
    }

    /// How wide a row of choices is, on one line.
    pub(super) fn segmented_width(&self, options: &[String]) -> f32 {
        self.choice_widths(options).iter().sum::<f32>() + 4.0
    }

    /// A row's choices, line by line: as many whole to a line as fit in `room` (on their own line in the card, all
    /// of its width: more than that, as the players music is followed from may be, ran past its left side).
    pub(super) fn choice_lines(&self, options: &[String], room: f32) -> Vec<std::ops::Range<usize>> {
        lines_of(&self.choice_widths(options), room)
    }

    /// The room a row's choices have on a line of their own in their card.
    pub(super) fn choices_room(&self) -> f32 {
        self.cards() - 2.0 * ROW_SIDE
    }

    /// How wide the list's cards are: wider as the window is (see `CARDS`).
    pub(super) fn cards(&self) -> f32 {
        (CARDS.0 + (self.client().0 - MIN_SIZE.0) * CARDS_GROW).clamp(CARDS.0, CARDS.1)
    }

    /// The list's column: its side's padding, its cards, and the room past them to the preview.
    pub(super) fn pane(&self) -> f32 {
        PAD_SIDE + self.cards() + LIST_GAP
    }

    /// The room for the words beside a code: what its row has left.
    pub(super) fn code_words_room(&self) -> f32 {
        self.cards() - 2.0 * ROW_SIDE - CODE - 20.0
    }

    /// Whether a row of choices with a word goes on a line of its own under
    /// its name: when beside it, it would leave the word too narrow a column
    /// to read (a few characters to a line).
    pub(super) fn stacked(&self, options: &[String]) -> bool {
        self.choices_room() - self.segmented_width(options) - 16.0 < MIN_WORDS
    }

    /// Whether a row of choices named `name` (with no word under it) has them beside its name, `room` across from
    /// where the name begins to where they end: else they go on lines of their own under it (beside it, as in a
    /// longer language or at a larger size, they lay over it).
    pub(super) fn beside_name(&self, name: &str, options: &[String], room: f32) -> bool {
        self.gfx
            .measure(name, Font::new(Family::Segoe, 14.0, 400.0))
            + 16.0
            + self.segmented_width(options)
            <= room
    }

    /// The room a row of choices has in a module's card, from where its name begins to where they end.
    pub(super) fn module_choices_room(&self) -> f32 {
        self.choices_room() + 2.0 * ROW_SIDE - ITEM_INSET - ROW_SIDE - CHEVRON_ROOM
    }

    /// How open module `id`'s card is: 0 closed, 1 open, between while it
    /// slides.
    pub(super) fn opened(&self, id: &str) -> f32 {
        let open = if self.expanded.contains(id) { 1.0 } else { 0.0 };
        self.motion
            .get(&format!("open:{id}"))
            .map_or(open, |t| t.value(Instant::now()).clamp(0.0, 1.0))
    }

    /// Opens or closes module `id`'s card, sliding unless `at_once`.
    pub(super) fn set_open(&mut self, id: &str, open: bool, at_once: bool) {
        if open {
            self.expanded.insert(id.to_string());
        } else {
            self.expanded.remove(id);
        }
        self.animate(
            format!("open:{id}"),
            if open { 1.0 } else { 0.0 },
            SLIDE,
            Instant::now(),
            at_once,
        );
        self.next_frame = Instant::now();
    }

    pub(super) fn content_height(&self) -> f32 {
        self.layout().last().map_or(0.0, |(_, y, h)| y + h) + PAD_BOTTOM
    }
}
