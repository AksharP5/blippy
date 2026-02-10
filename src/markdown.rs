use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

const TEXT: Color = Color::Rgb(226, 231, 238);
const MUTED: Color = Color::Rgb(119, 131, 149);
const ACCENT_PURPLE: Color = Color::Rgb(212, 171, 255);
const ACCENT_BLUE: Color = Color::Rgb(171, 229, 179);
const ACCENT_CYAN: Color = Color::Rgb(180, 223, 164);
const ACCENT_GREEN: Color = Color::Rgb(129, 199, 132);
const CODE_BG: Color = Color::Rgb(20, 26, 34);

#[derive(Debug, Default)]
pub struct RenderedMarkdown {
    pub lines: Vec<Line<'static>>,
    pub links: Vec<String>,
}

pub fn render(input: &str) -> RenderedMarkdown {
    let options = Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(input, options);

    let mut state = RenderState::new();
    for event in parser {
        state.handle(event);
    }

    let links = state.links.clone();
    let lines = state.finish();
    RenderedMarkdown {
        lines,
        links,
    }
}

struct RenderState {
    lines: Vec<Vec<Span<'static>>>,
    style_stack: Vec<Style>,
    links: Vec<String>,
    active_link: Option<usize>,
    list_depth: usize,
    blockquote_depth: usize,
    in_code_block: bool,
}

impl RenderState {
    fn new() -> Self {
        Self {
            lines: vec![Vec::new()],
            style_stack: vec![Style::default()],
            links: Vec::new(),
            active_link: None,
            list_depth: 0,
            blockquote_depth: 0,
            in_code_block: false,
        }
    }

    fn handle(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.push_text(text.as_ref()),
            Event::Code(text) => {
                let style = Style::default().fg(ACCENT_CYAN).bg(CODE_BG);
                self.push_span(Span::styled(text.into_string(), style));
            }
            Event::SoftBreak | Event::HardBreak => self.new_line(),
            Event::Rule => {
                self.new_line();
                self.push_span(Span::styled(
                    "----------------------------------------".to_string(),
                    Style::default().fg(MUTED),
                ));
                self.new_line();
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "[x] " } else { "[ ] " };
                self.push_text(marker);
            }
            _ => {}
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.ensure_blank_line();
                let style = heading_style(level);
                self.style_stack.push(style);
            }
            Tag::Emphasis => self.push_style(Style::default().add_modifier(Modifier::ITALIC)),
            Tag::Strong => self.push_style(Style::default().add_modifier(Modifier::BOLD)),
            Tag::Strikethrough => self.push_style(Style::default().add_modifier(Modifier::CROSSED_OUT)),
            Tag::BlockQuote(_) => {
                self.blockquote_depth += 1;
                self.new_line();
                self.push_text(&"| ".repeat(self.blockquote_depth));
            }
            Tag::List(_) => {
                self.list_depth += 1;
                self.new_line();
            }
            Tag::Item => {
                self.new_line();
                self.push_text(&format!("{}- ", "  ".repeat(self.list_depth.saturating_sub(1))));
            }
            Tag::CodeBlock(_) => {
                self.in_code_block = true;
                self.new_line();
                self.push_style(Style::default().fg(ACCENT_GREEN).bg(CODE_BG));
            }
            Tag::Link { dest_url, .. } => {
                self.links.push(dest_url.to_string());
                self.active_link = Some(self.links.len());
                self.push_style(
                    Style::default()
                        .fg(ACCENT_CYAN)
                        .add_modifier(Modifier::UNDERLINED),
                );
            }
            Tag::Paragraph => {
                self.ensure_blank_line();
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) => {
                self.pop_style();
                self.new_line();
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.pop_style();
            }
            TagEnd::BlockQuote(_) => {
                if self.blockquote_depth > 0 {
                    self.blockquote_depth -= 1;
                }
                self.new_line();
            }
            TagEnd::List(_) => {
                if self.list_depth > 0 {
                    self.list_depth -= 1;
                }
                self.new_line();
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                self.pop_style();
                self.new_line();
            }
            TagEnd::Link => {
                self.pop_style();
                if let Some(index) = self.active_link.take() {
                    self.push_span(Span::styled(
                        format!("[{}]", index),
                        Style::default().fg(ACCENT_CYAN),
                    ));
                }
            }
            TagEnd::Paragraph => {
                self.new_line();
            }
            _ => {}
        }
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        while self
            .lines
            .last()
            .is_some_and(|line| line.is_empty())
            && self.lines.len() > 1
        {
            self.lines.pop();
        }

        self.lines
            .into_iter()
            .map(Line::from)
            .collect::<Vec<Line<'static>>>()
    }

    fn ensure_blank_line(&mut self) {
        if self
            .lines
            .last()
            .is_some_and(|line| !line.is_empty())
        {
            self.new_line();
        }
    }

    fn new_line(&mut self) {
        self.lines.push(Vec::new());
        if self.blockquote_depth > 0 {
            self.push_text(&"| ".repeat(self.blockquote_depth));
        }
    }

    fn push_style(&mut self, style: Style) {
        let current = self.current_style();
        let merged = current.patch(style);
        self.style_stack.push(merged);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn current_style(&self) -> Style {
        self.style_stack
            .last()
            .copied()
            .unwrap_or_default()
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        let style = self.current_style();
        self.push_span(Span::styled(text.to_string(), style));
    }

    fn push_span(&mut self, span: Span<'static>) {
        if let Some(line) = self.lines.last_mut() {
            line.push(span);
            return;
        }

        self.lines.push(vec![span]);
    }
}

fn heading_style(level: HeadingLevel) -> Style {
    match level {
        HeadingLevel::H1 => Style::default().fg(ACCENT_PURPLE).add_modifier(Modifier::BOLD),
        HeadingLevel::H2 => Style::default().fg(ACCENT_BLUE).add_modifier(Modifier::BOLD),
        HeadingLevel::H3 => Style::default().fg(ACCENT_CYAN).add_modifier(Modifier::BOLD),
        _ => Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
    }
}

#[cfg(test)]
mod tests {
    use super::render;

    #[test]
    fn renders_heading_and_list() {
        let markdown = "# Title\n\n- one\n- two";
        let rendered = render(markdown);
        let text = rendered
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<String>>()
            .join("\n");

        assert!(text.contains("Title"));
        assert!(text.contains("- one"));
        assert!(text.contains("- two"));
    }

    #[test]
    fn captures_link_references() {
        let markdown = "See [docs](https://example.com)";
        let rendered = render(markdown);
        assert_eq!(rendered.links, vec!["https://example.com".to_string()]);
    }
}
