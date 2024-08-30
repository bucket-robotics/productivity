use std::sync::Arc;

use anthropic_client::{
    ContentBlock, ContentBlockProcessor as _, TextContentBlock, TextOrContentVector,
    ToolUseContentBlock,
};
use anyhow::Context;
use argh::FromArgs;

mod anthropic_client;
mod anthropic_tools;
mod host_info;
mod printer;

#[derive(FromArgs)]
/// Ask a question.
struct Ask {
    #[argh(switch, short = 'v')]
    /// verbose mode
    verbose: bool,
    #[argh(positional, greedy)]
    /// the question to ask
    question: Vec<String>,
}

fn get_system_prompt(settings: &productivity_config::Config) -> String {
    let terminal = console::Term::stdout();
    let terminal_width = terminal.size_checked().map_or(80, |s| s.1) as usize;
    let mut instructions = Vec::new();
    let host_info = host_info::HostInformation::new();

    instructions.push(
        r#"
You are assisting users through a CLI command they have run in their terminal.
The user doesn't have the ability to respond to follow up questions.
Use ASCII art for diagrams.
Use UTF-8 emojis to make things more fun or to draw the user's attention to sections of output.
Format your responses for terminal readability and use ASCII-based formatting.
When you formulate a plan of tool uses, provide the user with a simple ASCII art diagram of the plan, but do not elaborate further.
As you get information from tools, reevaluate the plan and provide the user with a new ASCII art diagram.
Do not retry failed tool uses.
Do not provide any information that the user has not requested.
Think step by step, indicate thinking by responding with blocks starting with ">".
Do not include extra newlines in your responses.
You are not a chatbot, be concise and matter of fact, do not engage in conversation, omit all pleasantries, do not thank the user, never apologize.
Act as if you are a tool, not a person.
"#
        .trim()
        .to_string(),
    );
    instructions.push(format!(
        "The width of the user's terminal is {terminal_width} characters."
    ));
    instructions.push(format!(
        "The user's operating system is {} and their CPU architecture is {}.",
        &host_info.os, &host_info.architecture
    ));
    instructions.push(format!(
        "The current date and time is {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M")
    ));

    if let Ok(cwd) = std::env::current_dir() {
        instructions.push(format!(
            "The user's current directory is {}.",
            cwd.display()
        ));
    }

    if let Ok(current_desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        instructions.push(format!(
            "The user's desktop environment is {current_desktop}."
        ));
    }

    if console::colors_enabled() {
        instructions.push("The user's terminal supports ANSI colors - can you use XML tags to denote the color to display text for example \"<red>\". Use colors to increase legibility.".to_string());
        instructions.push(
            "You can use XML tags to make text bold or italic, \"<bold>\" will make text bold and \"<italic>\" will make text italic.".to_string(),
        );
        instructions.push(
            "Use italic text to highlight file paths, commands, and tool names in your output. Use bold text if you want the user to pay particular attention to something.".to_string(),
        );
    }

    if let Some(config_path) = &settings.config_file_path {
        instructions.push(format!(
            "The user has a configuration file for the CLI tool they use to interact with you - it is located at {config_path}."
        ));
    }
    if let Some(extra_system_prompt) = &settings.ask_system_prompt {
        instructions.push(extra_system_prompt.clone());
    }

    instructions.join("\n")
}

