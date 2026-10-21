// SPDX-License-Identifier: GPL-3.0-only
//! A headless stand-in for iced's test simulator, for the popup's and the
//! panel's tests.
//!
//! `iced_test` doesn't build against the iced in libcosmic's fork, which has
//! moved on from it, so this does the part the tests need: lay an element out
//! on the tiny-skia renderer, list what it shows, click it, collecting the
//! messages its widgets emit, and draw it.
use cosmic::iced::core::{
    Color, Event, Font, Pixels, Point, Rectangle, Size, clipboard, mouse,
    renderer::{self, Headless},
    widget::{Id, Operation},
};
use cosmic::iced::runtime::user_interface::{Cache, UserInterface};
use cosmic::{Element, Renderer, Theme};

use crate::app::Message;
use crate::app::testing::TestApplet;

/// The popup's width, and a height anything it shows fits in.
const POPUP: Size = Size::new(360.0, 4000.0);

pub(crate) struct Simulator<'a> {
    ui: UserInterface<'a, Message, Theme, Renderer>,
    renderer: Renderer,
    size: Size,
    messages: Vec<Message>,
}

impl<'a> Simulator<'a> {
    /// `element`, laid out as the popup would be.
    pub(crate) fn new(element: impl Into<Element<'a, Message>>) -> Self {
        Self::with_size(element, POPUP)
    }

    pub(crate) fn with_size(element: impl Into<Element<'a, Message>>, size: Size) -> Self {
        let mut renderer = cosmic::iced::futures::executor::block_on(<Renderer as Headless>::new(
            Font::default(),
            Pixels(14.0),
            None,
        ))
        .expect("a headless renderer");
        let ui = UserInterface::build(element, size, Cache::default(), &mut renderer);
        Self {
            ui,
            renderer,
            size,
            messages: Vec::new(),
        }
    }

    /// Every text the element shows, in layout order.
    pub(crate) fn texts(&mut self) -> Vec<String> {
        self.inspect().texts.into_iter().map(|(t, _)| t).collect()
    }

    pub(crate) fn shows(&mut self, text: &str) -> bool {
        self.texts().iter().any(|t| t == text)
    }

    /// The bounds of every container: each button, row, column and padded
    /// control, in layout order. A button without text is found here.
    pub(crate) fn containers(&mut self) -> Vec<Rectangle> {
        self.inspect().containers
    }

    /// Where `text` is, panicking if the element doesn't show it.
    pub(crate) fn bounds_of(&mut self, text: &str) -> Rectangle {
        let inspection = self.inspect();
        let Some((_, bounds)) = inspection.texts.iter().find(|(t, _)| t == text) else {
            let shown: Vec<&String> = inspection.texts.iter().map(|(t, _)| t).collect();
            panic!("{text:?} isn't shown; the element shows {shown:?}");
        };
        *bounds
    }

    /// Click the middle of `text`, panicking if the element doesn't show it.
    pub(crate) fn click(&mut self, text: &str) -> &mut Self {
        let center = self.bounds_of(text).center();
        self.click_at(center)
    }

    /// Click the far end of the row `text` labels, where a setting's toggler
    /// sits: the toggler has no text of its own to find.
    pub(crate) fn click_toggler_of(&mut self, text: &str) -> &mut Self {
        let label = self.bounds_of(text);
        let row = self
            .containers()
            .into_iter()
            .filter(|c| c.contains(label.center()) && c.width > label.width)
            .min_by(|a, b| (a.width * a.height).total_cmp(&(b.width * b.height)))
            .unwrap_or_else(|| panic!("{text:?} labels no row"));
        self.click_at(Point::new(row.x + row.width - 12.0, row.center_y()))
    }

    /// Where the control `text` heads is: the rest of the column the label
    /// tops. A segmented control or a slider reports no text or container of
    /// its own to find it by.
    pub(crate) fn control_below(&mut self, text: &str) -> Rectangle {
        let label = self.bounds_of(text);
        let label_bottom = label.y + label.height;
        let column = self
            .containers()
            .into_iter()
            .filter(|c| c.contains(label.center()) && c.y + c.height > label_bottom + 1.0)
            .min_by(|a, b| (a.width * a.height).total_cmp(&(b.width * b.height)))
            .unwrap_or_else(|| panic!("{text:?} heads nothing"));
        Rectangle::new(
            Point::new(column.x, label_bottom),
            Size::new(column.width, column.y + column.height - label_bottom),
        )
    }

