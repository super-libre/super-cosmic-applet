// SPDX-License-Identifier: GPL-3.0-only
use cosmic::{
    Element, Renderer, Theme,
    iced::{
        core::{Rectangle, mouse},
        widget::{
            Canvas,
            canvas::{Frame, Geometry, Program},
        },
    },
};

use crate::app::Message;
use crate::models::theme::{VisualizationColorConfig, VisualizationSide, WorkingAnimationTheme};
use crate::ui::components::working_animations::{self, WorkingDrawContext};

/// Time-driven "working" animation canvas, shown during the transcribing
/// (`Processing`) phase. Analogous to `VisualizationComponent` but driven by
/// elapsed time instead of audio data.
#[derive(Debug, Clone)]
pub struct WorkingAnimationComponent {
    theme: WorkingAnimationTheme,
    colors: VisualizationColorConfig,
    /// Which side this applet renders (fixed per binary variant). Wave-style
    /// animations split on it so the side applets seam at the middle; compact
    /// indicators may ignore it and render in full on every side.
    side: VisualizationSide,
    elapsed_ms: f32,
}

/// What the app's tests read back, to check what the panel would draw.
#[cfg(test)]
impl WorkingAnimationComponent {
    pub(crate) fn theme(&self) -> WorkingAnimationTheme {
        self.theme
    }

    pub(crate) fn colors(&self) -> &VisualizationColorConfig {
        &self.colors
    }

    pub(crate) fn elapsed_ms(&self) -> f32 {
        self.elapsed_ms
    }
}

impl WorkingAnimationComponent {
    pub fn new(
        theme: WorkingAnimationTheme,
        side: VisualizationSide,
        colors: VisualizationColorConfig,
    ) -> Self {
        Self {
            theme,
            colors,
            side,
            elapsed_ms: 0.0,
        }
    }

    pub fn set_elapsed(&mut self, elapsed_ms: f32) {
        self.elapsed_ms = elapsed_ms;
    }

    pub fn reset(&mut self) {
        self.elapsed_ms = 0.0;
    }

    /// Switch the working animation theme.
    pub fn update_theme(&mut self, theme: WorkingAnimationTheme) {
        self.theme = theme;
    }

    pub fn update_colors(&mut self, colors: VisualizationColorConfig) {
        self.colors = colors;
    }
}

impl<'a> From<WorkingAnimationComponent> for Element<'a, Message> {
    fn from(component: WorkingAnimationComponent) -> Element<'a, Message> {
        Canvas::new(component)
            .width(cosmic::iced::Length::Fill)
            .height(cosmic::iced::Length::Fill)
            .into()
    }
}

impl Program<Message, Theme, Renderer> for WorkingAnimationComponent {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(
            cosmic::iced::Point::ORIGIN,
            bounds.size(),
            cosmic::iced::Color::TRANSPARENT,
        );
        let ctx = WorkingDrawContext {
            bounds,
            elapsed_ms: self.elapsed_ms,
            color_config: &self.colors,
            is_dark: theme.cosmic().is_dark,
            cosmic_theme: theme.cosmic(),
            side: &self.side,
        };
        working_animations::draw(self.theme, &mut frame, &ctx);
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod working_animation_component_tests {
    use super::*;

    fn component() -> WorkingAnimationComponent {
        WorkingAnimationComponent::new(
            WorkingAnimationTheme::Droplet,
            VisualizationSide::Full,
            VisualizationColorConfig::default(),
        )
    }

    #[test]
    fn a_new_animation_starts_at_its_first_frame() {
        assert_eq!(component().elapsed_ms, 0.0);
    }

    #[test]
    fn resetting_rewinds_to_the_first_frame() {
        // The applet resets the component whenever the transcribing clock
        // starts, so a second recording opens on frame zero instead of
        // wherever the previous one stopped.
        let mut component = component();
        component.set_elapsed(4_200.0);

        component.reset();

        assert_eq!(component.elapsed_ms, 0.0);
    }

    #[test]
    fn switching_theme_keeps_the_clock_running() {
        // Picking a different animation mid-cycle must not restart it: the
        // clock lives in the applet, and the component only follows it.
        let mut component = component();
        component.set_elapsed(1_500.0);

        component.update_theme(WorkingAnimationTheme::Comet);

        assert_eq!(component.elapsed_ms, 1_500.0);
    }
}

#[cfg(test)]
mod drawing_tests {
    //! The component as its canvas draws it: the animation it was given, at
    //! the moment it was set to, in the theme's color, wherever the canvas
    //! sits in the applet's window.
    use super::*;
    use crate::models::theme::VisualizationColor;
    use crate::ui::components::visualizations::test_support::{
        HEIGHT, Pixels, WIDTH, colors, render, render_program, rgb, same_color, theme,
    };
    use crate::util::u32_to_f32;
    use cosmic::iced::Size;

