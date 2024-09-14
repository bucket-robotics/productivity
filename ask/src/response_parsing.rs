//! Markdown response parsing library.

use crate::llm_client::TextOutput;

pub struct Printer {
    syntax_set: syntect::parsing::SyntaxSet,
    theme: syntect::highlighting::Theme,
}

impl Printer {
    pub fn new() -> Self {
        let mut themes = syntect::highlighting::ThemeSet::load_defaults();

        Printer {
            syntax_set: syntect::parsing::SyntaxSet::load_defaults_newlines(),
            theme: themes.themes.remove("base16-ocean.dark").unwrap(),
        }
    }

    pub fn print(&self, text_output: &TextOutput) {
        match text_output {
            TextOutput::Text(text) => {
                print!("{text}");
            }
            TextOutput::Bold(text) => {
                print!("{}", console::style(text).bold());
            }
            TextOutput::Italic(text) => {
                print!("{}", console::style(text).italic());
            }
            TextOutput::InlineCode(text) => {
                print!("{}", console::style(text).dim());
            }
            TextOutput::CodeBlock { language, content } => {
                let syntax = self
                    .syntax_set
                    .find_syntax_by_token(language)
                    .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
                let mut h = syntect::easy::HighlightLines::new(syntax, &self.theme);
                for line in syntect::util::LinesWithEndings::from(content) {
                    let regions = h.highlight_line(line, &self.syntax_set).unwrap();
                    print!(
                        "{}",
                        syntect::util::as_24_bit_terminal_escaped(&regions[..], true)
                    );
                }
                // Force a style reset
                println!("\x1b[0m\n");
            }
            TextOutput::Newline => {
                println!();
            }
        }
    }
}

pub fn parse_text(text: &str, output: &mut Vec<TextOutput>) -> anyhow::Result<()> {
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_GFM);
    let parser = pulldown_cmark::Parser::new_ext(text, options);

    let mut code_block_language = None;
    let mut accumulated_text = String::with_capacity(16);
    let mut emphasised = false;
    let mut strong = false;
    let mut list_indent = None;
    let mut list_index = vec![];

    let mut current_heading_level = 0;
    let mut thoughts_heading = None;

    for event in pulldown_cmark::TextMergeStream::new(parser) {
        match event {
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading { level, .. }) => {
                // Truncate the thoughts heading if we find a new heading
                if let Some(thoughts_heading) = thoughts_heading.take() {
                    output.truncate(thoughts_heading);
                }

                let level = level as usize;
                let mut prefix = String::with_capacity(level + 1);
                for _ in 0..level {
                    prefix.push('#');
                }
                prefix.push(' ');
                output.push(TextOutput::Text(prefix));
                current_heading_level = level as i64;
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
                current_heading_level = 0;
                output.push(TextOutput::Newline);
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Paragraph) => {}
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Paragraph) => {
                output.push(TextOutput::Newline);
                output.push(TextOutput::Newline);
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Emphasis) => {
                emphasised = true;
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Emphasis) => {
                emphasised = false;
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Strong) => {
                strong = true;
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Strong) => {
                strong = false;
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::List(index)) => {
                list_index.push(index);
                list_indent = Some(match list_indent {
                    None => 1i64,
                    Some(x) => {
                        output.push(TextOutput::Newline);
                        x + 2
                    }
                });
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::List(_)) => match list_indent {
                None => tracing::error!("Markdown list ended without start"),
                Some(1) => {
                    list_indent = None;
                    output.push(TextOutput::Newline);
                }
                Some(x) => {
                    list_indent = Some(x - 2);
                }
            },
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Item) => {
                let indent = list_indent.unwrap_or(0);
                let mut prefix = String::with_capacity(indent as usize + 1);
                for _ in 0..indent {
                    prefix.push(' ');
                }
                match list_index.last_mut() {
                    Some(Some(num)) => {
                        prefix.push_str(&format!("{}. ", *num));
                        *num += 1;
                    }
                    _ => {
                        prefix.push_str("• ");
                    }
                }
                output.push(TextOutput::Text(prefix));
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Item) => {
                output.push(TextOutput::Newline);
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::CodeBlock(kind)) => {
                code_block_language = Some(match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(language) => language.to_string(),
                    pulldown_cmark::CodeBlockKind::Indented => String::new(),
                });
            }
            pulldown_cmark::Event::Text(text_content) => {
                // Terrible hack to cut out the thoughts content:
                // We save the index of the thoughts heading and truncate the output when we find a new heading
                if current_heading_level == 1 && text_content.eq_ignore_ascii_case("Thoughts") {
                    thoughts_heading = Some(output.len() - 1);
                }

                if code_block_language.is_some() {
                    accumulated_text.push_str(&text_content);
                } else if strong || current_heading_level == 1 {
                    output.push(TextOutput::Bold(text_content.to_string()));
                } else if emphasised {
                    output.push(TextOutput::Italic(text_content.to_string()));
                } else {
                    output.push(TextOutput::Text(text_content.to_string()));
                }
            }
            pulldown_cmark::Event::Code(code) => {
                output.push(TextOutput::InlineCode(code.to_string()));
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::CodeBlock) => {
                output.push(TextOutput::CodeBlock {
                    language: code_block_language.take().unwrap(),
                    content: accumulated_text.trim_end().to_string(),
                });
                accumulated_text = String::with_capacity(16);
            }
            x => {
                tracing::warn!("Unhandled Markdown event: {:?}", x);
            }
        }
    }

    Ok(())
}
