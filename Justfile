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
    cargo run -- posts --count {{count}} $([ -n "{{subreddit}}" ] && echo "--subreddit {{subreddit}}") $([ "{{brief}}" = "true" ] && echo "--brief")

# Fetch posts with named parameters
posts-named:
    #!/usr/bin/env bash
    cargo run -- posts --count {{count}} $([ -n "{{subreddit}}" ] && echo "--subreddit {{subreddit}}") $([ "{{brief}}" = "true" ] && echo "--brief")

# Create a post with application-only authentication
create subreddit title text:
    cargo run -- create "{{subreddit}}" "{{title}}" "{{text}}"

# Create a post with application-only authentication using named parameters
create-named:
    cargo run -- create "{{subreddit}}" "{{title}}" "{{text}}"

# Create a post with user authentication (username/password)
user-create subreddit title text:
    cargo run -- user-create "{{subreddit}}" "{{title}}" "{{text}}"

# Create a post with user authentication using named parameters
user-create-named:
    cargo run -- user-create "{{subreddit}}" "{{title}}" "{{text}}"

# Create a post with browser-based OAuth authentication
browser-create subreddit title text port='':
    #!/usr/bin/env bash
    cargo run -- browser-create "{{subreddit}}" "{{title}}" "{{text}}" $([ -n "{{port}}" ] && echo "--port {{port}}")

# Create a post with browser-based OAuth authentication using named parameters
browser-create-named:
    #!/usr/bin/env bash
    cargo run -- browser-create "{{subreddit}}" "{{title}}" "{{text}}" $([ -n "{{port}}" ] && echo "--port {{port}}")

# Create a post with manual OAuth tokens
token-create subreddit title text expires_in='3600':
    #!/usr/bin/env bash
    cargo run -- token-create "{{subreddit}}" "{{title}}" "{{text}}" --expires-in {{expires_in}}

# Create a post with manual OAuth tokens using named parameters
token-create-named:
    #!/usr/bin/env bash
    cargo run -- token-create "{{subreddit}}" "{{title}}" "{{text}}" --expires-in {{expires_in}}

# Create a post with script application API credentials
api-create subreddit title text:
    cargo run -- api-create "{{subreddit}}" "{{title}}" "{{text}}"

# Create a post with script application API credentials using named parameters
api-create-named:
    cargo run -- api-create "{{subreddit}}" "{{title}}" "{{text}}"

# Create a comment on a post or another comment (basic method)
comment thing_id text:
    #!/usr/bin/env bash
    # Disable history expansion to prevent ! interpretation
    set +H
    cargo run -- comment "{{thing_id}}" "{{text}}"

# Create a comment using named parameters
comment-named:
    #!/usr/bin/env bash
    # Disable history expansion to prevent ! interpretation
    set +H
    cargo run -- comment "{{thing_id}}" "{{text}}"

# Create a comment with browser-based OAuth authentication
browser-comment thing_id text port='':
    #!/usr/bin/env bash
    # Disable history expansion to prevent ! interpretation
    set +H
    cargo run -- browser-comment "{{thing_id}}" "{{text}}" $([ -n "{{port}}" ] && echo "--port {{port}}")

# Create a comment with browser-based OAuth authentication using named parameters
browser-comment-named:
    #!/usr/bin/env bash
    # Disable history expansion to prevent ! interpretation
    set +H
    cargo run -- browser-comment "{{thing_id}}" "{{text}}" $([ -n "{{port}}" ] && echo "--port {{port}}")

# Create a comment with user authentication (username/password)
user-comment thing_id text:
    #!/usr/bin/env bash
    # Disable history expansion to prevent ! interpretation
    set +H
    cargo run -- user-comment "{{thing_id}}" "{{text}}"

# Create a comment with user authentication using named parameters
user-comment-named:
    #!/usr/bin/env bash
    # Disable history expansion to prevent ! interpretation
    set +H
    cargo run -- user-comment "{{thing_id}}" "{{text}}"

# Set default values for named parameter commands
count := "10"
subreddit := ""
brief := "false"
title := ""
text := ""
thing_id := ""
port := ""
expires_in := "3600"

# Examples:
# Get 5 posts from r/redrust in brief format
example-posts-redrust:
    just posts 5 rust true

# Get 10 posts from Reddit frontpage in detailed format
example-posts-frontpage:
    just posts 10

# Example of using named parameters
example-named-parameters:
    just count=5 subreddit=rust brief=true posts-named
    
# Example of creating post with browser auth using named parameters
example-browser-create-named:
    just subreddit=redrust title="Testing Named Parameters" text="This post was created using Justs named parameters" browser-create-named

# Example of creating a comment with browser auth
example-browser-comment:
    just thing_id=t3_POSTID text="This is a test comment" browser-comment-named

# Get posts and then comment workflow - this will list posts and inform how to comment on them
example-workflow:
    #!/usr/bin/env bash
    echo "First, let's list some posts to find one to comment on:"
    just posts 5 redrust true
    echo ""
    echo "Now copy a thing_id from above and use it to comment with:"
    echo "just thing_id=t3_POST_ID text=\"Your comment text\" browser-comment-named"