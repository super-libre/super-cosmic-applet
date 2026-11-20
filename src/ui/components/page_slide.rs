// SPDX-License-Identifier: GPL-3.0-only
//! Two pages of the popup side by side, sliding one out as the other comes
//! in.
//!
//! The slide is as tall as the taller page throughout. COSMIC draws a popup
//! out of place for a frame whenever its size changes, so a popup that
//! followed the pages' heights frame by frame jitters as it slides. Held at
//! the taller one's height, it changes size once: as the slide starts, into a
//! taller page, or as it ends, into a shorter one.
use cosmic::{
    Element, Renderer, Theme,
    iced::core::{
        Layout, Length, Point, Rectangle, Renderer as _, Size, layout, mouse, renderer,
        widget::{Operation, Tree, Widget},
    },
};

/// Which way the pages go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Into a page further in: the pages move right to left.
    Forward,
    /// Back out of one: left to right.
    Back,
}

pub struct PageSlide<'a, Message> {
    /// The page leaving, then the one entering.
    pages: [Element<'a, Message>; 2],
    /// How far the entering page has come, from 0 to 1.
    progress: f32,
    direction: Direction,
}

impl<'a, Message> PageSlide<'a, Message> {
    pub fn new(
        leaving: impl Into<Element<'a, Message>>,
        entering: impl Into<Element<'a, Message>>,
        progress: f32,
        direction: Direction,
    ) -> Self {
        Self {
            pages: [leaving.into(), entering.into()],
            progress: progress.clamp(0.0, 1.0),
            direction,
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for PageSlide<'_, Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn children(&self) -> Vec<Tree> {
        self.pages.iter().map(Tree::new).collect()
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut self.pages);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        // Each page as wide as the popup, laid out as if it were alone.
        let width = limits.max().width;
        let page_limits =
            layout::Limits::new(Size::new(width, 0.0), Size::new(width, limits.max().height));
        let [leaving, entering] = [0, 1].map(|i| {
            self.pages[i]
                .as_widget_mut()
                .layout(&mut tree.children[i], renderer, &page_limits)
        });

        let shift = width * self.progress;
        let (leaving_x, entering_x) = match self.direction {
            Direction::Forward => (-shift, width - shift),
            Direction::Back => (shift, shift - width),
        };
        let height = leaving.size().height.max(entering.size().height);

        layout::Node::with_children(
            limits.resolve(Length::Fill, Length::Shrink, Size::new(width, height)),
            vec![
                leaving.move_to(Point::new(leaving_x, 0.0)),
                entering.move_to(Point::new(entering_x, 0.0)),
            ],
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        // Only what is inside the slide's bounds shows: the rest of each
        // page is off to the side, or below the height they're at.
        let Some(clip) = layout.bounds().intersection(viewport) else {
            return;
        };
        renderer.with_layer(clip, |renderer| {
            for ((page, state), page_layout) in
                self.pages.iter().zip(&tree.children).zip(layout.children())
            {
                // No pointer: nothing lights up under it as the pages pass.
                page.as_widget().draw(
                    state,
                    renderer,
                    theme,
                    style,
                    page_layout,
                    mouse::Cursor::Unavailable,
                    &clip,
                );
            }
        });
    }

    // No `update`: the pages take no clicks while they move. The slide is
    // over in a moment, and a click on a page mid-way could land on
    // something other than what was under the pointer when it was aimed.

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            for ((page, state), page_layout) in self
                .pages
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
            {
                page.as_widget_mut()
                    .operate(state, page_layout, renderer, operation);
            }
        });
    }
}

impl<'a, Message: 'a> From<PageSlide<'a, Message>> for Element<'a, Message> {
    fn from(slide: PageSlide<'a, Message>) -> Self {
        Element::new(slide)
    }
}

#[cfg(test)]
mod tests {
    //! A slide between a red page 100px tall and a green one 200px tall, on a
    //! 120px-wide popup.
    use super::{Direction, PageSlide};
    use crate::app::Message;
    use crate::ui::simulator::Simulator;
    use cosmic::{
        Element,
        iced::{Background, Color, Length, Point, Size},
        widget::{Space, container, mouse_area},
    };

