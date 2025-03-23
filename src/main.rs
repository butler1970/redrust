use crate::cli::{Cli, Commands};
use clap::Parser;
use log::error;
use redrust::{
    operations::{
        api_create::handle_api_create_command_with_client,
        browser_create::handle_browser_create_command_with_client,
        comment::{
            handle_browser_comment_command_with_client, handle_comment_command_with_client,
            handle_user_comment_command_with_client,
        },
        create::handle_create_command_with_client,
        posts::handle_posts_command_with_client,
        token_create::handle_token_create_command_with_client,
        user_create::handle_user_create_command_with_client,
    },
    AppConfig,
};

mod cli;

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    // Load configuration from .env file and environment variables
    let config = AppConfig::load();

    // Create a RedditClient with the loaded configuration
    // This will be passed to all operation handlers to ensure
    // consistent configuration and credentials
    let client = config.create_client();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Posts {
            count,
            subreddit,
            brief,
        } => handle_posts_command_with_client(count, subreddit, brief, client.clone()).await,

        Commands::Create {
            subreddit,
            title,
            text,
        } => {
            // Use the properly configured client that already has the credentials
            handle_create_command_with_client(subreddit, title, text, client.clone()).await
        }

        Commands::UserCreate {
            subreddit,
            title,
            text,
        } => {
            // Use the fully configured client
            handle_user_create_command_with_client(subreddit, title, text, client.clone()).await
        }

        Commands::BrowserCreate {
            subreddit,
            title,
            text,
            port,
        } => {
            // Use port from CLI or config, with fully configured client
            let port_value = config.oauth_port.or(port);

            handle_browser_create_command_with_client(
                subreddit,
                title,
                text,
                port_value,
                client.clone(),
            )
            .await
        }

        Commands::TokenCreate {
            subreddit,
            title,
            text,
            expires_in,
        } => {
            // Use the fully configured client with expires_in from CLI or default
            handle_token_create_command_with_client(
                subreddit,
                title,
                text,
                expires_in,
                client.clone(),
            )
            .await
        }

        Commands::ApiCreate {
            subreddit,
            title,
            text,
        } => {
            // Use the fully configured client
            handle_api_create_command_with_client(subreddit, title, text, client.clone()).await
        }

        Commands::Comment { thing_id, text } => {
            // Use the fully configured client
            handle_comment_command_with_client(thing_id, text, client.clone()).await
        }

        Commands::BrowserComment {
            thing_id,
            text,
            port,
        } => {
            // Use port from CLI or config with fully configured client
            let port_value = config.oauth_port.or(port);

            handle_browser_comment_command_with_client(thing_id, text, port_value, client.clone())
                .await
        }

        Commands::UserComment { thing_id, text } => {
            // Use the fully configured client
            handle_user_comment_command_with_client(thing_id, text, client.clone()).await
        }
    };

    if let Err(err) = result {
        error!("Command execution failed: {:?}", err);
        std::process::exit(1);
    }
}