    fn component(
        animation: WorkingAnimationTheme,
        colors: VisualizationColorConfig,
    ) -> WorkingAnimationComponent {
        let mut component =
            WorkingAnimationComponent::new(animation, VisualizationSide::Full, colors);
        component.set_elapsed(400.0);
        component
    }

    /// Draw `component` as its canvas would, with the canvas at `(x, y)` in
    /// the window.
    fn draw_at(component: &WorkingAnimationComponent, x: f32, y: f32, dark: bool) -> Pixels {
        let theme = theme(dark);
        let bounds = Rectangle {
            x,
            y,
            width: u32_to_f32(WIDTH),
            height: u32_to_f32(HEIGHT),
        };
        render_program(WIDTH, HEIGHT, |renderer| {
            component.draw(&(), renderer, &theme, bounds, mouse::Cursor::Unavailable)
        })
    }

    #[test]
    fn it_draws_the_animation_it_was_given_at_its_moment() {
        let theme = theme(true);
        let colors = colors(VisualizationColor::White);
        let strip = Rectangle::with_size(Size::new(u32_to_f32(WIDTH), u32_to_f32(HEIGHT)));
        for animation in [
            WorkingAnimationTheme::Droplet,
            WorkingAnimationTheme::Comet,
            WorkingAnimationTheme::Dots,
        ] {
            let direct = render(WIDTH, HEIGHT, |frame| {
                working_animations::draw(
                    animation,
                    frame,
                    &WorkingDrawContext {
                        bounds: strip,
                        elapsed_ms: 400.0,
                        color_config: &colors,
                        is_dark: true,
                        cosmic_theme: theme.cosmic(),
                        side: &VisualizationSide::Full,
                    },
                );
            });
            assert_eq!(
                draw_at(&component(animation, colors.clone()), 0.0, 0.0, true).difference(&direct),
                0,
                "{animation:?} drew something else"
            );
        }
    }

    #[test]
    fn it_paints_the_color_set_for_the_themes_darkness() {
        let mut component = component(
            WorkingAnimationTheme::Dots,
            VisualizationColorConfig {
                light_colors: VisualizationColor::Red,
                dark_colors: VisualizationColor::Blue,
            },
        );
        let dark = draw_at(&component, 0.0, 0.0, true).paint_color();
        let light = draw_at(&component, 0.0, 0.0, false).paint_color();
        assert!(
            same_color(dark, rgb(VisualizationColor::Blue.to_color())),
            "{dark:?}"
        );
        assert!(
            same_color(light, rgb(VisualizationColor::Red.to_color())),
            "{light:?}"
        );

        component.update_colors(colors(VisualizationColor::Green));
        let recolored = draw_at(&component, 0.0, 0.0, true).paint_color();
        assert!(
            same_color(recolored, rgb(VisualizationColor::Green.to_color())),
            "{recolored:?}"
        );
    }

    #[test]
    fn it_becomes_a_canvas_element() {
        let element: Element<'_, Message> = component(
            WorkingAnimationTheme::Dots,
            colors(VisualizationColor::White),
        )
        .into();
        drop(element);
    }
}
