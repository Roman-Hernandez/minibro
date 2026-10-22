use crate::domain::{Character, Mood};
use crate::presentation::theme::{
    COLOR_ACCENT_ERROR, COLOR_ACCENT_IDLE, COLOR_ACCENT_SUCCESS, COLOR_ACCENT_THINKING,
    COLOR_ACCENT_WAITING, COLOR_ACCENT_WORKING, COLOR_FACE,
};
use iced::mouse::Cursor;
use iced::widget::canvas::{self, Frame, Geometry, Path, Program, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Theme};

#[derive(Debug, Clone)]
pub struct FaceProgram {
    pub character: Character,
}

impl FaceProgram {
    #[must_use]
    pub fn new(character: Character) -> Self {
        Self { character }
    }

    fn mood_color(&self) -> Color {
        match self.character.mood {
            Mood::Idle => COLOR_ACCENT_IDLE,
            Mood::Thinking => COLOR_ACCENT_THINKING,
            Mood::Working => COLOR_ACCENT_WORKING,
            Mood::Success => COLOR_ACCENT_SUCCESS,
            Mood::Waiting => COLOR_ACCENT_WAITING,
            Mood::Error => COLOR_ACCENT_ERROR,
        }
    }
}

impl<Message> Program<Message, Theme, Renderer> for FaceProgram {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);

        let eye_spacing = 14.0;
        let left_eye_x = center.x - eye_spacing;
        let right_eye_x = center.x + eye_spacing;
        let eye_y = center.y - 2.0;
        let openness = self.character.eye_openness;

        let stroke_color = self.mood_color();
        let stroke_style = Stroke::default().with_color(stroke_color).with_width(2.0);

        // Render eyes based on mood
        match self.character.mood {
            Mood::Success => {
                // Happy upward angles: ^ ^
                let draw_happy_eye = |frame: &mut Frame, x: f32| {
                    let path = Path::new(|builder| {
                        builder.move_to(Point::new(x - 5.0, eye_y + 2.0));
                        builder.line_to(Point::new(x, eye_y - 4.0));
                        builder.line_to(Point::new(x + 5.0, eye_y + 2.0));
                    });
                    frame.stroke(&path, stroke_style);
                };
                draw_happy_eye(&mut frame, left_eye_x);
                draw_happy_eye(&mut frame, right_eye_x);
            }
            Mood::Error => {
                // Cross eyes: x x
                let draw_cross_eye = |frame: &mut Frame, x: f32| {
                    let path = Path::new(|builder| {
                        builder.move_to(Point::new(x - 4.0, eye_y - 4.0));
                        builder.line_to(Point::new(x + 4.0, eye_y + 4.0));
                        builder.move_to(Point::new(x + 4.0, eye_y - 4.0));
                        builder.line_to(Point::new(x - 4.0, eye_y + 4.0));
                    });
                    frame.stroke(&path, stroke_style);
                };
                draw_cross_eye(&mut frame, left_eye_x);
                draw_cross_eye(&mut frame, right_eye_x);
            }
            Mood::Thinking => {
                // Oscillating gaze
                let tick_f = self.character.anim_tick as f32;
                let offset_x = (tick_f * 0.12).sin() * 3.0;

                let draw_thinking_eye = |frame: &mut Frame, x: f32| {
                    let radius = 3.5 * openness.max(0.15);
                    let eye_path = Path::circle(Point::new(x + offset_x, eye_y), radius);
                    frame.fill(&eye_path, stroke_color);
                };
                draw_thinking_eye(&mut frame, left_eye_x);
                draw_thinking_eye(&mut frame, right_eye_x);
            }
            Mood::Working => {
                // Concentrated horizontal slit or fast scan
                let tick_f = self.character.anim_tick as f32;
                let pulse = (tick_f * 0.2).sin().abs() * 1.5;
                let width = 8.0 + pulse;
                let height = (4.0 * openness).max(1.0);

                let draw_work_eye = |frame: &mut Frame, x: f32| {
                    let rect = Rectangle {
                        x: x - (width / 2.0),
                        y: eye_y - (height / 2.0),
                        width,
                        height,
                    };
                    frame.fill_rectangle(rect.position(), rect.size(), stroke_color);
                };
                draw_work_eye(&mut frame, left_eye_x);
                draw_work_eye(&mut frame, right_eye_x);
            }
            Mood::Idle | Mood::Waiting => {
                // Geometric round eyes that blink smoothly
                let base_radius = if self.character.mood == Mood::Waiting {
                    4.2
                } else {
                    3.8
                };

                let draw_blinking_eye = |frame: &mut Frame, x: f32| {
                    if openness < 0.15 {
                        // Eye is closed line
                        let path = Path::new(|builder| {
                            builder.move_to(Point::new(x - 4.0, eye_y));
                            builder.line_to(Point::new(x + 4.0, eye_y));
                        });
                        frame.stroke(&path, stroke_style);
                    } else {
                        // Oval or circle
                        let path = Path::new(|builder| {
                            let r_x = base_radius;
                            let r_y = base_radius * openness;
                            builder.ellipse(canvas::path::arc::Elliptical {
                                center: Point::new(x, eye_y),
                                radii: iced::Vector::new(r_x, r_y),
                                rotation: iced::Radians(0.0),
                                start_angle: iced::Radians(0.0),
                                end_angle: iced::Radians(std::f32::consts::TAU),
                            });
                        });
                        frame.fill(&path, COLOR_FACE);
                    }
                };

                draw_blinking_eye(&mut frame, left_eye_x);
                draw_blinking_eye(&mut frame, right_eye_x);
            }
        }

        // Render mouth
        let mouth_y = center.y + 6.0;
        match self.character.mood {
            Mood::Success => {
                // Smile arc
                let mouth = Path::new(|builder| {
                    builder.move_to(Point::new(center.x - 5.0, mouth_y - 1.0));
                    builder.quadratic_curve_to(
                        Point::new(center.x, mouth_y + 4.0),
                        Point::new(center.x + 5.0, mouth_y - 1.0),
                    );
                });
                frame.stroke(&mouth, stroke_style);
            }
            Mood::Thinking => {
                // Small dot mouth
                let mouth = Path::circle(Point::new(center.x, mouth_y + 1.0), 1.8);
                frame.fill(&mouth, stroke_color);
            }
            Mood::Working => {
                // Focused flat dash
                let mouth = Path::new(|builder| {
                    builder.move_to(Point::new(center.x - 3.5, mouth_y));
                    builder.line_to(Point::new(center.x + 3.5, mouth_y));
                });
                frame.stroke(&mouth, stroke_style);
            }
            Mood::Error => {
                // Slight wavy / inverted dash
                let mouth = Path::new(|builder| {
                    builder.move_to(Point::new(center.x - 4.0, mouth_y + 2.0));
                    builder.line_to(Point::new(center.x, mouth_y));
                    builder.line_to(Point::new(center.x + 4.0, mouth_y + 2.0));
                });
                frame.stroke(&mouth, stroke_style);
            }
            Mood::Idle | Mood::Waiting => {
                // Subtle small dot or tiny line
                let mouth = Path::circle(Point::new(center.x, mouth_y + 1.0), 1.4);
                frame.fill(&mouth, COLOR_FACE);
            }
        }

        vec![frame.into_geometry()]
    }
}
