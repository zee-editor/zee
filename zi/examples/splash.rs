use std::cmp;
use unicode_width::UnicodeWidthStr;
use zi::{
    frontend::Termion, layout, App, BindingMatch, BindingTransition, Canvas, Colour, Component,
    ComponentLink, Key, Layout, Rect, Result, Scheduler, ShouldRender, Size, Style,
};

#[derive(Clone, Debug)]
struct Theme {
    logo: Style,
    tagline: Style,
    credits: Style,
}

impl Default for Theme {
    fn default() -> Self {
        const DARK0_SOFT: Colour = Colour::rgb(50, 48, 47);
        const LIGHT2: Colour = Colour::rgb(213, 196, 161);
        const GRAY_245: Colour = Colour::rgb(146, 131, 116);
        const BRIGHT_BLUE: Colour = Colour::rgb(131, 165, 152);

        Self {
            logo: Style::normal(DARK0_SOFT, LIGHT2),
            tagline: Style::normal(DARK0_SOFT, BRIGHT_BLUE),
            credits: Style::normal(DARK0_SOFT, GRAY_245),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Properties {
    theme: Theme,
    logo: String,
    tagline: String,
    credits: String,
    offset: usize,
}

fn text_block_size(text: &str) -> Size {
    let width = text.lines().map(UnicodeWidthStr::width).max().unwrap_or(0);
    let height = text.lines().count();
    Size::new(width, height)
}

#[derive(Debug)]
struct Splash {
    properties: Properties,
    logo_size: Size,
    tagline_size: Size,
    credits_size: Size,
}

impl Splash {
    fn new(properties: Properties) -> Self {
        Self {
            logo_size: text_block_size(&properties.logo),
            tagline_size: text_block_size(&properties.tagline),
            credits_size: text_block_size(&properties.credits),
            properties,
        }
    }
}

impl Component for Splash {
    type Message = usize;
    type Properties = Properties;

    fn create(
        properties: Self::Properties,
        _link: ComponentLink<Self>,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> Self {
        Self::new(properties)
    }

    fn change(
        &mut self,
        properties: Self::Properties,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender {
        *self = Self::new(properties);
        ShouldRender::Yes
    }

    #[inline]
    fn view(&self, frame: Rect) -> Layout {
        let theme = Theme::default();
        let mut canvas = Canvas::new(frame.size);
        canvas.clear(theme.logo);

        // Draw logo
        let middle_x = (frame.size.width / 2).saturating_sub(self.logo_size.width / 2);
        let mut middle_y = cmp::min(8, frame.size.height.saturating_sub(self.logo_size.height))
            + self.properties.offset;
        for line in self.properties.logo.lines() {
            canvas.draw_str(middle_x, middle_y, theme.logo, line);
            middle_y += 1;
        }

        // Draw tagline
        middle_y += 2;
        let middle_x = (frame.size.width / 2).saturating_sub(self.tagline_size.width / 2);
        for line in self.properties.tagline.lines() {
            canvas.draw_str(middle_x, middle_y, theme.tagline, line);
            middle_y += 1;
        }

        // Draw credits
        middle_y += 1;
        let middle_x = (frame.size.width / 2).saturating_sub(self.credits_size.width / 2);
        for line in self.properties.credits.lines() {
            canvas.draw_str(middle_x, middle_y, theme.credits, line);
            middle_y += 1;
        }

        Layout::Canvas(canvas)
    }
}

#[derive(Debug, Default)]
struct SplashGrid {
    theme: Theme,
}

impl Component for SplashGrid {
    type Message = usize;
    type Properties = ();

    fn create(
        _properties: Self::Properties,
        _link: ComponentLink<Self>,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> Self {
        Default::default()
        // Self { properties }
    }

    fn change(
        &mut self,
        _properties: Self::Properties,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender {
        ShouldRender::Yes
    }

    fn view(&self, _frame: Rect) -> Layout {
        layout::column([layout::stretched(layout::component::<Splash>(Properties {
            theme: self.theme.clone(),
            logo: SPLASH_LOGO.into(),
            tagline: SPLASH_TAGLINE.into(),
            credits: SPLASH_CREDITS.into(),
            offset: 0,
        }))])
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        let transition;
        let message = match pressed {
            &[Key::Ctrl('x'), Key::Ctrl('c')] => {
                transition = BindingTransition::Exit;
                None
            }
            &[Key::Ctrl('x')] => {
                transition = BindingTransition::Continue;
                None
            }
            _ => {
                transition = BindingTransition::Clear;
                None
            }
        };
        BindingMatch {
            transition,
            message,
        }
    }
}

const SPLASH_LOGO: &str = r#"
   ▄████████    ▄███████▄  ▄█          ▄████████    ▄████████    ▄█    █▄
  ███    ███   ███    ███ ███         ███    ███   ███    ███   ███    ███
  ███    █▀    ███    ███ ███         ███    ███   ███    █▀    ███    ███
  ███          ███    ███ ███         ███    ███   ███         ▄███▄▄▄▄███▄▄
▀███████████ ▀█████████▀  ███       ▀███████████ ▀███████████ ▀▀███▀▀▀▀███▀
         ███   ███        ███         ███    ███          ███   ███    ███
   ▄█    ███   ███        ███▌    ▄   ███    ███    ▄█    ███   ███    ███
 ▄████████▀   ▄████▀      █████▄▄██   ███    █▀   ▄████████▀    ███    █▀
"#;
const SPLASH_TAGLINE: &str = "a splash screen for the terminal";
const SPLASH_CREDITS: &str = "C-x C-c to quit";

fn main() -> Result<()> {
    let mut app = App::new_with_component(SplashGrid::default())?;
    app.run_event_loop(Termion::new()?)
}
