# Justfile for redrust - A Rust wrapper for the Reddit API

# List available commands
@default:
    just --list

# Build the project
build:
    cargo build

# Run the full test suite
test:
    cargo test

# Check the code format
fmt:
    cargo fmt -- --check

# Format the code in place
fmt-fix:
    cargo fmt

# Run clippy linter
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Clean build artifacts
clean:
    cargo clean

# Build and run the application
run *ARGS:
    cargo run -- {{ARGS}}

# Show help
help:
    cargo run -- --help

# Fetch posts from a subreddit or the public frontpage
posts count subreddit='' brief='':
    #!/usr/bin/env bash
    ARGS="posts --count {{count}}"
    if [ -n "{{subreddit}}" ]; then
        ARGS="$ARGS --subreddit {{subreddit}}"
    fi
    if [ "{{brief}}" = "true" ]; then
        ARGS="$ARGS --brief"
    fi
    cargo run -- $ARGS

# Fetch posts with named parameters
posts-named:
    #!/usr/bin/env bash
    ARGS="posts --count {{count}}"
    if [ -n "{{subreddit}}" ]; then
        ARGS="$ARGS --subreddit {{subreddit}}"
    fi
    if [ "{{brief}}" = "true" ]; then
        ARGS="$ARGS --brief"
    fi
    cargo run -- $ARGS

# Create a post with application-only authentication
create subreddit title text client_id:
    cargo run -- create "{{subreddit}}" "{{title}}" "{{text}}" "{{client_id}}"

# Create a post with application-only authentication using named parameters
create-named:
    cargo run -- create "{{subreddit}}" "{{title}}" "{{text}}" "{{client_id}}"

# Create a post with user authentication (username/password)
user-create subreddit title text client_id username password:
    cargo run -- user-create "{{subreddit}}" "{{title}}" "{{text}}" "{{client_id}}" "{{username}}" "{{password}}"

# Create a post with user authentication using named parameters
user-create-named:
    cargo run -- user-create "{{subreddit}}" "{{title}}" "{{text}}" "{{client_id}}" "{{username}}" "{{password}}"

# Create a post with browser-based OAuth authentication
browser-create subreddit title text client_id port='':
    #!/usr/bin/env bash
    ARGS="browser-create \"{{subreddit}}\" \"{{title}}\" \"{{text}}\" \"{{client_id}}\""
    if [ -n "{{port}}" ]; then
        ARGS="$ARGS --port {{port}}"
    fi
    cargo run -- $ARGS

# Create a post with browser-based OAuth authentication using named parameters
browser-create-named:
    #!/usr/bin/env bash
    ARGS="browser-create \"{{subreddit}}\" \"{{title}}\" \"{{text}}\" \"{{client_id}}\""
    if [ -n "{{port}}" ]; then
        ARGS="$ARGS --port {{port}}"
    fi
    cargo run -- $ARGS

# Create a post with manual OAuth tokens
token-create subreddit title text client_id access_token refresh_token='' expires_in='3600':
    #!/usr/bin/env bash
    ARGS="token-create \"{{subreddit}}\" \"{{title}}\" \"{{text}}\" \"{{client_id}}\" \"{{access_token}}\""
    if [ -n "{{refresh_token}}" ]; then
        ARGS="$ARGS \"{{refresh_token}}\""
    fi
    ARGS="$ARGS --expires-in {{expires_in}}"
    cargo run -- $ARGS

# Create a post with manual OAuth tokens using named parameters
token-create-named:
    #!/usr/bin/env bash
    ARGS="token-create \"{{subreddit}}\" \"{{title}}\" \"{{text}}\" \"{{client_id}}\" \"{{access_token}}\""
    if [ -n "{{refresh_token}}" ]; then
        ARGS="$ARGS \"{{refresh_token}}\""
    fi
    ARGS="$ARGS --expires-in {{expires_in}}"
    cargo run -- $ARGS

# Create a post with script application API credentials
api-create subreddit title text client_id client_secret username password:
    cargo run -- api-create "{{subreddit}}" "{{title}}" "{{text}}" "{{client_id}}" "{{client_secret}}" "{{username}}" "{{password}}"

# Create a post with script application API credentials using named parameters
api-create-named:
    cargo run -- api-create "{{subreddit}}" "{{title}}" "{{text}}" "{{client_id}}" "{{client_secret}}" "{{username}}" "{{password}}"

# Set default values for named parameter commands
count := "10"
subreddit := ""
brief := "false"
title := ""
text := ""
client_id := ""
username := ""
password := ""
port := ""
access_token := ""
refresh_token := ""
expires_in := "3600"
client_secret := ""

# Examples:
# Get 5 posts from r/rust in brief format
example-posts-rust:
    just posts 5 rust true

# Get 10 posts from Reddit frontpage in detailed format
example-posts-frontpage:
    just posts 10

# Example of using named parameters
example-named-parameters:
    just count=5 subreddit=rust brief=true posts-named
    
# Example of creating post with browser auth using named parameters
example-browser-create-named:
    just subreddit=rust title="Testing Named Parameters" text="This post was created using Just's named parameters" client_id=YOUR_CLIENT_ID browser-create-named