use crate::locale::gettext;
use adw::gtk::{self, pango, prelude::*, Align, Justification, Orientation};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const KARAOKE_SIZE: i32 = 250;
const CROSSFADE_DURATION_MS: u32 = 190;
const BLANK_CUE: &str = "♪";

#[derive(Clone, Debug, PartialEq, Eq)]
enum KaraokeContent {
    Loading,
    Lyrics {
        previous: String,
        current: String,
        next: String,
    },
    Missing,
    Error,
}

#[derive(Clone)]
struct KaraokePanel {
    root: gtk::Box,
    previous: gtk::Label,
    current: gtk::Label,
    next: gtk::Label,
}

impl KaraokePanel {
    fn new() -> Self {
        let previous = context_label();
        previous.set_valign(Align::End);
        previous.set_vexpand(true);

        let current = gtk::Label::new(None);
        current.add_css_class("title-2");
        current.set_halign(Align::Fill);
        current.set_justify(Justification::Center);
        current.set_ellipsize(pango::EllipsizeMode::End);
        current.set_lines(3);
        current.set_max_width_chars(24);
        current.set_wrap(true);
        current.set_wrap_mode(pango::WrapMode::WordChar);
        current.set_xalign(0.5);

        let next = context_label();
        next.set_valign(Align::Start);
        next.set_vexpand(true);

        let root = gtk::Box::new(Orientation::Vertical, 12);
        root.set_margin_top(18);
        root.set_margin_bottom(12);
        root.set_margin_start(18);
        root.set_margin_end(18);
        root.append(&previous);
        root.append(&current);
        root.append(&next);

        Self {
            root,
            previous,
            current,
            next,
        }
    }

    fn render(&self, content: &KaraokeContent) {
        let (previous, current, next) = match content {
            KaraokeContent::Loading => (
                String::new(),
                gettext("Finding lyrics…"),
                gettext("Just a moment"),
            ),
            KaraokeContent::Lyrics {
                previous,
                current,
                next,
            } => (previous.clone(), display_current(current), next.clone()),
            KaraokeContent::Missing => (
                String::new(),
                gettext("No synchronized lyrics"),
                gettext("This song can still be your tiny concert."),
            ),
            KaraokeContent::Error => (
                String::new(),
                gettext("Couldn't load lyrics"),
                gettext("Try again in a moment."),
            ),
        };

        // Lyrics are untrusted remote text. Keep all rendering on the plain-text API.
        self.previous.set_text(&previous);
        self.current.set_text(&current);
        self.next.set_text(&next);
    }
}

/// Passive, cloneable view for line-synchronized lyrics.
///
/// The view owns two identical panels and alternates between them so content
/// changes crossfade without continuously animating the playback clock.
#[derive(Clone)]
pub(super) struct KaraokeView {
    root: gtk::Box,
    content_stack: gtk::Stack,
    panels: [KaraokePanel; 2],
    active_panel: Rc<Cell<usize>>,
    content: Rc<RefCell<KaraokeContent>>,
}

impl KaraokeView {
    pub(super) fn new() -> Self {
        let panels = [KaraokePanel::new(), KaraokePanel::new()];
        let content_stack = gtk::Stack::new();
        content_stack.set_hexpand(true);
        content_stack.set_vexpand(true);
        content_stack.set_hhomogeneous(true);
        content_stack.set_vhomogeneous(true);
        content_stack.set_transition_duration(CROSSFADE_DURATION_MS);
        content_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        content_stack.add_named(&panels[0].root, Some("karaoke-a"));
        content_stack.add_named(&panels[1].root, Some("karaoke-b"));

        let initial = KaraokeContent::Loading;
        panels[0].render(&initial);
        panels[1].render(&initial);
        content_stack.set_visible_child_name("karaoke-a");

        let attribution = gtk::Label::new(None);
        attribution.add_css_class("caption");
        attribution.add_css_class("dim-label");
        attribution.set_margin_bottom(10);
        attribution.set_text(&gettext("Lyrics by LRCLIB"));

        let root = gtk::Box::new(Orientation::Vertical, 0);
        root.set_size_request(KARAOKE_SIZE, KARAOKE_SIZE);
        root.append(&content_stack);
        root.append(&attribution);

        Self {
            root,
            content_stack,
            panels,
            active_panel: Rc::new(Cell::new(0)),
            content: Rc::new(RefCell::new(initial)),
        }
    }

    pub(super) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub(super) fn show_loading(&self) {
        self.set_content(KaraokeContent::Loading);
    }

    pub(super) fn show_lyrics(&self, previous: Option<&str>, current: &str, next: Option<&str>) {
        self.set_content(KaraokeContent::Lyrics {
            previous: previous.unwrap_or_default().to_string(),
            current: current.to_string(),
            next: next.unwrap_or_default().to_string(),
        });
    }

    pub(super) fn show_missing(&self) {
        self.set_content(KaraokeContent::Missing);
    }

    pub(super) fn show_error(&self) {
        self.set_content(KaraokeContent::Error);
    }

    fn set_content(&self, content: KaraokeContent) {
        if *self.content.borrow() == content {
            return;
        }

        let next_panel = 1 - self.active_panel.get();
        self.panels[next_panel].render(&content);
        self.content_stack
            .set_visible_child_name(if next_panel == 0 {
                "karaoke-a"
            } else {
                "karaoke-b"
            });
        self.active_panel.set(next_panel);
        *self.content.borrow_mut() = content;
    }
}

fn context_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.add_css_class("dim-label");
    label.set_ellipsize(pango::EllipsizeMode::End);
    label.set_halign(Align::Fill);
    label.set_justify(Justification::Center);
    label.set_lines(2);
    label.set_max_width_chars(30);
    label.set_wrap(true);
    label.set_wrap_mode(pango::WrapMode::WordChar);
    label.set_xalign(0.5);
    label
}

fn display_current(current: &str) -> String {
    if current.trim().is_empty() {
        BLANK_CUE.to_string()
    } else {
        current.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{display_current, BLANK_CUE};

    #[test]
    fn blank_current_cue_becomes_a_music_note() {
        assert_eq!(display_current(" \t"), BLANK_CUE);
    }

    #[test]
    fn current_cue_text_is_preserved() {
        assert_eq!(display_current("<b>sing</b>"), "<b>sing</b>");
    }
}