    const WIDTH: f32 = 120.0;
    const RED: [u8; 3] = [255, 0, 0];
    const GREEN: [u8; 3] = [0, 255, 0];

    fn page(color: Color, height: f32) -> Element<'static, Message> {
        mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fixed(height))
                .style(move |_| container::Style {
                    background: Some(Background::Color(color)),
                    ..Default::default()
                }),
        )
        .on_press(Message::OpenGitHub)
        .into()
    }

    fn slide(progress: f32, direction: Direction) -> Simulator<'static> {
        slide_between([100.0, 200.0], progress, direction)
    }

    /// A slide from a red page to a green one, of the `heights` given.
    fn slide_between(heights: [f32; 2], progress: f32, direction: Direction) -> Simulator<'static> {
        Simulator::with_size(
            PageSlide::new(
                page(Color::from_rgb(1.0, 0.0, 0.0), heights[0]),
                page(Color::from_rgb(0.0, 1.0, 0.0), heights[1]),
                progress,
                direction,
            ),
            Size::new(WIDTH, 400.0),
        )
    }

    /// The color drawn at `(x, y)`, or `None` where nothing is.
    fn color_at(pixels: &[u8], x: f32, y: f32) -> Option<[u8; 3]> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let i = (y as usize * WIDTH as usize + x as usize) * 4;
        (pixels[i + 3] > 0).then(|| [pixels[i], pixels[i + 1], pixels[i + 2]])
    }

    #[test]
    fn it_starts_on_the_page_leaving_and_ends_on_the_one_entering() {
        for direction in [Direction::Forward, Direction::Back] {
            let start = slide(0.0, direction).screenshot();
            let end = slide(1.0, direction).screenshot();
            for x in [1.0, WIDTH / 2.0, WIDTH - 1.0] {
                assert_eq!(color_at(&start, x, 50.0), Some(RED), "{direction:?}");
                assert_eq!(color_at(&end, x, 50.0), Some(GREEN), "{direction:?}");
            }
        }
    }

    /// Halfway into a page further in, the page leaving has gone half off
    /// to the left, and the one entering half in from the right. Back out,
    /// the other way around.
    #[test]
    fn forward_goes_right_to_left_and_back_left_to_right() {
        for (direction, left, right) in [
            (Direction::Forward, RED, GREEN),
            (Direction::Back, GREEN, RED),
        ] {
            let pixels = slide(0.5, direction).screenshot();
            assert_eq!(color_at(&pixels, WIDTH * 0.25, 50.0), Some(left));
            assert_eq!(color_at(&pixels, WIDTH * 0.75, 50.0), Some(right));
        }
    }

    /// From start to end, into a taller page or a shorter one, it is as tall
    /// as the taller page: the popup's size changes once, not frame by
    /// frame.
    #[test]
    fn it_is_as_tall_as_the_taller_page_throughout() {
        for heights in [[100.0, 200.0], [200.0, 100.0]] {
            for progress in [0.0, 0.25, 0.5, 1.0] {
                let mut ui = slide_between(heights, progress, Direction::Forward);
                let height = ui.containers()[0].height;
                assert!(
                    (height - 200.0).abs() < 0.5,
                    "{heights:?} at {progress}: {height}"
                );
            }
        }
    }

    /// Below the shorter page, nothing: the taller one beside it shows on.
    #[test]
    fn the_shorter_page_leaves_room_below_it() {
        let pixels = slide(0.5, Direction::Forward).screenshot();
        assert_eq!(color_at(&pixels, WIDTH * 0.25, 150.0), None);
        assert_eq!(color_at(&pixels, WIDTH * 0.75, 150.0), Some(GREEN));
    }

    #[test]
    fn the_pages_take_no_clicks_while_they_move() {
        let mut ui = slide(0.5, Direction::Forward);
        ui.click_at(Point::new(WIDTH * 0.25, 50.0))
            .click_at(Point::new(WIDTH * 0.75, 50.0));
        assert!(ui.into_messages().is_empty());
    }
}
