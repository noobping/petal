use adw::gtk::{self, prelude::*};
use std::{cell::Cell, rc::Rc};

const PROGRESS_HEIGHT: i32 = 3;
const LINE_WIDTH: f64 = 2.0;

#[derive(Clone)]
pub(super) struct TrackProgress {
    area: gtk::DrawingArea,
    fraction: Rc<Cell<Option<f64>>>,
}

impl TrackProgress {
    pub(super) fn set_fraction(&self, fraction: Option<f64>) {
        let fraction = fraction.map(|value| value.clamp(0.0, 1.0));
        if self.fraction.get() == fraction {
            return;
        }

        self.fraction.set(fraction);
        self.area.queue_draw();
    }
}

pub(super) fn make_track_progress() -> (gtk::DrawingArea, TrackProgress) {
    let fraction = Rc::new(Cell::new(None));
    let fraction_for_draw = fraction.clone();

    let area = gtk::DrawingArea::new();
    area.set_can_target(false);
    area.set_hexpand(true);
    area.set_halign(gtk::Align::Fill);
    area.set_valign(gtk::Align::End);
    area.set_content_height(PROGRESS_HEIGHT);
    area.set_height_request(PROGRESS_HEIGHT);
    area.add_css_class("track-progress");

    area.set_draw_func(move |area, cr, width, height| {
        let Some(fraction) = fraction_for_draw.get() else {
            return;
        };

        let width = f64::from(width);
        let height = f64::from(height);
        if width <= LINE_WIDTH || height <= 0.0 {
            return;
        }

        let (r, g, b) = widget_css_color(&area.clone().upcast::<gtk::Widget>());
        let inset = LINE_WIDTH / 2.0;
        let start_x = inset;
        let end_x = (width - inset).max(start_x);
        let y = height - inset;

        cr.set_line_width(LINE_WIDTH);
        cr.set_line_cap(gtk::cairo::LineCap::Round);

        cr.set_source_rgba(r, g, b, 0.20);
        cr.move_to(start_x, y);
        cr.line_to(end_x, y);
        let _ = cr.stroke();

        if fraction > 0.0 {
            cr.set_source_rgba(r, g, b, 0.92);
            cr.move_to(start_x, y);
            cr.line_to(start_x + (end_x - start_x) * fraction, y);
            let _ = cr.stroke();
        }
    });

    let progress = TrackProgress {
        area: area.clone(),
        fraction,
    };
    (area, progress)
}

fn widget_css_color(widget: &gtk::Widget) -> (f64, f64, f64) {
    let color = widget.style_context().color();
    (
        color.red() as f64,
        color.green() as f64,
        color.blue() as f64,
    )
}