    /// Press and release the left button at `point`, as a click does.
    pub(crate) fn click_at(&mut self, point: Point) -> &mut Self {
        let events = [
            Event::Mouse(mouse::Event::CursorMoved { position: point }),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ];
        let _ = self.ui.update(
            &events,
            mouse::Cursor::Available(point),
            &mut self.renderer,
            &mut clipboard::Null,
            &mut self.messages,
        );
        self
    }

    /// The messages the clicks so far made the element emit, in order.
    pub(crate) fn into_messages(self) -> Vec<Message> {
        self.messages
    }

    /// The element drawn on a transparent background, as RGBA bytes.
    pub(crate) fn screenshot(&mut self) -> Vec<u8> {
        let theme = cosmic::theme::active();
        self.ui.draw(
            &mut self.renderer,
            &theme,
            &renderer::Style::default(),
            mouse::Cursor::Unavailable,
        );
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let size = Size::new(self.size.width as u32, self.size.height as u32);
        self.renderer.screenshot(size, 1.0, Color::TRANSPARENT)
    }

    fn inspect(&mut self) -> Inspection {
        let mut inspection = Inspection::default();
        self.ui.operate(&self.renderer, &mut inspection);
        inspection
    }
}

/// What an element shows: its text and its containers, with their bounds.
#[derive(Default)]
struct Inspection {
    texts: Vec<(String, Rectangle)>,
    containers: Vec<Rectangle>,
}

impl Operation for Inspection {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn container(&mut self, _id: Option<&Id>, bounds: Rectangle) {
        self.containers.push(bounds);
    }

    fn text(&mut self, _id: Option<&Id>, bounds: Rectangle, text: &str) {
        self.texts.push((text.to_owned(), bounds));
    }
}

/// Every message a click anywhere in the element could emit, in layout order
/// and without repeats: one click in the middle of each container, each on a
/// fresh element from `build`, so one click's effects never reach the next.
pub(crate) fn every_click<'a>(build: impl Fn() -> Element<'a, Message>) -> Vec<String> {
    let centers: Vec<Point> = Simulator::new(build())
        .containers()
        .iter()
        .map(Rectangle::center)
        .collect();
    let mut messages: Vec<String> = Vec::new();
    for center in centers {
        let mut ui = Simulator::new(build());
        ui.click_at(center);
        for message in ui.into_messages() {
            let message = format!("{message:?}");
            if !messages.contains(&message) {
                messages.push(message);
            }
        }
    }
    messages
}

/// Where to click in the element from `build` to send `message`, as its
/// `Debug` form: the middle of the first container whose click sends it. For
/// a control with no text to find it by, like a color swatch.
pub(crate) fn where_to_send<'a>(
    build: impl Fn() -> Element<'a, Message>,
    message: &str,
) -> Option<Point> {
    let centers: Vec<Point> = Simulator::new(build())
        .containers()
        .iter()
        .map(Rectangle::center)
        .collect();
    centers.into_iter().find(|center| {
        let mut ui = Simulator::new(build());
        ui.click_at(*center);
        ui.into_messages()
            .iter()
            .any(|sent| format!("{sent:?}") == message)
    })
}

/// Do `act` to `applet`'s popup, then hand the applet what that made the
/// popup send, as iced would. Returns what was sent.
///
/// Panics rather than deliver a message whose handler starts a process: a
/// test may check that a button sends one, never run it.
pub(crate) fn in_popup(
    applet: &mut TestApplet,
    act: impl FnOnce(&mut Simulator<'_>),
) -> Vec<String> {
    let messages = {
        let mut ui = Simulator::new(applet.popup());
        act(&mut ui);
        ui.into_messages()
    };
    let sent = messages.iter().map(|m| format!("{m:?}")).collect();
    for message in messages {
        assert!(
            !matches!(message, Message::LaunchApp(_) | Message::OpenGitHub),
            "{message:?} would start a process"
        );
        applet.send(message);
    }
    sent
}

/// What `applet`'s popup shows.
pub(crate) fn popup_texts(applet: &TestApplet) -> Vec<String> {
    Simulator::new(applet.popup()).texts()
}

/// Whether any pixel of an RGBA screenshot is drawn at all.
pub(crate) fn draws_anything(rgba: &[u8]) -> bool {
    rgba.chunks_exact(4).any(|pixel| pixel[3] > 0)
}
