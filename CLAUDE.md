# RedRust Development Guidelines

## Building and Testing

```bash
# Build the library
cargo build

# Run tests
cargo test

# Build with headless support
cargo build --features headless

# Check formatting
cargo fmt -- --check

# Run linting
cargo clippy
```

## Development Environment

### Setting up the development environment

1. Install Rust and Cargo using rustup:
   ```
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Clone the repository:
   ```
   git clone https://github.com/yourusername/redrust.git
   cd redrust
   ```

3. For headless browsing testing, start the Docker container:
   ```
   docker-compose up -d
   ```

### Creating a Reddit App for Testing

1. Go to https://www.reddit.com/prefs/apps
2. Click "create another app..." at the bottom
3. Select app type:
   - For browser/headless OAuth: choose "installed app"
   - For script authentication: choose "script"
4. Fill in name and description
5. For redirect URI, use http://localhost:8080
6. After creation, note your client ID (under the app name) and secret if applicable

## Testing Flow

### Test the SetupTokens command

```bash
cargo run -- setup-tokens --client-id YOUR_CLIENT_ID
```

This will authenticate through your browser and store tokens in ~/.redrust/YOUR_CLIENT_ID.json.

### Test creating posts with stored tokens

```bash
cargo run -- browser-create --subreddit testingground4bots --title "Test Post" --text "This is a test post from RedRust" --client-id YOUR_CLIENT_ID
```

### Test headless browser authentication (requires Docker)

```bash
# Start Selenium Chrome container
docker-compose up -d

# Run with headless feature
cargo run --features headless -- headless-create --subreddit testingground4bots --title "Headless Test" --text "This is a test from headless browser" --client-id YOUR_CLIENT_ID
```

### Debug headless issues

If headless browser authentication fails:

1. Check if the container is running:
   ```
   docker ps
   ```

2. Connect to the VNC interface to see the browser:
   ```
   # Open in browser: http://localhost:7900
   # Password: secret
   ```

3. Look for logs with the element selectors and page content:
   ```
   RUST_LOG=debug cargo run --features headless -- headless-create [options]
   ```

## Headless Environment Workflow

The recommended workflow for headless environments is:

1. Set up tokens on a development machine:
   ```
   cargo run -- setup-tokens --client-id YOUR_CLIENT_ID
   ```

2. Copy the ~/.redrust/YOUR_CLIENT_ID.json file to the same location on the headless machine.

3. Run any command that requires authentication - it will use the stored tokens.

## Common Issues and Solutions

### "USER_REQUIRED" Error

This occurs when trying to post with application-only authentication. Solution:
- Use browser-create, user-create, or token-create instead.

### "access_denied" Error from Reddit

This typically means the Reddit app permissions are not correctly set up. Solution:
- Ensure you created the correct app type (installed app or script).
- For browser authentication, make sure the redirect URI is http://localhost:8080.

### Headless Browser Authentication Failures

If the headless browser cannot complete the OAuth flow:
- Use the setup-tokens approach to obtain tokens on a development machine.
- Copy the stored tokens to the headless environment.
- Use the browser-create or token-create commands which will automatically use stored tokens.

## Release Process

1. Update version in Cargo.toml
2. Update CHANGELOG.md
3. Run tests: `cargo test`
4. Commit changes: `git commit -am "Release vX.Y.Z"`
5. Create a tag: `git tag vX.Y.Z`
6. Push changes: `git push && git push --tags`
7. Publish to crates.io: `cargo publish`

## Current Development Context

### Reddit OAuth Issues

We're currently fixing issues with Reddit's Google OAuth login flow:

1. **Problem**: The headless browser can click the Google OAuth button but fails to detect the Google login form that appears.

2. **Root cause**: 
   - Reddit uses modern web components with shadow DOM
   - The login form loads dynamically and may be in a popup or iframe
   - Selenium has difficulty finding elements in these structures

3. **Implementation details**:
   - Added a specialized `handle_google_login` helper method that uses multiple strategies
   - Using JavaScript to traverse shadow DOM and find elements
   - Added iframe detection and interaction
   - Implemented better wait strategies for dynamic content
   - Added JavaScript fallback approaches when standard Selenium fails

4. **Testing approach**:
   - Test with Docker Selenium container: `docker compose up -d`
   - VNC debugging available at http://localhost:7900 (password: secret)
   - Run with headless feature: `cargo run --features headless -- headless-create [options]`

5. **Code structure**:
   - Main login logic in `authenticate_with_headless_browser` in `src/client/mod.rs`
   - New helper method `handle_google_login` added for Google-specific login

### Key fixes implemented:

1. Enhanced Google OAuth button detection
2. Added JavaScript-based DOM traversal for shadow DOM
3. Added iframe support for handling Google authentication popup
4. Improved error handling and logging
5. Added adaptive wait strategies based on page state
6. Added fallback to direct Reddit login when available

### Current Progress (2025-03-22)

We're currently in the middle of implementing several key fixes:

1. **Added a specialized Google login helper method**: We've added a `handle_google_login` method to the `RedditClient` to handle various Google login scenarios.

2. **Compilation issues being fixed**: We're in the process of fixing compilation errors after adding the new Google login handler. These include:
   - Adding proper imports (`use thirtyfour::{WebDriver, By};`)
   - Fixing URL type handling (need to call `.to_string()` before using `.contains()`)
   - Fixing indentation/bracket matching in the code

3. **Last attempted command**: `cargo build --features headless` which was showing compilation errors we were fixing.

4. **Next immediate steps**:
   - Fix remaining compilation errors
   - Test the updated code with the actual Reddit login flow
   - Verify that the Google login detection works correctly

### Next steps:

1. Complete and test remaining fixes
2. Ensure OAuth flow works end-to-end with Google authentication
3. Consider other OAuth providers (like Apple, Facebook) if needed
4. Document approach in comments for future maintenance