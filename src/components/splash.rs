use once_cell::sync::Lazy;
use pkg_version::{pkg_version_major, pkg_version_minor, pkg_version_patch};
use std::{borrow::Cow, cmp};
use unicode_width::UnicodeWidthStr;
use zi::{Canvas, Component, ComponentLink, Layout, Rect, ShouldRender, Size, Style};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub logo: Style,
    pub tagline: Style,
    pub credits: Style,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Properties {
    pub theme: Cow<'static, Theme>,
}

#[derive(Debug)]
pub struct Splash {
    properties: Properties,
    frame: Rect,
}

impl Component for Splash {
    type Message = ();
    type Properties = Properties;

    fn create(properties: Self::Properties, frame: Rect, _link: ComponentLink<Self>) -> Self {
        Self { properties, frame }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        if self.properties != properties {
            self.properties = properties;
            ShouldRender::Yes
        } else {
            ShouldRender::No
        }
    }

    fn resize(&mut self, frame: Rect) -> ShouldRender {
        self.frame = frame;
        ShouldRender::Yes
    }

    #[inline]
    fn view(&self) -> Layout {
        let Self {
            properties: Properties { ref theme },
            frame,
        } = *self;
        let logo_size = text_block_size(LOGO);
        let tagline_size = text_block_size(TAGLINE);
        let credits_size = text_block_size(&CREDITS);

        let mut canvas = Canvas::new(frame.size);
        canvas.clear(theme.logo);

        // Draw logo
        let middle_x = (frame.size.width / 2).saturating_sub(logo_size.width / 2);
        let mut middle_y = cmp::min(8, frame.size.height.saturating_sub(logo_size.height));
        for line in LOGO.lines() {
            canvas.draw_str(middle_x, middle_y, theme.logo, line);
            middle_y += 1;
        }

        // Draw tagline
        middle_y += 2;
        let middle_x = (frame.size.width / 2).saturating_sub(tagline_size.width / 2);
        for line in TAGLINE.lines() {
            canvas.draw_str(middle_x, middle_y, theme.tagline, line);
            middle_y += 1;
        }

        // Draw credits
        middle_y += 1;
        let middle_x = (frame.size.width / 2).saturating_sub(credits_size.width / 2);
        for line in CREDITS.lines() {
            canvas.draw_str(middle_x, middle_y, theme.credits, line);
            middle_y += 1;
        }

        Layout::Canvas(canvas)
    }
}

fn text_block_size(text: &str) -> Size {
    let width = text.lines().map(UnicodeWidthStr::width).max().unwrap_or(0);
    let height = text.lines().count();
    Size::new(width, height)
}

const LOGO: &str = r#"
zzzzzzzzzzzzzzzzz     eeeeeeeeeeee         eeeeeeeeeeee
z:::::::::::::::z   ee::::::::::::ee     ee::::::::::::ee
z::::::::::::::z   e::::::eeeee:::::ee  e::::::eeeee:::::ee
zzzzzzzz::::::z   e::::::e     e:::::e e::::::e     e:::::e
      z::::::z    e:::::::eeeee::::::e e:::::::eeeee::::::e
     z::::::z     e:::::::::::::::::e  e:::::::::::::::::e
    z::::::z      e::::::eeeeeeeeeee   e::::::eeeeeeeeeee
   z::::::z       e:::::::e            e:::::::e
  z::::::zzzzzzzz e::::::::e           e::::::::e
 z::::::::::::::z  e::::::::eeeeeeee    e::::::::eeeeeeee
z:::::::::::::::z   ee:::::::::::::e     ee:::::::::::::e
zzzzzzzzzzzzzzzzz     eeeeeeeeeeeeee       eeeeeeeeeeeeee
"#;
const TAGLINE: &str = "a modern editor for the terminal";

static CREDITS: Lazy<String> = Lazy::new(|| {
    format!(
        r#"
               version {}.{}.{}
        by Marius Cobzarenco et al.
zee is open source and freely distributable"#,
        MAJOR, MINOR, PATCH
    )
});

const MAJOR: u32 = pkg_version_major!();
const MINOR: u32 = pkg_version_minor!();
const PATCH: u32 = pkg_version_patch!();
