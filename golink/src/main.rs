use anyhow::Context;
use argh::FromArgs;

#[derive(FromArgs)]
/// CLI to access `go/` links.
struct Go {
    #[argh(positional, default = "String::from(\"\")")]
    /// the link to open
    link: String,
    #[argh(switch)]
    /// print the link instead of opening it
    print: bool,
    #[argh(switch)]
    /// print the query result as JSON
    json: bool,
}

/// Attempt to open a link.
///
/// If opening the link fails then just print out the URL.
fn open_link(link: &str) -> () {
    if let Err(e) = open::that(link) {
        eprintln!("Failed to open browser: {}", e);
        println!("{link}");
    }
}

async fn actual_main(go: Go) -> anyhow::Result<(), Box<dyn std::error::Error>> {
    let config = productivity_config::Config::get_or_default().context("Reading config")?;
    let client = orgorg_client::Client::new_with_url(
        config
            .get_orgorg_api_key()
            .ok_or_else(|| anyhow::anyhow!("No API key configured"))?,
        config.get_orgorg_url_base(),
    );
    let response = client
        .go_find(&go.link)
        .await
        .context("Querying `go/` links")?;

    if go.json {
        serde_json::to_writer_pretty(std::io::stdout(), &response).context("Writing response")?;
        return Ok(());
    }

    let link_url = if let Some(link) = response.links.iter().find(|link| link.name == go.link) {
        link.url.clone()
    } else if response.links.is_empty() {
        eprintln!("{}", console::style("No links found").red());
        std::process::exit(1);
    } else {
        // If there are multiple matches prompt the user for a selection
        let options: Vec<String> = response
            .links
            .iter()
            .map(|x| format!("{}: {}", x.name, x.url))
            .collect();

        let Some(selection) = dialoguer::FuzzySelect::new()
            .with_prompt("Choose a link (or press ESC) -")
            .items(&options)
            .interact_opt()
            .unwrap()
        else {
            std::process::exit(0)
        };

        // This is a hideous way to do this
        options[selection]
            .split_once(':')
            .unwrap()
            .1
            .trim()
            .to_owned()
    };

    if go.print {
        println!("{}", &link_url);
    } else {
        open_link(&link_url);
    }

    Ok(())
}

fn main() {
    let go: Go = argh::from_env();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    if let Err(e) = runtime.block_on(actual_main(go)) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
