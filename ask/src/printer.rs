/// The printer maintains the XML parsing state and prints data as it receives it.
pub struct Printer<W> {
    /// The destination to write to.
    writer: W,
    /// The current buffer of text to print.
    buffer: String,
    /// The current style to print with.
    style: console::Style,
}

/// Set the color of the printer.
///
/// # Returns
///
/// Whether or not the color was valid.
fn tag_to_style(tag: &str) -> Option<console::Style> {
    Some(match tag {
        "red" => console::Style::new().red(),
        "yellow" => console::Style::new().yellow(),
        "green" => console::Style::new().green(),
        "blue" => console::Style::new().blue(),
        "cyan" => console::Style::new().cyan(),
        "bold" => console::Style::new().bold(),
        "italic" => console::Style::new().italic(),
        _ => return None,
    })
}

impl<W: std::io::Write> Printer<W> {
    /// Create a new printer.
    pub fn new(writer: W) -> Self {
        Printer {
            writer,
            buffer: String::new(),
            style: console::Style::new(),
        }
    }

    /// Print the given text.
    pub fn print(&mut self, text: &str) -> anyhow::Result<()> {
        self.buffer.push_str(text);

        let mut text = self.buffer.as_str();
        loop {
            if text.is_empty() {
                self.buffer.clear();
                break;
            }

            if text.starts_with('<') {
                if let Some(index) = text.find('>') {
                    let tag = &text[1..index];

                    let is_color_tag = if let Some(tag) = tag.strip_prefix("/") {
                        if tag_to_style(tag).is_some() {
                            self.style = console::Style::new();
                            true
                        } else {
                            false
                        }
                    } else if let Some(style) = tag_to_style(tag) {
                        self.style = style;
                        true
                    } else {
                        false
                    };

                    if !is_color_tag {
                        write!(&mut self.writer, "{}", self.style.apply_to(&text[..=index]))?;
                    }

                    text = &text[index + 1..];
                    continue;
                }

                // If we don't have a closing tag, we need to buffer the text.
                if text.len() < 10 {
                    self.buffer = text.to_string();
                    break;
                }

                // If we have a bunch of characters and no closing tag skip the token
                write!(&mut self.writer, "{}", self.style.apply_to("<"))?;
                text = &text[1..];
                continue;
            }

            if let Some(index) = text.find('<') {
                write!(&mut self.writer, "{}", self.style.apply_to(&text[..index]))?;
                text = &text[index..];
                continue;
            }

            write!(&mut self.writer, "{}", self.style.apply_to(text))?;
            self.buffer.clear();
            break;
        }

        Ok(())
    }

    /// Flush the printer.
    pub fn flush(&mut self) {
        write!(
            &mut self.writer,
            "{}",
            self.style.apply_to(self.buffer.trim_end())
        )
        .unwrap();
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_printer_basic() {
        let mut output = Vec::new();
        let mut printer = Printer::new(&mut output);
        printer.print("Hello, world!").unwrap();
        printer.flush();
        assert_eq!(String::from_utf8(output).unwrap(), "Hello, world!");
    }

    #[test]
    fn test_printer_with_color() {
        let mut output = Vec::new();
        let mut printer = Printer::new(&mut output);
        printer.print("<red>Red text</red>").unwrap();
        printer.flush();
        // Note: This test assumes that the ANSI color codes are used.
        // The exact string may need to be adjusted based on the actual implementation.
        assert!(String::from_utf8(output).unwrap().contains("Red text"));
    }

    #[test]
    fn test_printer_multiple_colors() {
        let mut output = Vec::new();
        let mut printer = Printer::new(&mut output);
        printer.print("<red>Red</red><blue>Blue</blue>").unwrap();
        printer.flush();
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("Red"));
        assert!(result.contains("Blue"));
    }

    #[test]
    fn test_printer_incomplete_tag() {
        let mut output = Vec::new();
        let mut printer = Printer::new(&mut output);
        printer.print("Text with <incomplet").unwrap();
        printer.flush();
        assert_eq!(String::from_utf8(output).unwrap(), "Text with <incomplet");
    }

    #[test]
    fn test_printer_streamed_tag_tag() {
        let mut output = Vec::new();
        let mut printer = Printer::new(&mut output);
        printer.print("Text with <r").unwrap();
        printer.print("ed").unwrap();
        printer.print(">").unwrap();
        printer.flush();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "Text with \u{1b}[31m\u{1b}[0m"
        );
    }

    #[test]
    fn test_printer_invalid_tag() {
        let mut output = Vec::new();
        let mut printer = Printer::new(&mut output);
        printer.print("<invalid>Text</invalid>").unwrap();
        printer.flush();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "<invalid>Text</invalid>"
        );
    }
}
