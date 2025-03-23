# RedRust Project Guide

## Overview

RedRust is a command-line client for posting to Reddit subreddits, with special support for accounts that use Google OAuth login.

## Build/Lint/Test Commands

### Using Cargo

```bash
# Build the project in debug mode
cargo build

# Build the project in release mode
cargo build --release

# Run tests
cargo test

# Run linting with Clippy
cargo clippy -- -D warnings

# Format the code
cargo fmt
```

### Using Just

RedRust includes a Justfile with convenient commands for common operations:

```bash
# List all available commands
just

# Build the project
just build

# Run the tests
just test

# Check code formatting
just fmt

# Fix code formatting
just fmt-fix

# Run clippy linter
just clippy

# Show help
just help

# Fetch posts from a subreddit (positional parameters)
just posts 5 rust true  # Get 5 posts from r/rust in brief format
just posts 10           # Get 10 posts from Reddit frontpage in detailed format

# Fetch posts with named parameters
just count=5 subreddit=rust brief=true posts-named

# Create posts with different authentication methods (positional parameters)
just create subreddit "Post Title" "Post content" client_id
just user-create subreddit "Post Title" "Post content" client_id username password
just browser-create subreddit "Post Title" "Post content" client_id
just token-create subreddit "Post Title" "Post content" client_id access_token refresh_token
just api-create subreddit "Post Title" "Post content" client_id client_secret username password

# Create posts with named parameters (more readable for complex commands)
just subreddit=rust \
  title="Post Title" \
  text="Post content" \
  client_id=YOUR_CLIENT_ID \
  create-named

just subreddit=rust \
  title="Post Title" \
  text="Post content" \
  client_id=YOUR_CLIENT_ID \
  port=8888 \
  browser-create-named
```

## Code Style Guidelines

This project follows the standard Rust style guidelines:
- Use 4 spaces for indentation
- Follow the official Rust naming conventions (snake_case for variables and functions, CamelCase for types)
- Keep functions focused on a single responsibility
- Document public API functions with doc comments
- Use Result for error handling with appropriate error propagation
- Limit line length to approximately 100 characters

## Project Structure

- `src/main.rs` - Main entry point for the CLI application
- `src/cli.rs` - CLI argument parsing with clap
- `src/lib.rs` - Library interfaces and re-exports
- `src/client/mod.rs` - Reddit client implementation with authentication methods
- `src/models/` - Data structures for Reddit API responses
  - `mod.rs` - Common model definitions
  - `public_feed.rs` - Models for the public feed
  - `subreddit_posts.rs` - Models for subreddit posts
- `src/operations/` - Operation modules for each command
  - `posts.rs` - Fetching posts from Reddit
  - `create.rs` - Creating posts with application-only auth
  - `user_create.rs` - Creating posts with user credentials
  - `browser_create.rs` - Creating posts with browser-based auth
  - `token_create.rs` - Creating posts with manual tokens
  - `api_create.rs` - Creating posts with script API credentials

## Authentication Methods

This project supports multiple authentication methods for the Reddit API:

1. **App-only Authentication** (no user context, read-only)
2. **Username/Password Authentication** (for standard Reddit accounts)
3. **Browser-based OAuth** (for any account, including Google OAuth)
4. **Script-app Authentication** (using Reddit API credentials)
5. **Manual Token Authentication** (for headless environments)

The recommended approach for posting is to use the Browser-based OAuth method with token persistence, which works with any Reddit account including those using Google OAuth.

## Token Storage

The application stores authentication tokens in the user's home directory at `~/.redrust/` to avoid requiring login for each use. Refresh tokens are used to automatically renew access when needed.

## Headless Environments

For headless environments without browser access, you have two options:

1. **Use the TokenCreate command**: Manually obtain tokens from Reddit elsewhere and provide them directly to the application. This is useful for server environments or automated scripts.

```bash
# Using cargo
cargo run -- token-create --subreddit mysubreddit --title "My Post Title" --text "Post content here" --client-id YOUR_CLIENT_ID --access-token YOUR_ACCESS_TOKEN --refresh-token YOUR_REFRESH_TOKEN

# Using just
just token-create mysubreddit "My Post Title" "Post content here" YOUR_CLIENT_ID YOUR_ACCESS_TOKEN YOUR_REFRESH_TOKEN
```

2. **Transfer token files**: After authenticating on a machine with a browser, copy the token files from `~/.redrust/` to the headless environment.

### Workflow for Headless Environments

Here's a typical workflow for using RedRust in headless environments:

1. **Initial Setup (on a machine with browser access)**:
   ```bash
   # Using cargo
   cargo run -- browser-create --subreddit test --title "Test Post" --text "Test content" --client-id YOUR_CLIENT_ID
   
   # Using just
   just browser-create test "Test Post" "Test content" YOUR_CLIENT_ID
   ```
   This will create and store tokens in `~/.redrust/YOUR_CLIENT_ID.json`.

2. **Transfer tokens to headless environment**:
   ```bash
   # Copy the token file to the headless machine
   scp ~/.redrust/YOUR_CLIENT_ID.json user@headless-server:~/.redrust/
   ```

3. **Use on headless environment**:
   ```bash
   # Using cargo (tokens will be automatically loaded and refreshed)
   cargo run -- browser-create --subreddit mysubreddit --title "Headless Post" --text "This was posted from a headless environment" --client-id YOUR_CLIENT_ID
   
   # Using just
   just browser-create mysubreddit "Headless Post" "This was posted from a headless environment" YOUR_CLIENT_ID
   ```

4. **If token transfer isn't possible, extract tokens and use the token-create command**:
   ```bash
   # Examine the token file to get the values
   cat ~/.redrust/YOUR_CLIENT_ID.json
   
   # Using cargo
   cargo run -- token-create --subreddit mysubreddit --title "Headless Post" --text "This was posted using extracted tokens" --client-id YOUR_CLIENT_ID --access-token YOUR_ACCESS_TOKEN --refresh-token YOUR_REFRESH_TOKEN
   
   # Using just
   just token-create mysubreddit "Headless Post" "This was posted using extracted tokens" YOUR_CLIENT_ID YOUR_ACCESS_TOKEN YOUR_REFRESH_TOKEN
   ```