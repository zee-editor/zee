use zi::{Canvas, Component, ComponentLink, Layout, Rect, ShouldRender, Style};

#[derive(Clone, PartialEq)]
pub struct Properties {
    pub style: Style,
    pub line_offset: usize,
    pub num_lines: usize,
}

pub struct LineInfo {
    properties: Properties,
    frame: Rect,
}

impl Component for LineInfo {
    type Message = ();
    type Properties = Properties;

    fn create(properties: Self::Properties, frame: Rect, _link: ComponentLink<Self>) -> Self {
        LineInfo { properties, frame }
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

    fn view(&self) -> Layout {
        let Self {
            properties:
                Properties {
                    style,
                    line_offset,
                    num_lines,
                },
            frame,
        } = *self;

        let mut canvas = Canvas::new(frame.size);
        for line_index in 0..frame.size.height {
            canvas.draw_str(
                0,
                line_index as usize,
                style,
                if line_offset + line_index < num_lines - 1 {
                    " "
                } else {
                    "╶"
                },
            );
        }
        canvas.into()
    }
}
