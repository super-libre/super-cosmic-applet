// SPDX-License-Identifier: GPL-3.0-only
//! Which page the popup shows, and the slide from one page to the next.
use std::time::Duration;

use cosmic::iced::time::Instant;

use crate::models::state::Page;

/// How long a page takes to slide in.
pub(crate) const PAGE_TURN: Duration = Duration::from_millis(250);

/// The page the popup shows, and the slide to it while that lasts.
#[derive(Debug, Default)]
pub(crate) struct Pages {
    shown: Page,
    turn: Option<Turn>,
}

/// A slide from one page to the one shown.
#[derive(Debug, Clone, Copy)]
struct Turn {
    from: Page,
    started: Instant,
    /// How much of [`PAGE_TURN`] has passed, from 0 to 1.
    elapsed: f32,
}

impl Pages {
    /// The page the popup shows, or is sliding to.
    pub(crate) fn shown(&self) -> Page {
        self.shown
    }

    /// Slide from the page shown to `page`. Already there, nothing moves.
    pub(crate) fn open(&mut self, page: Page, now: Instant) {
        if page == self.shown {
            return;
        }
        self.turn = Some(Turn {
            from: self.shown,
            started: now,
            elapsed: 0.0,
        });
        self.shown = page;
    }

    /// Time passed: the slide moves on, and ends once it has taken
    /// [`PAGE_TURN`].
    pub(crate) fn tick(&mut self, now: Instant) {
        let Some(turn) = &mut self.turn else {
            return;
        };
        let elapsed =
            now.saturating_duration_since(turn.started).as_secs_f32() / PAGE_TURN.as_secs_f32();
        if elapsed >= 1.0 {
            self.turn = None;
        } else {
            turn.elapsed = elapsed;
        }
    }

    pub(crate) fn turning(&self) -> bool {
        self.turn.is_some()
    }

    /// While a page slides in: the page it replaces, and how far it has
    /// come, from 0 to 1. It sets off fast and settles into place.
    pub(crate) fn slide(&self) -> Option<(Page, f32)> {
        self.turn
            .map(|turn| (turn.from, 1.0 - (1.0 - turn.elapsed).powi(3)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn after(start: Instant, fraction: f32) -> Instant {
        start + PAGE_TURN.mul_f32(fraction)
    }

    #[test]
    fn the_popup_opens_on_the_main_page_and_still() {
        let pages = Pages::default();
        assert_eq!(pages.shown(), Page::Main);
        assert!(!pages.turning());
        assert_eq!(pages.slide(), None);
    }

    #[test]
    fn a_page_slides_in_over_the_turn_and_then_rests() {
        let start = Instant::now();
        let mut pages = Pages::default();

        pages.open(Page::Colors, start);
        assert_eq!(pages.shown(), Page::Colors);
        assert_eq!(pages.slide(), Some((Page::Main, 0.0)));

        pages.tick(after(start, 0.5));
        let (from, halfway) = pages.slide().expect("still sliding");
        assert_eq!(from, Page::Main);
        // Eased: more than half the way at half the time.
        assert!(halfway > 0.5 && halfway < 1.0, "{halfway}");

        pages.tick(after(start, 1.0));
        assert_eq!(pages.slide(), None);
        assert_eq!(pages.shown(), Page::Colors);
    }

    /// It sets off fast and slows as it lands: each slice of time moves it
    /// less than the one before.
    #[test]
    fn the_slide_slows_as_it_lands() {
        let start = Instant::now();
        let mut pages = Pages::default();
        pages.open(Page::Colors, start);
        let progress: Vec<f32> = (0_u8..=9)
            .map(|tenth| {
                pages.tick(after(start, f32::from(tenth) / 10.0));
                pages.slide().map_or(1.0, |(_, progress)| progress)
            })
            .collect();
        let steps: Vec<f32> = progress.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(steps.windows(2).all(|w| w[1] < w[0]), "{progress:?}");
    }

    #[test]
    fn back_slides_from_the_page_left() {
        let start = Instant::now();
        let mut pages = Pages::default();
        pages.open(Page::Colors, start);
        pages.tick(after(start, 1.0));

        let back = after(start, 2.0);
        pages.open(Page::Main, back);

        assert_eq!(pages.shown(), Page::Main);
        assert_eq!(pages.slide(), Some((Page::Colors, 0.0)));
    }

    #[test]
    fn opening_the_page_shown_moves_nothing() {
        let mut pages = Pages::default();
        pages.open(Page::Main, Instant::now());
        assert!(!pages.turning());
    }
}
