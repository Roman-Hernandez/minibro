//! Captures the complete gesture, including releases outside the header.

use crate::application::positioning::GestureInput;
use crate::presentation::panel_view::Message;
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
    widget::{Operation, Tree, tree},
};
use iced::{
    Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector, touch, window,
};

pub fn drag_area(
    content: impl Into<Element<'static, Message>>,
    origin: Option<Point>,
) -> Element<'static, Message> {
    Element::new(DragArea {
        content: content.into(),
        origin,
    })
}

struct DragArea<'a> {
    content: Element<'a, Message>,
    origin: Option<Point>,
}

#[derive(Default)]
struct State {
    mouse_pressed: bool,
    finger: Option<touch::Finger>,
    cursor: Option<Point>,
    origin: Option<Point>,
}

impl State {
    fn input(
        &mut self,
        event: &Event,
        cursor: mouse::Cursor,
        bounds: Rectangle,
        captured: bool,
        initial_origin: Option<Point>,
    ) -> Option<GestureInput> {
        self.origin = self.origin.or(initial_origin);
        match event {
            Event::Window(window::Event::Moved(origin)) => {
                if let (Some(previous), Some(cursor)) = (self.origin, self.cursor) {
                    self.cursor = Some(cursor + (previous - *origin));
                }
                self.origin = Some(*origin);
                None
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                self.cursor = Some(*position);
                if self.mouse_pressed {
                    let point = *position + (self.origin? - Point::ORIGIN);
                    Some(GestureInput::Moved {
                        x: point.x,
                        y: point.y,
                    })
                } else {
                    None
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if !captured && self.finger.is_none() && !self.mouse_pressed =>
            {
                let position = self.cursor.or_else(|| cursor.position())?;
                if !bounds.contains(position) {
                    return None;
                }
                let point = position + (self.origin? - Point::ORIGIN);
                self.mouse_pressed = true;
                Some(GestureInput::Pressed {
                    x: point.x,
                    y: point.y,
                })
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if self.mouse_pressed =>
            {
                self.mouse_pressed = false;
                Some(GestureInput::Released)
            }
            Event::Touch(touch::Event::FingerPressed { id, position })
                if !captured
                    && !self.mouse_pressed
                    && self.finger.is_none()
                    && bounds.contains(*position) =>
            {
                let point = *position + (self.origin? - Point::ORIGIN);
                self.finger = Some(*id);
                Some(GestureInput::Pressed {
                    x: point.x,
                    y: point.y,
                })
            }
            Event::Touch(touch::Event::FingerMoved { id, position })
                if self.finger == Some(*id) =>
            {
                let point = *position + (self.origin? - Point::ORIGIN);
                Some(GestureInput::Moved {
                    x: point.x,
                    y: point.y,
                })
            }
            Event::Touch(touch::Event::FingerLifted { id, .. }) if self.finger == Some(*id) => {
                self.finger = None;
                Some(GestureInput::Released)
            }
            Event::Touch(touch::Event::FingerLost { id, .. }) if self.finger == Some(*id) => {
                self.finger = None;
                Some(GestureInput::Cancelled)
            }
            Event::Window(window::Event::Unfocused)
                if self.mouse_pressed || self.finger.is_some() =>
            {
                self.mouse_pressed = false;
                self.finger = None;
                Some(GestureInput::Cancelled)
            }
            _ => None,
        }
    }
}

impl Widget<Message, Theme, Renderer> for DragArea<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        let input = tree.state.downcast_mut::<State>().input(
            event,
            cursor,
            layout.bounds(),
            shell.is_event_captured(),
            self.origin,
        );
        if let Some(input) = input {
            shell.publish(Message::Gesture(input));
            shell.capture_event();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if tree.state.downcast_ref::<State>().mouse_pressed {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Grab
        } else {
            self.content.as_widget().mouse_interaction(
                &tree.children[0],
                layout,
                cursor,
                viewport,
                renderer,
            )
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::positioning::{Gesture, GestureAction};
    use iced::Point;

    #[test]
    fn a_batched_fast_drag_uses_event_time_positions() {
        let mut state = State::default();
        let mut gesture = Gesture::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(168.0, 32.0));
        let final_cursor = mouse::Cursor::Available(Point::new(50.0, 10.0));
        let events = [
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(20.0, 10.0),
            }),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(50.0, 10.0),
            }),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ];
        let actions: Vec<_> = events
            .iter()
            .filter_map(|event| {
                state.input(event, final_cursor, bounds, false, Some(Point::ORIGIN))
            })
            .map(|input| gesture.update(input))
            .collect();
        assert_eq!(actions.last(), Some(&GestureAction::Dropped));
        assert!(actions.contains(&GestureAction::Drag {
            dx: 30.0,
            dy: 0.0,
            started: true
        }));
    }

    #[test]
    fn moving_window_keeps_event_coordinates_in_a_stable_frame() {
        let mut state = State::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(168.0, 32.0));
        let cursor = mouse::Cursor::Available(Point::new(30.0, 10.0));
        let origin = Some(Point::new(1000.0, 100.0));
        let input = |state: &mut State, event| state.input(&event, cursor, bounds, false, origin);
        input(
            &mut state,
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(20.0, 10.0),
            }),
        );
        assert_eq!(
            input(
                &mut state,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            ),
            Some(GestureInput::Pressed {
                x: 1020.0,
                y: 110.0
            })
        );
        assert_eq!(
            input(
                &mut state,
                Event::Window(window::Event::Moved(Point::new(1030.0, 100.0)))
            ),
            None
        );
        assert_eq!(
            input(
                &mut state,
                Event::Mouse(mouse::Event::CursorMoved {
                    position: Point::new(30.0, 10.0)
                })
            ),
            Some(GestureInput::Moved {
                x: 1060.0,
                y: 110.0
            })
        );
    }

    #[test]
    fn release_outside_header_finishes_the_original_gesture() {
        let mut state = State::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(168.0, 32.0));
        let final_cursor = mouse::Cursor::Available(Point::new(300.0, 200.0));
        let events = [
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(20.0, 10.0),
            }),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(300.0, 200.0),
            }),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ];
        let inputs: Vec<_> = events
            .iter()
            .filter_map(|event| {
                state.input(event, final_cursor, bounds, false, Some(Point::ORIGIN))
            })
            .collect();
        assert_eq!(
            inputs.first(),
            Some(&GestureInput::Pressed { x: 20.0, y: 10.0 })
        );
        assert_eq!(inputs.last(), Some(&GestureInput::Released));
        assert!(!state.mouse_pressed);
    }
}
