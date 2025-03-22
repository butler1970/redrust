# RedRust - Reddit API Client Library for Rust

A Rust wrapper for the Reddit API, providing functionality for both reading and posting to Reddit.

## Features

- Fetch posts from subreddits
- Create posts in subreddits
- Multiple authentication methods:
  - Application-only (read-only)
  - Password flow (for accounts with username/password)
  - OAuth browser flow (for all accounts, including Google OAuth)
  - Headless browser OAuth (for automated environments)
  - Token-based authentication (for headless environments)
  - Script app credentials (for all accounts)
- Token persistence for reduced re-authentication
- Support for headless environments with browser automation

## Installation

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
redrust = "0.1.0"
```

For headless browser support (optional):

```toml
[dependencies]
redrust = { version = "0.1.0", features = ["headless"] }
```

## Usage

### Setup OAuth Tokens (Recommended for Headless Environments)

The best approach for headless environments is to set up tokens once on a machine with a browser, 
then copy those tokens to your headless environment:

```bash
# On a machine with a web browser
redrust setup-tokens --client-id YOUR_CLIENT_ID

# This will:
# 1. Open a browser for you to authorize with Reddit
# 2. Store tokens in ~/.redrust/YOUR_CLIENT_ID.json
# 3. Provide instructions for using these tokens in headless environments
```

Then copy the generated `~/.redrust/YOUR_CLIENT_ID.json` file to the same location on your headless machine.

### Fetching Posts

```bash
redrust posts --subreddit rust --count 10
```

### Creating Posts (Different Auth Methods)

#### Application-Only Authentication (Read-Only, Not Recommended for Posting)

```bash
redrust create --subreddit rust --title "Hello from Rust" --text "This is a test post" --client-id YOUR_CLIENT_ID
```

Note: This will fail with a USER_REQUIRED error since app-only auth can't create posts.

#### Using Browser-Based OAuth (Works with All Account Types)

```bash
redrust browser-create --subreddit rust --title "Hello from Rust" --text "This is a test post" --client-id YOUR_CLIENT_ID
```

This opens a browser window for authentication.

#### Using Username/Password (Not for Google OAuth Accounts)

```bash
redrust user-create --subreddit rust --title "Hello from Rust" --text "This is a test post" --client-id YOUR_CLIENT_ID --username YOUR_USERNAME --password YOUR_PASSWORD
```

#### Using Pre-obtained Tokens (Good for Headless Environments)

After using the `setup-tokens` command:

```bash
redrust token-create --subreddit rust --title "Hello from Rust" --text "This is a test post" --client-id YOUR_CLIENT_ID --access-token YOUR_ACCESS_TOKEN --refresh-token YOUR_REFRESH_TOKEN
```

#### Using Script App Credentials (Works with All Account Types)

```bash
redrust api-create --subreddit rust --title "Hello from Rust" --text "This is a test post" --client-id YOUR_CLIENT_ID --client-secret YOUR_CLIENT_SECRET --username YOUR_USERNAME --password YOUR_PASSWORD
```

#### Using Headless Browser Automation (Experimental)

Requires the `headless` feature and Selenium ChromeDriver:

```bash
redrust headless-create --subreddit rust --title "Hello from Rust" --text "This is a test post" --client-id YOUR_CLIENT_ID
```

## Headless Environment Setup

For truly headless environments, the recommended workflow is:

1. Run `redrust setup-tokens --client-id YOUR_CLIENT_ID` on a machine with a browser
2. Copy the generated `~/.redrust/YOUR_CLIENT_ID.json` file to your headless environment
3. Use any RedRust command that requires authentication; it will automatically use the stored tokens

The stored tokens include refresh tokens and will be automatically renewed when needed.

## Docker Support for Headless Browsing

For browser automation in Docker containers, a `docker-compose.yml` file is provided:

```yaml
version: '3'
services:
  chrome:
    image: selenium/standalone-chrome:latest
    container_name: selenium-chrome
    ports:
      - "4444:4444"  # WebDriver
      - "7900:7900"  # VNC viewer (password: secret)
    environment:
      - SE_VNC_NO_PASSWORD=1
      - SE_NODE_MAX_SESSIONS=2
    volumes:
      - /dev/shm:/dev/shm
```

Run with:

```bash
docker-compose up -d
```

Then connect to VNC on port 7900 (password: secret) to see the browser interactions.

## Authentication Types Explained

RedRust supports multiple authentication methods for different use cases:

1. **App-Only Authentication**: For reading public data only; cannot post.
2. **Password Flow**: Works with Reddit accounts that use username/password login.
3. **OAuth Browser Flow**: Works with all accounts, opens a browser for login.
4. **Stored Token Authentication**: Reuses previously obtained tokens; good for automation.
5. **Headless Browser OAuth**: Uses Selenium to automate the browser authentication flow.
6. **Script App Authentication**: Works with all accounts but requires a script-type app.

## Creating a Reddit App

1. Go to https://www.reddit.com/prefs/apps
2. Click "create another app..." at the bottom
3. Select app type:
   - For browser/headless OAuth: choose "installed app"
   - For script authentication: choose "script"
4. Fill in name and description
5. For redirect URI, use http://localhost:8080
6. After creation, note your client ID (under the app name) and secret if applicable

## License

[MIT License](LICENSE)