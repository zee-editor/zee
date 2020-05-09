use num_complex::Complex;
use rayon::prelude::*;
use zi::{
    self,
    frontend::Termion,
    layout,
    terminal::{canvas::Textel, SquarePixelGrid},
    App, BindingMatch, BindingTransition, Canvas, Colour, Component, ComponentLink, Key, Layout,
    Rect, Result, Scheduler, ShouldRender, Size, Style,
};

type Position = euclid::default::Point2D<f64>;

#[derive(Clone, Debug)]
struct Theme {
    logo: Style,
    tagline: Style,
    credits: Style,
}

const DARK0_SOFT: Colour = Colour::rgb(50, 48, 47);
const LIGHT2: Colour = Colour::rgb(213, 196, 161);
const GRAY_245: Colour = Colour::rgb(146, 131, 116);
const BRIGHT_BLUE: Colour = Colour::rgb(131, 165, 152);

impl Default for Theme {
    fn default() -> Self {
        Self {
            logo: Style::normal(DARK0_SOFT, LIGHT2),
            tagline: Style::normal(DARK0_SOFT, BRIGHT_BLUE),
            credits: Style::normal(DARK0_SOFT, GRAY_245),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Properties {
    position: Position,
    scale: f64,
}

#[derive(Debug)]
struct Mandelbrot {
    properties: Properties,
    grid: Vec<Complex<f64>>,
}

impl Component for Mandelbrot {
    type Message = ();
    type Properties = Properties;

    fn create(
        properties: Self::Properties,
        _link: ComponentLink<Self>,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> Self {
        Self {
            properties,
            grid: Vec::new(),
        }
    }

    fn change(
        &mut self,
        properties: Self::Properties,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender {
        self.properties = properties;
        ShouldRender::Yes
    }

    #[inline]
    fn view(&self, frame: Rect) -> Layout {
        let Self {
            properties: Properties { position, scale },
            ..
        } = *self;
        let theme = Theme::default();
        let mut grid = SquarePixelGrid::from_available(frame.size);

        let width = grid.size().width as f64;
        let height = grid.size().height as f64;

        for (x, y, colour) in (0..grid.size().width)
            .into_par_iter()
            .flat_map(|x| {
                (0..grid.size().height).into_par_iter().map(move |y| {
                    let xf = ((x as f64 / width) - 0.5) * scale + position.x;
                    let yf = ((y as f64 / height) - 0.5) * scale + position.y;

                    let c = Complex::new(xf, yf);
                    let mut z = Complex::new(0.0, 0.0);

                    let target = 4.0;
                    let mut num_steps = 0;
                    for _ in 0..1000 {
                        num_steps += 1;
                        z = z * z + c;
                        if z.norm_sqr() > target {
                            break;
                        }
                    }
                    let conv = (num_steps as f64 / 1000.0).max(0.0).min(1.0);
                    // let conv2 = 1.0 - (z.norm_sqr() / target).max(0.0).min(1.0);
                    // let conv = conv1 * conv2;
                    // let xx = (conv * 255.0).floor() as u8;
                    let g = colorous::CUBEHELIX.eval_continuous(1.0 - conv);
                    (x, y, Colour::rgb(g.r, g.g, g.b))
                })
            })
            .collect::<Vec<_>>()
        {
            grid.draw(zi::Position::new(x, y), colour);
        }

        grid.into_canvas().into()
    }
}

enum Message {
    MoveUp,
    MoveRight,
    MoveDown,
    MoveLeft,
    ZoomIn,
    ZoomOut,
}

#[derive(Debug)]
struct Viewer {
    theme: Theme,
    position: Position,
    scale: f64,
}

impl Default for Viewer {
    fn default() -> Self {
        Self {
            theme: Default::default(),
            position: Position::new(-1.0, -1.0),
            scale: 2.0,
        }
    }
}

impl Component for Viewer {
    type Message = Message;
    type Properties = ();

    fn create(
        _properties: Self::Properties,
        _link: ComponentLink<Self>,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> Self {
        Default::default()
    }

    fn update(
        &mut self,
        message: Self::Message,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender {
        let step = self.scale / 20.0;
        match message {
            Message::MoveUp => self.position.y -= step,
            Message::MoveDown => self.position.y += step,
            Message::MoveLeft => self.position.x -= step,
            Message::MoveRight => self.position.x += step,
            Message::ZoomIn => self.scale /= 1.1,
            Message::ZoomOut => self.scale *= 1.1,
            _ => {}
        }
        ShouldRender::Yes
    }

    fn change(
        &mut self,
        _properties: Self::Properties,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender {
        ShouldRender::Yes
    }

    fn view(&self, _frame: Rect) -> Layout {
        layout::component::<Mandelbrot>(
            1,
            Properties {
                position: self.position,
                scale: self.scale,
            },
        )
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        let mut transition = BindingTransition::Clear;
        let message = match pressed {
            &[Key::Char('w')] => Some(Message::MoveUp),
            &[Key::Char('d')] => Some(Message::MoveRight),
            &[Key::Char('s')] => Some(Message::MoveDown),
            &[Key::Char('a')] => Some(Message::MoveLeft),
            &[Key::Char('=')] => Some(Message::ZoomIn),
            &[Key::Char('-')] => Some(Message::ZoomOut),
            &[Key::Ctrl('x'), Key::Ctrl('c')] => {
                transition = BindingTransition::Exit;
                None
            }
            &[Key::Ctrl('x')] => {
                transition = BindingTransition::Continue;
                None
            }
            _ => None,
        };
        BindingMatch {
            transition,
            message,
        }
    }
}

fn main() -> Result<()> {
    let mut app = App::new_with_component(Viewer::default())?;
    app.run_event_loop(Termion::new()?)
}