async fn actual_main(ask: Ask) -> anyhow::Result<()> {
    let config = productivity_config::Config::get_or_default().context("Reading config")?;
    let client = std::sync::Arc::new(anthropic_client::AnthropicClient {
        base_url: config.get_anthropic_url_base(),
        token: config.get_anthropic_api_key().context("Getting API key")?,
    });

    let mut tool_map = std::collections::HashMap::<String, Arc<dyn anthropic_tools::Tool>>::new();
    let mut tool_definitions = vec![];
    for tool in anthropic_tools::rust_tools::get_rust_tools() {
        let definition = tool.get_definition();
        tool_map.insert(definition.name.to_string(), tool);
        tool_definitions.push(definition);
    }

    let mut original_query = anthropic_client::AnthropicQuery {
        messages: vec![anthropic_client::AnthropicMessage {
            role: "user".to_string(),
            content: TextOrContentVector::Text(ask.question.join(" ")),
        }],
        system: Some(get_system_prompt(&config)),
        tools: tool_definitions,
        ..Default::default()
    };

    let mut new_message = true;

    while new_message {
        let mut content_blocks = std::collections::BTreeMap::<i64, ContentBlock>::new();
        new_message = false;

        let (tx, mut rx) =
            tokio::sync::mpsc::channel::<anthropic_client::AnthropicStreamResponse>(128);
        let query = original_query.clone();
        let client_clone = client.clone();
        let response =
            tokio::spawn(async move { client_clone.query_anthropic(query.clone(), tx).await });

        while let Some(message) = rx.recv().await {
            tracing::debug!("Received message: {:?}", &message);

            match message.r#type.as_str() {
                "content_block_delta" => {
                    let Some(index) = message.index else {
                        tracing::error!("No index in delta message: {:?}", &message);
                        continue;
                    };
                    let Some(delta) = message.delta else {
                        tracing::error!("No delta in delta message: {:?}", &message);
                        continue;
                    };
                    let Some(content_block_state) = content_blocks.get_mut(&index) else {
                        tracing::error!("No content block for index {index}");
                        continue;
                    };
                    content_block_state.process_delta(delta)?;
                }
                "message_delta" => {
                    let Some(ref delta) = message.delta else {
                        tracing::error!("No delta in message: {:?}", &message);
                        continue;
                    };
                    if let Some(stop_reason) = delta.get("stop_reason") {
                        match stop_reason.as_str().unwrap() {
                            "end_turn" | "stop_sequence" => {
                                break;
                            }
                            "max_tokens" => {
                                anyhow::bail!("Maximum token count exceeded");
                            }
                            "tool_use" => {
                                let mut assistant_message_content = vec![];
                                let mut user_message_content = vec![];
                                for content_block in content_blocks.values() {
                                    assistant_message_content
                                        .push(content_block.get_original_content_block()?);
                                    if let Some(user_content) =
                                        content_block.get_user_content_block().await
                                    {
                                        user_message_content.push(user_content?);
                                    }
                                }
                                original_query
                                    .messages
                                    .push(anthropic_client::AnthropicMessage {
                                        role: "assistant".to_string(),
                                        content: TextOrContentVector::Content(
                                            assistant_message_content,
                                        ),
                                    });

                                original_query
                                    .messages
                                    .push(anthropic_client::AnthropicMessage {
                                        role: "user".to_string(),
                                        content: TextOrContentVector::Content(user_message_content),
                                    });

                                new_message = true;
                            }
                            _ => {
                                tracing::error!("Unknown stop reason: {:?}", &stop_reason);
                            }
                        }
                    }
                }
                "content_block_start" => {
                    let Some(ref content_block) = message.content_block else {
                        continue;
                    };
                    let Some(content_type) = content_block.get("type") else {
                        continue;
                    };
                    let Some(index) = message.index else {
                        anyhow::bail!("No index in message: {:?}", message);
                    };
                    content_blocks.insert(
                        index,
                        match content_type.as_str().unwrap() {
                            "text" => ContentBlock::Text(TextContentBlock::new(content_block)?),
                            "tool_use" => {
                                let tool_name = content_block
                                    .get("name")
                                    .context("Tool name not provided")?
                                    .as_str()
                                    .context("Tool name was not a string")?;
                                let tool = tool_map.get(tool_name).with_context(|| {
                                    format!("Could not find tool with name {tool_name}")
                                })?;
                                ContentBlock::ToolUse(ToolUseContentBlock::new(
                                    tool.clone(),
                                    content_block,
                                )?)
                            }
                            _ => {
                                tracing::error!("Unknown content block type: {:?}", &&content_type);
                                continue;
                            }
                        },
                    );
                }
                "content_block_stop" => {
                    let Some(index) = message.index else {
                        tracing::error!("No index in delta message: {:?}", &message);
                        continue;
                    };
                    let Some(content_block_state) = content_blocks.get_mut(&index) else {
                        tracing::error!("No content block for index {index}");
                        continue;
                    };
                    content_block_state.finalize()?;
                }
                "message_start" | "message_stop" | "ping" => {}
                "error" => {
                    anyhow::bail!("Received error message: {:?}", &message);
                }
                _ => {
                    tracing::error!("Unknown message type in: {:?}", &message);
                }
            }
        }

        response.await?.context("During query")?;
    }

    // Make sure the output ends in a newline
    println!();

    Ok(())
}

/// Set up tracing.
fn set_up_tracing(verbose: bool) {
    use tracing_subscriber::prelude::*;

    let env_filter =
        tracing_subscriber::filter::EnvFilter::try_from_default_env().unwrap_or(if verbose {
            tracing_subscriber::filter::EnvFilter::new("INFO")
        } else {
            tracing_subscriber::filter::EnvFilter::new("WARN")
        });

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::Layer::default()
                .pretty()
                .with_writer(std::io::stderr)
                .boxed(),
        )
        .with(env_filter)
        .init();
}

fn main() {
    let ask: Ask = argh::from_env();
    set_up_tracing(ask.verbose);

    if ask.question.is_empty() {
        tracing::error!("No question provided (TODO provide a little TUI)");
        std::process::exit(1);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    if let Err(e) = runtime.block_on(actual_main(ask)) {
        tracing::error!("Error: {e}");
        std::process::exit(1);
    }
}
