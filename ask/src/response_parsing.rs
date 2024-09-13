//! XML response parsing library.

use quick_xml::events::Event;

use crate::llm_client::TextOutput;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TagType {
    Thought,
    Text,
    Followup,
}

struct TextAccumulator<'a> {
    output: &'a mut Vec<TextOutput>,
    text: String,
    tag_stack: Vec<TagType>,
    bold: bool,
    italic: bool,
    color: Option<String>,
    did_skip: bool,
}

impl TextAccumulator<'_> {
    /// Wrap an output vector in a text accumulator.
    fn new(output: &mut Vec<TextOutput>) -> TextAccumulator {
        TextAccumulator {
            output,
            text: String::with_capacity(8),
            bold: false,
            tag_stack: Vec::with_capacity(2),
            italic: false,
            color: None,
            did_skip: false,
        }
    }

    /// Push a block.
    fn push(&mut self) {
        let should_skip = self
            .tag_stack
            .iter()
            .any(|x| matches!(x, TagType::Thought | TagType::Followup));
        if should_skip {
            self.text = String::with_capacity(8);
            self.did_skip = true;
            return;
        }

        if !self.text.is_empty() {
            let mut output = TextOutput {
                bold: self.bold,
                italic: self.italic,
                color: self.color.clone(),
                text: String::with_capacity(8),
            };
            std::mem::swap(&mut self.text, &mut output.text);
            self.output.push(output);
        }
    }

    fn push_tag(&mut self, tag: TagType) {
        self.push();
        self.tag_stack.push(tag);
    }

    fn pop_tag_of_type(&mut self, tag: TagType) -> anyhow::Result<()> {
        self.push();

        if let Some(last) = self.tag_stack.pop() {
            if last != tag {
                anyhow::bail!("Expected tag {:?}, got {:?}", tag, last);
            }
        } else {
            anyhow::bail!("Expected tag {:?} to be in the stack, got nothing", tag);
        }
        Ok(())
    }

    fn set_bold(&mut self, state: bool) {
        if self.bold == state {
            return;
        }
        self.push();
        self.bold = state;
    }

    fn set_italic(&mut self, state: bool) {
        if self.italic == state {
            return;
        }
        self.push();
        self.italic = state;
    }

    fn set_color(&mut self, color: Option<String>) {
        if self.color == color {
            return;
        }
        self.push();
        self.color = color;
    }

    fn push_text(&mut self, text: &str) {
        self.text.push_str(if self.did_skip {
            self.did_skip = false;
            text.trim_start()
        } else {
            text
        });
    }
}

pub fn parse_text(text: &str, output: &mut Vec<TextOutput>) -> anyhow::Result<()> {
    let mut reader = quick_xml::reader::Reader::from_str(text);
    let mut writer = TextAccumulator::new(output);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => panic!("Error at position {}: {:?}", reader.error_position(), e),
            // exits the loop when reaching end of file
            Ok(Event::Eof) => {
                // Make sure the content ends in a newline.
                writer.push_text("\n");
                break;
            }

            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"thought" => {
                    writer.push_tag(TagType::Thought);
                }
                b"followup" => {
                    writer.push_tag(TagType::Followup);
                }
                b"text" => {
                    writer.push_tag(TagType::Text);
                }
                b"bold" => {
                    writer.set_bold(true);
                }
                b"italic" => {
                    writer.set_italic(true);
                }
                b"red" => {
                    writer.set_color(Some("red".to_string()));
                }
                b"green" => {
                    writer.set_color(Some("green".to_string()));
                }
                b"yellow" => {
                    writer.set_color(Some("yellow".to_string()));
                }
                _ => writer.push_text(&format!("<{}>", std::str::from_utf8(&e).unwrap())),
            },
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"thought" => {
                    writer.pop_tag_of_type(TagType::Thought)?;
                }
                b"followup" => {
                    writer.pop_tag_of_type(TagType::Followup)?;
                }
                b"text" => {
                    writer.pop_tag_of_type(TagType::Text)?;
                }
                b"bold" => {
                    writer.set_bold(false);
                }
                b"italic" => {
                    writer.set_italic(false);
                }
                b"red" | b"green" | b"yellow" => {
                    writer.set_color(None);
                }
                _ => writer.push_text(&format!("</{}>", std::str::from_utf8(&e).unwrap())),
            },
            Ok(e) => writer.push_text(std::str::from_utf8(&e).unwrap()),
        }

        buf.clear();
    }

    // Make sure any remaining content is flushed
    writer.push();

    Ok(())
}
