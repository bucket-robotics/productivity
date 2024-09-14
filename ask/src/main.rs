use std::sync::Arc;

use anyhow::Context;
use argh::FromArgs;

use llm_client::{LlmClient, LlmQuery};
use ollama::OllamaClient;

mod anthropic_client;
mod host_info;
mod llm_client;
mod ollama;
mod path_utils;
mod response_parsing;
mod tools;

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
    let mut instructions = Vec::with_capacity(6);
    let mut formatting = Vec::with_capacity(6);
    let mut environment = Vec::with_capacity(6);
    let host_info = host_info::HostInformation::new();

    instructions.push(
        r"
1. You are assisting users through a CLI command they have run in their terminal.
2. The user doesn't have the ability to respond to follow up questions.
4. As you get information from tools, reevaluate the plan and provide the user with a summary of the new plan.
5. Do not retry failed tool uses.
6. Do not provide any information that the user has not requested.
7. Think step by step.
8. Information about the user's environment is available in the <environment> tags, use tools when necessary to gather more information.
9. When a README.md file is present in a directory a user mentions or a directory that is relevant, it should be read to gather context.
10. Act as if you are a tool, not a person, omit all pleasantries, do not thank the user, never apologize.
"
        .trim()
        .to_string(),
    );
    formatting.push(r"
        Format your responses for terminal readability.
        use Markdown formatting in your responses, start your message with your thoughts, put your thoughts under a Markdown '# Thoughts' heading, put your planning under a Markdown '# Plan' heading, use other Markdown headings such as '# Result' or '# Error' as needed, do not use ':' at the end of headings.
        Use italic text to highlight file paths, commands, and tool names in your output.
        Use bold text if you want the user to pay particular attention to something,
        Use ASCII art for diagrams.
        Use UTF-8 emojis to make things more fun or to draw the user's attention to sections of output.
    ".trim().to_string());
    formatting.push(format!(
        "The width of the user's terminal is {terminal_width} characters."
    ));

    environment.push(format!(
        "The user's operating system is {} and their CPU architecture is {}.",
        &host_info.os, &host_info.architecture
    ));
    environment.push(format!(
        "The current date and time is {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M")
    ));

    if let Ok(cwd) = std::env::current_dir() {
        environment.push(format!(
            "The user's current directory is {}.",
            cwd.display()
        ));
    }

    if let Some(user_dirs) = directories::UserDirs::new() {
        environment.push(format!(
            "The user's home directory is {}.",
            user_dirs.home_dir().display()
        ));
    }

    if let Ok(current_desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        environment.push(format!(
            "The user's desktop environment is {current_desktop}."
        ));
    }

    if let Some(config_path) = &settings.config_file_path {
        environment.push(format!(
            "The user has a configuration file for the CLI tool they use to interact with you, the config file is located at {config_path}."
        ));
    }

    if let Some(extra_system_prompt) = &settings.ask_system_prompt {
        instructions.push("\n".to_string());
        instructions.push(extra_system_prompt.clone());
    }

    format!(
        "{}\n{}\n{}",
        instructions.join("\n"),
        formatting.join("\n"),
        environment.join("\n"),
    )
}

async fn actual_main<C: LlmClient>(
    client: C,
    config: &productivity_config::Config,
    ask: Ask,
) -> anyhow::Result<()> {
    let mut tool_map = std::collections::HashMap::<String, Arc<dyn tools::Tool>>::new();
    let mut tool_definitions = vec![];
    for tool in tools::rust_tools::get_rust_tools() {
        let definition = tool.get_definition();
        tool_map.insert(definition.name.to_string(), tool);
        tool_definitions.push(definition);
    }

    let mut original_query = C::Query::create_query(get_system_prompt(config));
    original_query.add_question(ask.question.join(" "));

    let mut new_message = true;
    let printer = response_parsing::Printer::new();

    while new_message {
        new_message = false;

        let (response, mut new_query) = client.query(original_query.clone()).await?;

        // Print the communication
        for content in &response.text {
            printer.print(content);
        }

        // If tool use is requested then run the tools and send a new message
        if !response.tool_invocations.is_empty() {
            println!("----------");
            let mut tool_pairs = Vec::with_capacity(response.tool_invocations.len());
            for invocation in response.tool_invocations {
                if tracing::enabled!(tracing::Level::INFO) {
                    if let Ok(serialized_input) = serde_json::to_string_pretty(&invocation.input)
                        .context("Serializing tool input")
                    {
                        tracing::info!("Calling {} with:\n{}", &invocation.name, serialized_input);
                    }
                }

                let tool = tool_map
                    .get(&invocation.name)
                    .context("Tool not found")?
                    .clone();
                let tool_response = if let Err(message) = tool.get_prequisites().is_satisfied() {
                    format!("Could not run {}:\n{message}", &invocation.name)
                } else {
                    tool.run(invocation.input).await?
                };
                tool_pairs.push((invocation.id, tool_response));
            }

            new_query.add_tool_results(tool_pairs);

            // Send a new message with the tool results
            new_message = true;
        }

        original_query = new_query;
    }

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

fn main() -> anyhow::Result<()> {
    let ask: Ask = argh::from_env();
    set_up_tracing(ask.verbose);

    if ask.question.is_empty() {
        tracing::error!("No question provided");
        std::process::exit(1);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let config = productivity_config::Config::get_or_default().context("Reading config")?;
    let result = match &config.llm_provider {
        productivity_config::LlmProvider::Ollama { model, .. } => {
            let ollama_client = OllamaClient {
                base_url: config.llm_provider.get_url_base().to_string(),
                model: model.clone().unwrap_or_else(|| String::from("llama3.1:8b")),
            };
            runtime.block_on(actual_main(ollama_client, &config, ask))
        }
        productivity_config::LlmProvider::Anthropic { api_key } => {
            if api_key.is_empty() {
                anyhow::bail!(
                    "Anthropic API key is not set - configure it in {}",
                    config
                        .config_file_path
                        .as_deref()
                        .unwrap_or("the config file")
                );
            }
            runtime.block_on(actual_main(
                anthropic_client::AnthropicClient {
                    base_url: config.llm_provider.get_url_base().to_string(),
                    token: api_key.to_string(),
                },
                &config,
                ask,
            ))
        }
    };

    if let Err(e) = result {
        tracing::error!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
