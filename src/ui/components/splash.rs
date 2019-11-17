use super::{Component, Context};
use crate::ui::{Screen, Style};

#[derive(Clone, Debug)]
pub struct Theme {
    pub logo: Style,
    pub tagline: Style,
    pub credits: Style,
}

#[derive(Debug, Default)]
pub struct Splash;

impl Component for Splash {
    #[inline]
    fn draw(&mut self, screen: &mut Screen, context: &Context) {
        let theme = &context.theme.splash;

        screen.clear_region(context.frame, theme.logo);

        let middle_x = context.frame.origin.x + (context.frame.size.width / 2).saturating_sub(28);
        let mut middle_y =
            context.frame.origin.y + (context.frame.size.height / 2).saturating_sub(12);
        for line in SPLASH_LOGO.lines() {
            screen.draw_str(middle_x, middle_y, theme.logo, line);
            middle_y += 1;
        }
        for line in SPLASH_TAGLINE.lines() {
            screen.draw_str(middle_x, middle_y, theme.tagline, line);
            middle_y += 1;
        }
        for line in SPLASH_CREDITS.lines() {
            screen.draw_str(middle_x, middle_y, theme.credits, line);
            middle_y += 1;
        }
    }
}

const SPLASH_LOGO: &str = r#"
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
const SPLASH_TAGLINE: &str = r#"

             a modern editor for the terminal


"#;
const SPLASH_CREDITS: &str = r#"
                       version 0.1
               by Marius Cobzarenco et al.
       zee is open source and freely distributable
"#;
