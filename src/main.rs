use crate::cli::{Cli, Commands};
use clap::Parser;
use log::error;
use redrust::operations::{
    api_create::handle_api_create_command,
    browser_create::handle_browser_create_command,
    comment::{
        handle_browser_comment_command, handle_comment_command, handle_user_comment_command,
    },
    create::handle_create_command,
    posts::handle_posts_command,
    token_create::handle_token_create_command,
    user_create::handle_user_create_command,
};

mod cli;

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Posts {
            count,
            subreddit,
            brief,
        } => handle_posts_command(count, subreddit, brief).await,

        Commands::Create {
            subreddit,
            title,
            text,
            client_id,
        } => handle_create_command(subreddit, title, text, client_id).await,

        Commands::UserCreate {
            subreddit,
            title,
            text,
            client_id,
            username,
            password,
        } => {
            handle_user_create_command(subreddit, title, text, client_id, username, password).await
        }

        Commands::BrowserCreate {
            subreddit,
            title,
            text,
            client_id,
            port,
        } => handle_browser_create_command(subreddit, title, text, client_id, port).await,

        Commands::TokenCreate {
            subreddit,
            title,
            text,
            client_id,
            access_token,
            refresh_token,
            expires_in,
        } => {
            handle_token_create_command(
                subreddit,
                title,
                text,
                client_id,
                access_token,
                refresh_token,
                expires_in,
            )
            .await
        }

        Commands::ApiCreate {
            subreddit,
            title,
            text,
            client_id,
            client_secret,
            username,
            password,
        } => {
            handle_api_create_command(
                subreddit,
                title,
                text,
                client_id,
                client_secret,
                username,
                password,
            )
            .await
        }

        Commands::Comment {
            thing_id,
            text,
            client_id,
        } => handle_comment_command(thing_id, text, client_id).await,

        Commands::BrowserComment {
            thing_id,
            text,
            client_id,
            port,
        } => handle_browser_comment_command(thing_id, text, client_id, port).await,

        Commands::UserComment {
            thing_id,
            text,
            client_id,
            username,
            password,
        } => handle_user_comment_command(thing_id, text, client_id, username, password).await,
    };

    if let Err(err) = result {
        error!("Command execution failed: {:?}", err);
        std::process::exit(1);
    }
}
