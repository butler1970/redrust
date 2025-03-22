use reqwest::{Client, Error as ReqwestError};
use crate::RedditRNewResponse;
use log::{debug, info};
use std::collections::HashMap;
use std::fmt;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use rand::{Rng, distributions::Alphanumeric};
use tiny_http::{Server, Response, StatusCode};
use webbrowser;
use url::Url;

// Define a custom error type for handling Reddit API errors
#[derive(Debug)]
pub enum RedditClientError {
    RequestError(ReqwestError),
    ApiError(String),
    ParseError(serde_json::Error),
}

impl fmt::Display for RedditClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RedditClientError::RequestError(err) => write!(f, "Request error: {}", err),
            RedditClientError::ApiError(msg) => write!(f, "Reddit API error: {}", msg),
            RedditClientError::ParseError(err) => write!(f, "Parse error: {}", err),
        }
    }
}

impl std::error::Error for RedditClientError {}

impl From<ReqwestError> for RedditClientError {
    fn from(err: ReqwestError) -> Self {
        RedditClientError::RequestError(err)
    }
}

impl From<serde_json::Error> for RedditClientError {
    fn from(err: serde_json::Error) -> Self {
        RedditClientError::ParseError(err)
    }
}

#[cfg(feature = "headless")]
impl From<thirtyfour::error::WebDriverError> for RedditClientError {
    fn from(err: thirtyfour::error::WebDriverError) -> Self {
        RedditClientError::ApiError(format!("WebDriver error: {}", err))
    }
}

#[cfg(feature = "headless")]
use thirtyfour::{WebDriver, By};

/// Structure to store OAuth tokens and credentials
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenStorage {
    pub client_id: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<u64>,
    pub last_updated: u64,
}

impl TokenStorage {
    pub fn new(client_id: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            access_token: None,
            refresh_token: None,
            token_expires_at: None,
            last_updated: chrono::Utc::now().timestamp() as u64,
        }
    }
    
    pub fn is_access_token_valid(&self) -> bool {
        match (self.access_token.as_ref(), self.token_expires_at) {
            (Some(_), Some(expiry)) => {
                let now = chrono::Utc::now().timestamp() as u64;
                // Add a 5-minute buffer to avoid edge cases
                now + 300 < expiry
            }
            _ => false,
        }
    }
    
    pub fn has_refresh_token(&self) -> bool {
        self.refresh_token.is_some()
    }
}

#[derive(Clone)]
pub struct RedditClient {
    pub client: Client,
    pub access_token: Option<String>,
    pub user_agent: String,
    pub token_storage: Option<TokenStorage>,
}

impl RedditClient {
    pub fn new() -> Self {
        let user_agent = format!("redrust/1.0 (by /u/Aggravating-Fix-3871)");
        Self {
            client: Self::get_client(&user_agent).unwrap(),
            access_token: None,
            user_agent,
            token_storage: None,
        }
    }
    
    // Helper function to print manual testing info
    #[cfg(feature = "headless")]
    fn print_manual_testing_info(auth_url: &str, state: &str, redirect_uri: &str) {
        info!("======== MANUAL TESTING INFO ========");
        info!("Full authorization URL: {}", auth_url);
        info!("State token: {}", state);
        info!("Callback URL: {}", redirect_uri);
        info!("======================================");
    }
    
    #[cfg(feature = "headless")]
    /// Advanced Google login helper for detecting and interacting with Google OAuth forms
    async fn handle_google_login(&self, driver: &WebDriver, email: &str, password: &str) -> Result<bool, RedditClientError> {
        // 1. First check if we're actually on a Google login page
        let current_url = match driver.current_url().await {
            Ok(url) => url.to_string(),
            Err(_) => String::new()
        };
        
        let _is_google_page = current_url.contains("accounts.google.com") || 
                             current_url.contains("google.com/signin");
        
        info!("Checking for Google login page at URL: {}", current_url);
        
        // 2. Try to use JavaScript to detect Google login elements
        let page_check = driver.execute(r#"
            // Check if we're on a Google login page
            const isGooglePage = document.title.includes('Google') || 
                                window.location.href.includes('google');
                                
            // Try to find common Google login elements
            const hasEmailField = !!document.querySelector('input[type="email"], #identifierId, input[name*="email"], input[name*="identifier"]');
            const hasGoogleBranding = !!document.querySelector('img[alt*="Google"], div[aria-label*="Google"]');
            
            return JSON.stringify({
                isGooglePage,
                hasEmailField,
                hasGoogleBranding,
                title: document.title,
                bodyText: document.body.textContent.substring(0, 100) // Get a sample of the text
            });
        "#, vec![]).await;
        
        if let Ok(result) = page_check {
            info!("Google page check result: {:?}", result);
        }
        
        // 3. Try different strategies to find and enter the email
        
        // Strategy 1: Standard Selenium selectors for email field
        let email_field = match driver.find(By::XPath("//input[@type='email']")).await {
            Ok(field) => {
                info!("Found Google email field by type");
                Some(field)
            },
            Err(_) => match driver.find(By::Id("identifierId")).await {
                Ok(field) => {
                    info!("Found Google email field by ID");
                    Some(field)
                },
                Err(_) => match driver.find(By::XPath("//input[contains(@name, 'email') or contains(@name, 'identifier')]")).await {
                    Ok(field) => {
                        info!("Found Google email field by name");
                        Some(field)
                    },
                    Err(_) => match driver.find(By::Css("input:not([type='hidden']):not([type='checkbox']):not([type='radio'])")).await {
                        Ok(field) => {
                            info!("Found generic input field as fallback");
                            Some(field)
                        },
                        Err(_) => None
                    }
                }
            }
        };
        
        // If we found an email field, enter credentials
        if let Some(field) = email_field {
            info!("Found email field, entering email: {}", email);
            field.send_keys(email).await
                .map_err(|e| RedditClientError::ApiError(format!("Failed to enter Google email: {}", e)))?;
            
            // Look for next button with multiple selectors
            let next_button = match driver.find(By::XPath("//button[contains(., 'Next')]")).await {
                Ok(button) => {
                    info!("Found Next button by text");
                    Some(button)
                },
                Err(_) => match driver.find(By::Id("identifierNext")).await {
                    Ok(button) => {
                        info!("Found Next button by ID");
                        Some(button)
                    },
                    Err(_) => match driver.find(By::XPath("//div[@role='button' and contains(., 'Next')]")).await {
                        Ok(button) => {
                            info!("Found Next button by role and text");
                            Some(button)
                        },
                        Err(_) => match driver.find(By::Css("button[type='submit'], input[type='submit']")).await {
                            Ok(button) => {
                                info!("Found submit button");
                                Some(button)
                            },
                            Err(_) => None
                        }
                    }
                }
            };
            
            // Click the next button
            if let Some(button) = next_button {
                info!("Clicking Next button");
                button.click().await
                    .map_err(|e| RedditClientError::ApiError(format!("Failed to click Google Next button: {}", e)))?;
                
                // Wait for password field to appear
                tokio::time::sleep(Duration::from_secs(5)).await;
                
                // Look for password field
                let password_field = match driver.find(By::XPath("//input[@type='password']")).await {
                    Ok(field) => {
                        info!("Found password field by type");
                        Some(field)
                    },
                    Err(_) => match driver.find(By::Name("password")).await {
                        Ok(field) => {
                            info!("Found password field by name");
                            Some(field)
                        },
                        Err(_) => match driver.find(By::Css("input:not([type='hidden'])")).await {
                            Ok(field) => {
                                info!("Found generic input field for password");
                                Some(field)
                            },
                            Err(_) => None
                        }
                    }
                };
                
                // Enter password
                if let Some(field) = password_field {
                    info!("Entering Google password");
                    field.send_keys(password).await
                        .map_err(|e| RedditClientError::ApiError(format!("Failed to enter Google password: {}", e)))?;
                    
                    // Look for sign in button
                    let signin_button = match driver.find(By::XPath("//button[contains(., 'Next') or contains(., 'Sign in')]")).await {
                        Ok(button) => {
                            info!("Found Sign in button by text");
                            Some(button)
                        },
                        Err(_) => match driver.find(By::Id("passwordNext")).await {
                            Ok(button) => {
                                info!("Found Sign in button by ID");
                                Some(button)
                            },
                            Err(_) => match driver.find(By::Css("button[type='submit'], input[type='submit']")).await {
                                Ok(button) => {
                                    info!("Found submit button for sign in");
                                    Some(button)
                                },
                                Err(_) => None
                            }
                        }
                    };
                    
                    // Click the sign in button
                    if let Some(button) = signin_button {
                        info!("Clicking Sign in button");
                        button.click().await
                            .map_err(|e| RedditClientError::ApiError(format!("Failed to click Google Sign in button: {}", e)))?;
                        
                        // Wait for authentication to complete
                        info!("Waiting for Google authentication to complete");
                        tokio::time::sleep(Duration::from_secs(10)).await;
                        return Ok(true);
                    } else {
                        info!("Could not find Sign in button");
                    }
                } else {
                    info!("Could not find password field");
                }
            } else {
                info!("Could not find Next button");
            }
        } else {
            info!("Could not find email field with normal methods, trying JavaScript");
            
            // Strategy 2: Use JavaScript to find and interact with the form
            let js_result = driver.execute(&format!(r#"
                try {{
                    // First check if we're on a Google page
                    const isGooglePage = document.title.includes('Google') || 
                                        window.location.href.includes('google');
                    
                    if (!isGooglePage) {{
                        return "Not on a Google page";
                    }}
                    
                    // Try to find email input
                    const emailInput = document.querySelector('input[type="email"]') || 
                                      document.getElementById('identifierId') ||
                                      document.querySelector('input[name*="email"]') ||
                                      document.querySelector('input[name*="identifier"]') ||
                                      document.querySelector('input:not([type="hidden"]):not([type="checkbox"]):not([type="radio"])');
                    
                    if (emailInput) {{
                        // Enter email
                        emailInput.value = "{}";
                        emailInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        emailInput.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        
                        // Find next button
                        const nextButton = document.querySelector('#identifierNext') ||
                                          document.querySelector('button[type="submit"]') ||
                                          Array.from(document.querySelectorAll('button')).find(b => b.textContent.includes('Next')) ||
                                          document.querySelector('div[role="button"]');
                        
                        if (nextButton) {{
                            nextButton.click();
                            return "Entered email and clicked next via JavaScript";
                        }}
                        
                        return "Entered email but couldn't find next button";
                    }}
                    
                    return "Could not find email input via JavaScript";
                }} catch (e) {{
                    return "JavaScript error: " + e.message;
                }}
            "#, email), vec![]).await;
            
            if let Ok(result) = js_result {
                info!("JavaScript Google login result: {:?}", result);
                
                if format!("{:?}", result).contains("clicked next") {
                    // Wait for password field
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    
                    // Try to enter password with JavaScript
                    let js_password = driver.execute(&format!(r#"
                        try {{
                            // Find password input
                            const passwordInput = document.querySelector('input[type="password"]') ||
                                                document.querySelector('input[name*="password"]') ||
                                                document.querySelector('input:not([type="hidden"]):not([type="email"]):not([type="checkbox"]):not([type="radio"])');
                            
                            if (passwordInput) {{
                                // Enter password
                                passwordInput.value = "{}";
                                passwordInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                                passwordInput.dispatchEvent(new Event('change', {{ bubbles: true }}));
                                
                                // Find sign in button
                                const signInButton = document.querySelector('#passwordNext') ||
                                                   document.querySelector('button[type="submit"]') ||
                                                   Array.from(document.querySelectorAll('button')).find(b => 
                                                       b.textContent.includes('Sign in') || b.textContent.includes('Next')
                                                   ) ||
                                                   document.querySelector('div[role="button"]');
                                
                                if (signInButton) {{
                                    signInButton.click();
                                    return "Entered password and clicked sign in via JavaScript";
                                }}
                                
                                return "Entered password but couldn't find sign in button";
                            }}
                            
                            return "Could not find password input via JavaScript";
                        }} catch (e) {{
                            return "JavaScript password error: " + e.message;
                        }}
                    "#, password), vec![]).await;
                    
                    if let Ok(result) = js_password {
                        info!("JavaScript password entry result: {:?}", result);
                        
                        if format!("{:?}", result).contains("clicked sign in") {
                            // Wait for authentication to complete
                            info!("Waiting for Google authentication to complete after JavaScript login");
                            tokio::time::sleep(Duration::from_secs(10)).await;
                            return Ok(true);
                        }
                    }
                }
            }
        }
        
        // Check if we're now at a new URL (might have signed in already)
        let new_url = match driver.current_url().await {
            Ok(url) => url.to_string(),
            Err(_) => String::new()
        };
        
        if new_url != current_url {
            info!("URL changed from {} to {} during Google login", current_url, new_url);
            return Ok(true);
        }
        
        info!("Google login handling complete, but couldn't confirm successful authentication");
        Ok(false)
    }
    
    #[cfg(feature = "headless")]
    /// Authenticate using a headless browser (Selenium) - supports Google OAuth
    pub async fn authenticate_with_headless_browser(
        &mut self, 
        client_id: &str, 
        redirect_port: Option<u16>,
        google_email: Option<&str>,
        google_password: Option<&str>,
        scopes: Option<&str>
    ) -> Result<String, RedditClientError> {
        use thirtyfour::{WebDriver, By, DesiredCapabilities};
        
        // Setup parameters
        let port = redirect_port.unwrap_or(8080);
        let scopes = scopes.unwrap_or("identity read submit");
        let redirect_uri = format!("http://localhost:{}/callback", port);
        
        info!("");
        info!("==== IMPORTANT - MANUAL AUTHORIZATION INFO ====");
        info!("If the automated browser doesn't work, you can authorize manually:");
        info!("1. A local server is running on port {} to receive the OAuth callback", port);
        
        // Generate a random state token to prevent CSRF
        let state: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        
        // Create the authorization URL
        let auth_url = format!(
            "https://www.reddit.com/api/v1/authorize?client_id={}&response_type=code&state={}&redirect_uri={}&duration=permanent&scope={}",
            client_id, state, redirect_uri, scopes
        );
        
        // Start the local server to receive the callback
        let server = match Server::http(format!("127.0.0.1:{}", port)) {
            Ok(server) => server,
            Err(e) => return Err(RedditClientError::ApiError(
                format!("Failed to start local server: {}", e)
            )),
        };
        
        // Create a channel to receive the authorization code
        let (tx, rx) = mpsc::channel();
        
        // Clone values for the server thread
        let state_clone = state.clone();
        let tx_clone = tx.clone();
        
        // Start the server in a separate thread
        let server_thread = thread::spawn(move || {
            info!("Waiting for authorization callback on http://localhost:{}/callback", port);
            
            // Set a timeout value - we'll exit the loop after this duration
            let _timeout_duration = Duration::from_secs(300); // 5 minutes
            
            // Handle incoming requests
            for request in server.incoming_requests() {
                let path = request.url();
                
                // Only handle the expected callback path
                if path.starts_with("/callback") {
                    debug!("Received callback: {}", path);
                    
                    // Parse the query parameters
                    let url_str = format!("http://localhost{}", path);
                    let query = match Url::parse(&url_str) {
                        Ok(url) => {
                            // Collect into a HashMap to avoid borrowing issues
                            let pairs: HashMap<String, String> = url.query_pairs()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect();
                            pairs
                        },
                        Err(_) => {
                            // Failed to parse URL, return an error page
                            let response = Response::from_string(
                                "<html><body><h1>Error</h1><p>Invalid callback URL</p></body></html>"
                            ).with_status_code(StatusCode(400));
                            request.respond(response).ok();
                            continue;
                        }
                    };
                    
                    // Check for errors from Reddit
                    if let Some(error) = query.get("error") {
                        tx_clone.send(Err(format!("Authorization error: {}", error))).unwrap();
                        
                        // Return an error page
                        let response = Response::from_string(format!(
                            "<html><body><h1>Authentication Error</h1><p>{}</p></body></html>",
                            error
                        )).with_status_code(StatusCode(400));
                        request.respond(response).ok();
                        break;
                    }
                    
                    // Check for the state parameter
                    match query.get("state") {
                        Some(received_state) if received_state == &state_clone => {
                            // State matches, check for the code
                            if let Some(code) = query.get("code") {
                                tx_clone.send(Ok(code.to_string())).unwrap();
                                
                                // Return a success page
                                let response = Response::from_string(
                                    "<html><body><h1>Authentication Successful</h1><p>You can now close this window and return to the application.</p></body></html>"
                                ).with_status_code(StatusCode(200));
                                request.respond(response).ok();
                                break;
                            } else {
                                tx_clone.send(Err("No authorization code received".to_string())).unwrap();
                                
                                // Return an error page
                                let response = Response::from_string(
                                    "<html><body><h1>Authentication Error</h1><p>No authorization code received</p></body></html>"
                                ).with_status_code(StatusCode(400));
                                request.respond(response).ok();
                                break;
                            }
                        },
                        Some(_) => {
                            tx_clone.send(Err("State mismatch - possible CSRF attack".to_string())).unwrap();
                            
                            // Return an error page
                            let response = Response::from_string(
                                "<html><body><h1>Authentication Error</h1><p>State mismatch - possible CSRF attack</p></body></html>"
                            ).with_status_code(StatusCode(400));
                            request.respond(response).ok();
                            break;
                        },
                        None => {
                            tx_clone.send(Err("No state parameter received".to_string())).unwrap();
                            
                            // Return an error page
                            let response = Response::from_string(
                                "<html><body><h1>Authentication Error</h1><p>No state parameter received</p></body></html>"
                            ).with_status_code(StatusCode(400));
                            request.respond(response).ok();
                            break;
                        }
                    }
                } else {
                    // Not the callback endpoint
                    let response = Response::from_string(
                        "<html><body><h1>404 Not Found</h1></body></html>"
                    ).with_status_code(StatusCode(404));
                    request.respond(response).ok();
                }
            }
        });
        
        // Initialize WebDriver (assuming chromedriver is running)
        let mut caps = DesiredCapabilities::chrome();
        
        // DEBUGGING: Force visible mode for VNC debugging
        info!("DEBUGGING MODE ENABLED: Running browser in visible mode for VNC debugging");
        info!("Connect to http://localhost:7900 with password 'secret' to see the browser");
        
        // Enable cookies to maintain session state across redirects
        caps.add_chrome_arg("--enable-cookies")?;
        
        // Common Chrome arguments to avoid detection
        caps.add_chrome_arg("--disable-extensions")?;
        caps.add_chrome_arg("--no-sandbox")?;
        caps.add_chrome_arg("--disable-blink-features=AutomationControlled")?; // Hide automation
        caps.add_chrome_arg("--disable-dev-shm-usage")?; // Overcome limited /dev/shm in Docker
        caps.add_chrome_arg("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36")?; // Use common user agent
        
        // Force browser to be visible (no headless mode)
        caps.add_chrome_arg("--start-maximized")?;
        caps.add_chrome_arg("--disable-notifications")?; // Block notifications
        
        // Determine if we should use headless mode based on credentials (but override for debugging)
        let fully_headless = google_email.is_some() && google_password.is_some();
        
        // Log connection info
        info!("Connecting to WebDriver at http://localhost:4444");
        info!("Authentication URL: {}", &auth_url);
        info!("Waiting for browser to load...");
        
        // FOR DEBUGGING: Add a longer wait time before starting WebDriver
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // Print the authorization URL for manual use
        info!("Manual authorization URL: {}", &auth_url);
        
        // Connect to WebDriver
        info!("Connecting to WebDriver...");
        let driver = WebDriver::new("http://localhost:4444", caps).await
            .map_err(|e| RedditClientError::ApiError(format!("Failed to connect to WebDriver: {}", e)))?;
        
        // Check if the driver connected successfully
        info!("WebDriver connected successfully");
        
        // Set a longer page load timeout for slower connections
        driver.set_page_load_timeout(Duration::from_secs(30)).await
            .map_err(|e| RedditClientError::ApiError(format!("Failed to set page load timeout: {}", e)))?;
            
        // Verify browser is reachable by navigating to a simple page first
        info!("Testing browser connection with simple navigation...");
        driver.goto("about:blank").await
            .map_err(|e| RedditClientError::ApiError(format!("Failed initial navigation test: {}", e)))?;
            
        // Check if we can execute JavaScript to confirm browser is working
        let _ = driver.execute("return navigator.userAgent;", vec![]).await
            .map_err(|e| RedditClientError::ApiError(format!("Failed to execute JavaScript test: {}", e)))?;
            
        info!("Browser is responsive");
        
        // Wait for VNC to refresh and show the browser
        info!("Browser should now be visible in VNC. Waiting 5 seconds before continuing...");
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Navigate to the authorization URL
        info!("Navigating to Reddit OAuth page...");
        driver.goto(&auth_url).await
            .map_err(|e| RedditClientError::ApiError(format!("Failed to navigate to authorization URL: {}", e)))?;
        
        // Add an explicit wait for a longer time for the page to fully load
        info!("Waiting for the page to fully load and render...");
        tokio::time::sleep(Duration::from_secs(8)).await;
        
        // Log more details about the page for debugging
        let page_title = match driver.title().await {
            Ok(title) => title,
            Err(_) => "Unknown title".to_string(),
        };
        info!("Page loaded with title: {}", page_title);
        
        // Check if we hit a Reddit error page or CAPTCHA
        let page_source = match driver.source().await {
            Ok(source) => {
                info!("Page source length: {} characters", source.len());
                if source.len() < 1000 {
                    debug!("Full page source (likely an error page): {}", source);
                }
                // Check for CAPTCHA with detailed logging
                if source.contains("captcha") || source.contains("CAPTCHA") {
                    info!("Potential CAPTCHA keyword detected on the page");
                    // Log surrounding context to verify if it's actually a CAPTCHA
                    if let Some(captcha_idx) = source.to_lowercase().find("captcha") {
                        let start = captcha_idx.saturating_sub(100);
                        let end = (captcha_idx + 100).min(source.len());
                        let context = &source[start..end];
                        debug!("CAPTCHA context: {}", context);
                    }
                }
                source
            },
            Err(e) => {
                info!("Failed to get page source: {}", e);
                String::new()
            }
        };
        
        // Check if we need to sign in or if we're already at the authorization page
        // Look for the "Continue with Google" button first
        info!("Looking for login options...");
        
        // Check if we're on a login page based on URL and title
        let current_url = match driver.current_url().await {
            Ok(url) => url.to_string(),
            Err(_) => String::new(),
        };
        
        if current_url.contains("/login/") {
            info!("Detected Reddit login page. Attempting to log in...");
            
            // Wait for login page to fully load
            tokio::time::sleep(Duration::from_secs(3)).await;
            
            // Try the Reddit username field
            if let (Some(email), Some(password)) = (google_email, google_password) {
                match driver.find(By::Id("loginUsername")).await {
                    Ok(username_field) => {
                        info!("Found username field, entering email");
                        username_field.send_keys(email).await
                            .map_err(|e| RedditClientError::ApiError(format!("Failed to enter username: {}", e)))?;
                            
                        match driver.find(By::Id("loginPassword")).await {
                            Ok(password_field) => {
                                info!("Found password field, entering password");
                                password_field.send_keys(password).await
                                    .map_err(|e| RedditClientError::ApiError(format!("Failed to enter password: {}", e)))?;
                                    
                                match driver.find(By::XPath("//button[contains(text(), 'Log In')]")).await {
                                    Ok(login_button) => {
                                        info!("Found login button, clicking...");
                                        login_button.click().await
                                            .map_err(|e| RedditClientError::ApiError(format!("Failed to click login button: {}", e)))?;
                                            
                                        info!("Clicked login button, waiting for redirect...");
                                        tokio::time::sleep(Duration::from_secs(8)).await;
                                    },
                                    Err(_) => {
                                        info!("Could not find login button");
                                    }
                                }
                            },
                            Err(_) => {
                                info!("Could not find password field");
                            }
                        }
                    },
                    Err(_) => {
                        info!("Could not find username field, trying Google login");
                        
                        // Try to find Google login button
                        match driver.find(By::XPath("//*[contains(text(), 'Continue with Google')]")).await {
                            Ok(google_button) => {
                                info!("Found Google login button, clicking...");
                                google_button.click().await
                                    .map_err(|e| RedditClientError::ApiError(format!("Failed to click Google button: {}", e)))?;
                                tokio::time::sleep(Duration::from_secs(5)).await;
                            },
                            Err(_) => {
                                info!("No Google button found");
                            }
                        }
                    }
                }
            } else {
                info!("No credentials provided for automated login. Manual intervention required.");
                // If we're in visible mode with VNC, give a longer timeout
                if !fully_headless {
                    info!("Please log in through the VNC interface (http://localhost:7900) with password 'secret'.");
                    tokio::time::sleep(Duration::from_secs(120)).await;
                }
            }
        }

        // Detect if we're being shown a very minimal page (anti-bot measure)
        if page_source.len() < 500 {
            info!("Received a very small page (likely an anti-bot measure). Trying alternative approach...");
            
            // Log full page source
            debug!("Full page source: {}", page_source);
            
            // Try opening the authorization URL again with a different approach
            info!("Trying to navigate to the OAuth URL again with different parameters...");
            
            // Add random state to URL parameters to avoid caching
            let random_suffix: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(8)
                .map(char::from)
                .collect();
                
            let modified_auth_url = format!(
                "{}&t={}", auth_url, random_suffix
            );
            
            info!("Retrying with modified URL: {}", modified_auth_url);
            driver.goto(&modified_auth_url).await
                .map_err(|e| RedditClientError::ApiError(format!("Failed to navigate to modified authorization URL: {}", e)))?;
                
            // Wait longer for the page to load
            tokio::time::sleep(Duration::from_secs(8)).await;
            
            // Check if this approach worked
            let new_page_title = match driver.title().await {
                Ok(title) => title,
                Err(_) => "Unknown title".to_string(),
            };
            info!("After retry, page title: {}", new_page_title);
        }
        
        // First, wait for the dynamic content to be fully loaded
        info!("Waiting for authentication components to fully load...");
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Try to locate and click the login option more directly with JavaScript
        info!("Attempting to find login UI with JavaScript direct manipulation...");
        let js_locate = driver.execute(r#"
            // Helper function to try clicking a potential login element
            function tryClickElement(selector, description) {
                try {
                    const elements = document.querySelectorAll(selector);
                    if (elements.length > 0) {
                        for (let i = 0; i < elements.length; i++) {
                            const el = elements[i];
                            const text = el.textContent || '';
                            const rect = el.getBoundingClientRect();
                            
                            // Only try to click visible elements
                            if (rect.width > 0 && rect.height > 0) {
                                if (text.includes('Google') || text.includes('Continue with Google')) {
                                    console.log('Found by ' + description + ': ' + text);
                                    el.click();
                                    return `Clicked: ${description} - ${text}`;
                                }
                            }
                        }
                    }
                    return '';
                } catch (e) {
                    return `Error with ${description}: ${e.message}`;
                }
            }
            
            // Try various selectors and click the first one that works
            let result = '';
            
            // Try to find buttons first
            result = tryClickElement('button', 'button tag');
            if (result) return result;
            
            // Try links next
            result = tryClickElement('a', 'link tag');
            if (result) return result;
            
            // Try elements with login-related classes
            result = tryClickElement('.social-button, .login-button, .google, .Google, .auth-provider', 'class selector');
            if (result) return result;
            
            // Try elements with certain aria roles
            result = tryClickElement('[role="button"]', 'role button');
            if (result) return result;
            
            // Try to access shadow DOM
            const shadowRoots = [];
            function collectShadowRoots(root) {
                const elements = root.querySelectorAll('*');
                for (const el of elements) {
                    if (el.shadowRoot) {
                        shadowRoots.push(el.shadowRoot);
                        collectShadowRoots(el.shadowRoot);
                    }
                }
            }
            
            // Start with document body
            collectShadowRoots(document);
            
            // Check each shadow root
            for (const root of shadowRoots) {
                result = tryClickElement.call(null, root.querySelectorAll('button, a'), 'shadow DOM');
                if (result) return result;
            }
            
            // Look for the string "Continue with Google" anywhere in the page content
            // and try to find a clickable parent
            const allElements = document.querySelectorAll('*');
            for (const el of allElements) {
                const text = el.textContent || '';
                if (text.includes('Google') || text.includes('Continue with Google')) {
                    // Try to find clickable parent or self
                    let current = el;
                    for (let i = 0; i < 5; i++) { // Try up to 5 levels up
                        if (current.tagName === 'BUTTON' || current.tagName === 'A' || 
                            current.getAttribute('role') === 'button' || 
                            current.onclick || current.getAttribute('class')?.includes('button')) {
                            current.click();
                            return `Clicked parent of Google text: ${current.tagName}`;
                        }
                        if (!current.parentElement) break;
                        current = current.parentElement;
                    }
                }
            }
            
            // Return details about potentially relevant elements for debugging
            const potentialElements = [];
            document.querySelectorAll('button, a, [role="button"]').forEach(el => {
                potentialElements.push({
                    tag: el.tagName,
                    text: el.textContent,
                    class: el.getAttribute('class'),
                    visible: el.getBoundingClientRect().width > 0 && el.getBoundingClientRect().height > 0
                });
            });
            
            return "No suitable element found. Potential elements: " + JSON.stringify(potentialElements.slice(0, 10));
        "#, vec![]).await;
        
        match js_locate {
            Ok(result) => {
                let result_str = format!("{:?}", result);
                info!("JavaScript login location result: {}", result_str);
                if result_str.contains("Clicked") {
                    info!("Successfully clicked on login element via JavaScript");
                    // Wait for potential redirect or popup
                    tokio::time::sleep(Duration::from_secs(8)).await;
                    
                    // Check if we were redirected to Google
                    if let Ok(url) = driver.current_url().await {
                        if url.to_string().contains("accounts.google.com") {
                            info!("Successfully redirected to Google login: {}", url);
                            // If we have credentials, proceed with login
                            if let (Some(_email), Some(_password)) = (google_email, google_password) {
                                // Handle Google login logic here - keep existing code for this
                                info!("Now attempting to login with the provided Google credentials");
                            }
                        } else {
                            info!("Current URL after click: {}", url);
                        }
                    }
                }
            },
            Err(e) => info!("JavaScript login location error: {:?}", e)
        };
        
        // Dump the full page source for debugging
        let _page_source = match driver.source().await {
            Ok(source) => {
                info!("Page source length: {} characters", source.len());
                
                // Look for specific structures that would indicate the presence of auth components
                if source.contains("auth-flow-manager") {
                    info!("Found auth-flow-manager in page source");
                }
                if source.contains("faceplate-partial") {
                    info!("Found faceplate-partial in page source");
                }
                if source.contains("googleid-signin-script") {
                    info!("Found googleid-signin-script in page source");
                }
                
                // Check if we're inside a Shadow DOM structure
                if source.contains("shadow-root") || source.contains("shadowRoot") {
                    info!("Page may be using Shadow DOM for authentication components");
                }
                
                source
            },
            Err(e) => {
                info!("Failed to get page source: {}", e);
                String::new()
            }
        };
        
        // Try using JavaScript to find elements within shadow DOM if present
        let shadow_dom_check = driver.execute(r#"
            // Helper function to find elements in Shadow DOM
            function findButtonInShadowDOM(root, text) {
                if (!root) return null;
                
                // Check all buttons in this shadow root
                const elements = root.querySelectorAll('button, a');
                for (const el of elements) {
                    if (el.textContent && (
                        el.textContent.includes('Google') || 
                        el.textContent.includes('Continue with Google') ||
                        el.textContent.includes('Sign in with Google')
                    )) {
                        return el;
                    }
                }
                
                // Recursively search in child shadow roots
                const shadowElements = root.querySelectorAll('*');
                for (const el of shadowElements) {
                    if (el.shadowRoot) {
                        const found = findButtonInShadowDOM(el.shadowRoot, text);
                        if (found) return found;
                    }
                }
                
                return null;
            }
            
            // Start search from document
            const shadowHosts = document.querySelectorAll('auth-flow-manager, *[shadow-root], *[shadowroot]');
            for (const host of shadowHosts) {
                if (host.shadowRoot) {
                    const button = findButtonInShadowDOM(host.shadowRoot, 'Google');
                    if (button) {
                        button.click();
                        return "Shadow DOM button clicked";
                    }
                }
            }
            
            return "No button found in Shadow DOM";
        "#, vec![]).await;
        
        match shadow_dom_check {
            Ok(result) => {
                info!("Shadow DOM search result: {:?}", result);
                // If JavaScript reports success, wait for redirection
                if format!("{:?}", result).contains("clicked") {
                    info!("Successfully clicked Google button via Shadow DOM");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    // Check if we've been redirected to Google
                    if let Ok(current_url) = driver.current_url().await {
                        if current_url.to_string().contains("accounts.google.com") {
                            info!("Successfully redirected to Google login: {}", current_url);
                            // Don't return yet, let the authorization code flow continue
                            info!("Successfully interacted with shadow DOM, waiting for authorization callback...");
                        }
                    }
                }
            },
            Err(e) => info!("Shadow DOM JavaScript execution error: {}", e)
        }
        
        // Enhanced selectors for the Google login button - focusing on deep structures and iframes
        info!("Trying enhanced selectors for Google login button...");
        
        // Try to find iframes that might contain login components
        let frames = match driver.find_all(By::Tag("iframe")).await {
            Ok(frames) => {
                info!("Found {} iframes on the page", frames.len());
                frames
            },
            Err(_) => vec![]
        };
        
        // Try switching to each iframe to look for the Google button
        for (i, frame) in frames.iter().enumerate() {
            info!("Checking iframe #{}", i+1);
            
            // Try to switch to this frame using newer API
            if let Err(e) = frame.clone().enter_frame().await {
                info!("Failed to switch to frame #{}: {:?}", i+1, e);
                continue;
            }
            
            info!("Switched to iframe #{}, looking for Google button", i+1);
            
            // Look for Google button in this frame
            if let Ok(button) = driver.find(By::XPath("//button[contains(text(), 'Google') or contains(text(), 'Continue with Google')]")).await {
                info!("Found Google button in iframe #{}", i+1);
                if let Err(e) = button.click().await {
                    info!("Failed to click Google button in iframe: {}", e);
                } else {
                    info!("Successfully clicked Google button in iframe");
                    // Switch back to main content using newer API
                    let _ = driver.enter_default_frame().await;
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    // Don't return yet, let the authorization code flow continue
                    info!("Successfully interacted with iframe, waiting for authorization callback...");
                }
            }
            
            // Switch back to main content using newer API
            if let Err(e) = driver.enter_default_frame().await {
                info!("Failed to switch back to main content: {:?}", e);
            }
        }
        
        // Now try more advanced selectors in the main document
        let google_login_button = match driver.find(By::XPath("//button[contains(text(), 'Continue with Google')]")).await {
            Ok(button) => {
                info!("Found 'Continue with Google' button (text-based), clicking...");
                Some(button)
            },
            Err(_) => match driver.find(By::XPath("//div[contains(@class, 'Google')]//button")).await {
                Ok(button) => {
                    info!("Found Google button (class-based), clicking...");
                    Some(button)
                },
                Err(_) => match driver.find(By::XPath("//button[contains(@data-provider, 'google')]")).await {
                    Ok(button) => {
                        info!("Found Google button (data-provider), clicking...");
                        Some(button)
                    },
                    Err(_) => match driver.find(By::XPath("//a[contains(@href, 'google')]")).await {
                        Ok(button) => {
                            info!("Found Google link, clicking...");
                            Some(button)
                        },
                        Err(_) => match driver.find(By::XPath("//*[contains(text(), 'Continue with Google') or contains(text(), 'Sign in with Google')]")).await {
                            Ok(button) => {
                                info!("Found element with Google text, clicking...");
                                Some(button)
                            },
                            Err(_) => match driver.find(By::Css("[aria-label*='Google'], [title*='Google'], [data-provider='google'], [class*='Google'], [class*='google'], button[class*='social'], .social-icon, iframe[id*='google'], iframe[src*='google'], iframe[name*='google']")).await {
                                Ok(button) => {
                                    info!("Found element with Google in aria-label, title, or class, clicking...");
                                    // Try to log more details about this button
                                    if let Ok(tag_name) = button.tag_name().await {
                                        info!("Button tag name: {}", tag_name);
                                    }
                                    
                                    // If this is an iframe, we need special handling
                                    if let Ok(tag) = button.tag_name().await {
                                        if tag.to_lowercase() == "iframe" {
                                            info!("Found a Google iframe - this is likely a Google Sign-In button inside an iframe");
                                            
                                            // Try to switch to the iframe first
                                            if let Err(e) = button.clone().enter_frame().await {
                                                info!("Failed to enter Google iframe directly: {:?}", e);
                                                
                                                // Try script-based approach to interact with the iframe content
                                                let js_click = driver.execute(r#"
                                                    // Find all iframes related to Google
                                                    const iframes = document.querySelectorAll('iframe[id*="google"], iframe[src*="google"], iframe[name*="google"]');
                                                    if (iframes.length > 0) {
                                                        // Log what we found
                                                        const sources = Array.from(iframes).map(f => f.src || f.id || "unknown");
                                                        
                                                        // Try to interact with the first one
                                                        try {
                                                            // First approach: Try to click the iframe itself
                                                            iframes[0].click();
                                                            return "Clicked Google iframe directly: " + sources.join(", ");
                                                        } catch (e1) {
                                                            try {
                                                                // Second approach: Try to use postMessage to the iframe
                                                                iframes[0].contentWindow.postMessage('{"action":"click"}', '*');
                                                                return "Sent postMessage to Google iframe: " + sources.join(", ");
                                                            } catch (e2) {
                                                                // Third approach: Create a click at iframe's position
                                                                const rect = iframes[0].getBoundingClientRect();
                                                                const centerX = rect.left + rect.width / 2;
                                                                const centerY = rect.top + rect.height / 2;
                                                                
                                                                // Create and dispatch a click event at the iframe's center
                                                                const clickEvent = new MouseEvent('click', {
                                                                    view: window,
                                                                    bubbles: true,
                                                                    cancelable: true,
                                                                    clientX: centerX,
                                                                    clientY: centerY
                                                                });
                                                                
                                                                document.elementFromPoint(centerX, centerY).dispatchEvent(clickEvent);
                                                                return "Simulated click at Google iframe center: " + sources.join(", ");
                                                            }
                                                        }
                                                    } else {
                                                        // Look for the "Sign in with Google" button by text content
                                                        const allElements = document.querySelectorAll('*');
                                                        for (const el of allElements) {
                                                            if (el.textContent && (
                                                                el.textContent.includes('Sign in with Google') || 
                                                                el.textContent.includes('Continue with Google') ||
                                                                el.textContent.includes('Google')
                                                            )) {
                                                                try {
                                                                    el.click();
                                                                    return "Found and clicked Google text: " + el.textContent;
                                                                } catch (e) {
                                                                    // Try clicking a parent element
                                                                    try {
                                                                        if (el.parentElement) {
                                                                            el.parentElement.click();
                                                                            return "Clicked parent of Google text";
                                                                        }
                                                                    } catch (e2) {
                                                                        // Last resort - create a synthetic click
                                                                        const rect = el.getBoundingClientRect();
                                                                        if (rect.width > 0 && rect.height > 0) {
                                                                            const centerX = rect.left + rect.width / 2;
                                                                            const centerY = rect.top + rect.height / 2;
                                                                            
                                                                            // Click the element at this position
                                                                            const elementAtPoint = document.elementFromPoint(centerX, centerY);
                                                                            if (elementAtPoint) {
                                                                                elementAtPoint.click();
                                                                                return "Clicked element at position of Google text";
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    return "No Google iframe or button interaction succeeded";
                                                "#, vec![]).await;
                                                
                                                if let Ok(result) = js_click {
                                                    info!("Google iframe interaction result: {:?}", result);
                                                }
                                                
                                                // Wait for potential popup/redirect
                                                tokio::time::sleep(Duration::from_secs(5)).await;
                                                
                                                // Check if we've been redirected to Google
                                                if let Ok(current_url) = driver.current_url().await {
                                                    info!("Current URL after iframe interaction: {}", current_url);
                                                }
                                            } else {
                                                info!("Successfully entered Google iframe");
                                                
                                                // Now look for the sign-in button inside the iframe
                                                if let Ok(sign_in_btn) = driver.find(By::XPath("//*[contains(text(), 'Sign in') or contains(@aria-label, 'Sign in') or contains(@id, 'signin')]")).await {
                                                    info!("Found sign-in button inside iframe, clicking...");
                                                    
                                                    // First try regular click
                                                    let click_result = sign_in_btn.click().await;
                                                    if let Err(e) = click_result {
                                                        info!("Failed to click sign-in button in iframe with standard method: {:?}", e);
                                                        
                                                        // Try JavaScript click as fallback for ElementNotInteractable errors
                                                        info!("Attempting JavaScript fallback for clicking the sign-in button...");
                                                        // Try JavaScript click without passing the element directly
                                                        let js_click = driver.execute(r#"
                                                            try {
                                                                // Try to find the sign-in button again within JavaScript
                                                                const possibleButtons = document.querySelectorAll('button, [role="button"], a, input[type="submit"]');
                                                                
                                                                // Try to find anything resembling a sign-in button
                                                                for (const btn of possibleButtons) {
                                                                    const text = (btn.textContent || '').toLowerCase();
                                                                    const label = (btn.getAttribute('aria-label') || '').toLowerCase();
                                                                    const id = (btn.id || '').toLowerCase();
                                                                    
                                                                    if (text.includes('sign in') || text.includes('login') || 
                                                                        label.includes('sign in') || id.includes('signin') || 
                                                                        id.includes('login')) {
                                                                        
                                                                        // Try direct click
                                                                        btn.click();
                                                                        
                                                                        // Also try dispatching click event
                                                                        const event = new MouseEvent('click', {
                                                                            bubbles: true,
                                                                            cancelable: true,
                                                                            view: window
                                                                        });
                                                                        btn.dispatchEvent(event);
                                                                        
                                                                        return "Successfully clicked sign-in button via JavaScript";
                                                                    }
                                                                }
                                                                
                                                                return "No sign-in button found for JavaScript click";
                                                            } catch (e) {
                                                                return "JavaScript click error: " + e.message;
                                                            }
                                                        "#, vec![]).await;
                                                        
                                                        match js_click {
                                                            Ok(result) => info!("JavaScript click result: {:?}", result),
                                                            Err(e) => info!("JavaScript click error: {:?}", e)
                                                        }
                                                        
                                                        // Wait a bit to see if the click had any effect
                                                        tokio::time::sleep(Duration::from_secs(3)).await;
                                                        
                                                        // Also try clicking on the center of the iframe directly
                                                        info!("Trying to click in the center of the iframe as a last resort...");
                                                        let center_click = driver.execute(r#"
                                                            try {
                                                                // Find the active iframe
                                                                const iframes = document.querySelectorAll('iframe');
                                                                for (let i = 0; i < iframes.length; i++) {
                                                                    const iframe = iframes[i];
                                                                    const rect = iframe.getBoundingClientRect();
                                                                    if (rect.width > 0 && rect.height > 0) {
                                                                        // Click in the center of this visible iframe
                                                                        const centerX = rect.left + rect.width / 2;
                                                                        const centerY = rect.top + rect.height / 2;
                                                                        
                                                                        // Create a click at this position
                                                                        const element = document.elementFromPoint(centerX, centerY);
                                                                        if (element) {
                                                                            element.click();
                                                                            return "Clicked center of iframe at coordinates: " + centerX + "," + centerY;
                                                                        }
                                                                    }
                                                                }
                                                                return "No visible iframe found for center click";
                                                            } catch (e) {
                                                                return "Center click error: " + e.message;
                                                            }
                                                        "#, vec![]).await;
                                                        
                                                        if let Ok(result) = center_click {
                                                            info!("Center click result: {:?}", result);
                                                        }
                                                    } else {
                                                        info!("Successfully clicked sign-in button in iframe");
                                                    }
                                                }
                                                
                                                // Switch back to main frame
                                                let _ = driver.enter_default_frame().await;
                                            }
                                            
                                            // Wait for potential popup/redirect
                                            tokio::time::sleep(Duration::from_secs(8)).await;
                                            
                                            // Don't return yet, let the authorization code flow continue
                                            info!("Successfully interacted with Google iframe, waiting for authorization callback...");
                                        }
                                    }
                                    
                                    // Try to get attributes directly with JavaScript
                                    let js_script = r#"
                                        // Use the currently selected element (if any)
                                        const activeElement = document.activeElement;
                                        if (activeElement) {
                                            const attributes = {};
                                            for (let i = 0; i < activeElement.attributes.length; i++) {
                                                const attr = activeElement.attributes[i];
                                                attributes[attr.name] = attr.value;
                                            }
                                            return JSON.stringify(attributes);
                                        }
                                        return "No active element";
                                    "#;
                                    
                                    if let Ok(attrs) = driver.execute(js_script, vec![]).await {
                                        info!("Active element attributes: {:?}", attrs);
                                    }
                                    Some(button)
                                },
                                Err(_) => match driver.find(By::Css("button, a")).await {
                                    Ok(button) => {
                                        let button_text = button.text().await.unwrap_or_default();
                                        info!("Found a generic button/link with text: {}", button_text);
                                        if button_text.contains("Google") || button_text.contains("Continue") {
                                            info!("Button text contains 'Google' or 'Continue', clicking...");
                                            Some(button)
                                        } else {
                                            // Try finding all buttons for debugging
                                            if let Ok(buttons) = driver.find_all(By::XPath("//button")).await {
                                                info!("Found {} buttons on the page", buttons.len());
                                                for (i, btn) in buttons.iter().enumerate() {
                                                    if let Ok(text) = btn.text().await {
                                                        info!("Button #{} text: {}", i+1, text);
                                                    }
                                                }
                                            }
                                            
                                            // Try one last desperate approach - find all clickable elements and
                                            // look for any with Google icon/images in their attributes or classes
                                            if let Ok(elements) = driver.find_all(By::Css("button, a, [role='button'], [tabindex='0']")).await {
                                                info!("Found {} potentially clickable elements", elements.len());
                                                for (i, _el) in elements.iter().enumerate() {
                                                    // Check element attributes using JavaScript instead of html() method
                                                    let elem_index = i;
                                                    let js_check = driver.execute(&format!(r#"
                                                        const elements = document.querySelectorAll("button, a, [role='button'], [tabindex='0']");
                                                        const el = elements[{}];
                                                        if (!el) return "Element not found";
                                                        
                                                        const outerHTML = el.outerHTML || "";
                                                        const hasGoogle = outerHTML.includes("Google") || outerHTML.includes("google");
                                                        
                                                        if (hasGoogle) {{
                                                            try {{
                                                                el.click();
                                                                return "Clicked element with Google in HTML";
                                                            }} catch (e) {{
                                                                return "Failed to click: " + e.message;
                                                            }}
                                                        }}
                                                        
                                                        return "No Google content in element";
                                                    "#, elem_index), vec![]).await;
                                                    
                                                    if let Ok(result) = js_check {
                                                        let result_str = format!("{:?}", result);
                                                        if result_str.contains("Clicked") {
                                                            info!("Found and clicked element #{} with Google reference: {}", i+1, result_str);
                                                            tokio::time::sleep(Duration::from_secs(5)).await;
                                                            // Don't return yet, let the authorization code flow continue
                                            info!("Successfully found and clicked element, waiting for authorization callback...");
                                                        }
                                                    }
                                                }
                                            }
                                            None
                                        }
                                    },
                                    Err(_) => {
                                        info!("No buttons or links found on the page at all.");
                                        
                                        // Try JavaScript as a last resort
                                        let js_result = driver.execute(r#"
                                            // Try different approaches to find Google button
                                            const googleButtons = Array.from(document.querySelectorAll('*')).filter(el => {
                                                const text = el.textContent || '';
                                                const classes = el.className || '';
                                                const attrs = el.attributes ? Array.from(el.attributes).map(a => a.value).join(' ') : '';
                                                return (text.includes('Google') || classes.includes('Google') || 
                                                       attrs.includes('Google') || attrs.includes('google')) &&
                                                       (el.tagName === 'BUTTON' || el.tagName === 'A' || 
                                                        el.getAttribute('role') === 'button' || el.onclick);
                                            });
                                            
                                            if (googleButtons.length > 0) {
                                                googleButtons[0].click();
                                                return "Clicked element via JavaScript";
                                            }
                                            
                                            return "No Google element found via JavaScript";
                                        "#, vec![]).await;
                                        
                                        match js_result {
                                            Ok(result) => info!("JavaScript result: {:?}", result),
                                            Err(e) => info!("JavaScript error: {}", e)
                                        }
                                        
                                        // Wait a bit longer for any potential redirection
                                        tokio::time::sleep(Duration::from_secs(5)).await;
                                        
                                        debug!("Google login button not found. Checking if we're already at the authorization page...");
                                        None
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };
        
        // If we found the Google login button, click it
        if let Some(button) = google_login_button {
            button.click().await
                .map_err(|e| RedditClientError::ApiError(format!("Failed to click Google login button: {}", e)))?;
            
            // Wait for Google login page to load
            tokio::time::sleep(Duration::from_secs(5)).await; // Longer wait time
            
            // If Google credentials were provided, try to log in automatically
            if let (Some(email), Some(password)) = (google_email, google_password) {
                info!("Attempting to enter Google credentials...");
                
                // Use our advanced Google login helper instead of the original code
                if let Err(e) = self.handle_google_login(&driver, email, password).await {
                    info!("Error during Google login: {:?}", e);
                }
                
                // Check if we've been redirected away from the login page
                if let Ok(current_url) = driver.current_url().await {
                    let url_string = current_url.to_string();
                    info!("Current URL after Google login attempt: {}", url_string);
                    
                    if !url_string.contains("login") && !url_string.contains("accounts.google.com") {
                        info!("Successfully navigated away from login page, likely authenticated");
                    } else {
                        info!("Still on login page after Google login attempt");
                    }
                }
            } else if fully_headless {
                // We're in headless mode but missing one of the credentials
                return Err(RedditClientError::ApiError(
                    "Both Google email and password must be provided when using fully headless mode.".to_string()
                ));
            } else {
                // We're in visible browser mode - guide the user
                info!("Please complete the Google authentication in the browser window that appeared.");
                info!("After signing in with Google, authorize the Reddit application if prompted.");
                info!("Waiting up to 3 minutes for authentication to complete...");
                
                // Wait longer for manual authentication through VNC
                // 5 minutes should be enough time to connect to VNC and authenticate
                info!("Waiting for you to complete authentication through the VNC interface (http://localhost:7900)...");
                tokio::time::sleep(Duration::from_secs(300)).await;
            }
        } else {
            // No Google button found, try to find if there's a direct Reddit login we could use
            // as a fallback (only if credentials were provided)
            if let (Some(email), Some(password)) = (google_email, google_password) {
                match driver.find(By::Id("loginUsername")).await {
                    Ok(username_field) => {
                        info!("Standard Reddit login form found. Attempting direct Reddit login as fallback.");
                        
                        // Try direct Reddit login since we have credentials
                        username_field.send_keys(email).await
                            .map_err(|e| RedditClientError::ApiError(format!("Failed to enter username: {}", e)))?;
                            
                        match driver.find(By::Id("loginPassword")).await {
                            Ok(password_field) => {
                                info!("Found password field, entering password");
                                password_field.send_keys(password).await
                                    .map_err(|e| RedditClientError::ApiError(format!("Failed to enter password: {}", e)))?;
                                    
                                // Try different login button selectors
                                let login_button = match driver.find(By::XPath("//button[contains(text(), 'Log In')]")).await {
                                    Ok(btn) => Some(btn),
                                    Err(_) => match driver.find(By::Css("button[type='submit']")).await {
                                        Ok(btn) => Some(btn),
                                        Err(_) => match driver.find(By::XPath("//button[contains(@class, 'login')]")).await {
                                            Ok(btn) => Some(btn),
                                            Err(_) => None
                                        }
                                    }
                                };
                                
                                if let Some(btn) = login_button {
                                    info!("Found login button, clicking...");
                                    btn.click().await
                                        .map_err(|e| RedditClientError::ApiError(format!("Failed to click login button: {}", e)))?;
                                        
                                    info!("Clicked login button, waiting for redirect...");
                                    tokio::time::sleep(Duration::from_secs(8)).await;
                                } else {
                                    info!("No login button found");
                                }
                            },
                            Err(_) => {
                                info!("Could not find password field");
                            }
                        }
                    },
                    Err(_) => {
                        debug!("No login form found. We might already be at the authorization page or logged in.");
                    }
                }
            } else {
                match driver.find(By::Id("loginUsername")).await {
                    Ok(_) => {
                        info!("Standard Reddit login form found, but no credentials provided for this flow.");
                        info!("Consider using the 'token-create' command with pre-obtained tokens instead.");
                        
                        // Try advanced methods to detect and interact with Google login buttons
                        info!("Using advanced methods to locate Google Sign-In button...");
                        let google_button_attempt = driver.execute(r#"
                            function findGoogleLoginOptions() {
                                // Helper to locate a Google login button in various ways
                                let foundButtons = [];
                                
                                // 1. Look for standard Google buttons
                                try {
                                    // Look for any visible elements containing Google
                                    document.querySelectorAll('*').forEach(el => {
                                        // Skip hidden elements
                                        const rect = el.getBoundingClientRect();
                                        if (rect.width === 0 || rect.height === 0) return;
                                        
                                        // Check text content for Google
                                        const text = el.textContent?.toLowerCase() || '';
                                        if (text.includes('google') || text.includes('sign in with google') || text.includes('continue with google')) {
                                            foundButtons.push({
                                                element: el,
                                                type: 'text',
                                                text: text
                                            });
                                        }
                                        
                                        // Check attributes for Google
                                        for (const attr of el.attributes || []) {
                                            if ((attr.name.includes('google') || attr.value.includes('google')) && 
                                                (el.tagName === 'BUTTON' || el.tagName === 'A' || el.getAttribute('role') === 'button')) {
                                                foundButtons.push({
                                                    element: el,
                                                    type: 'attribute',
                                                    attr: `${attr.name}="${attr.value}"`
                                                });
                                            }
                                        }
                                    });
                                } catch (e) {
                                    console.error('Error in standard button search:', e);
                                }
                                
                                // 2. Look for Google Identity API elements
                                try {
                                    // The Google Identity API renders into divs with specific IDs
                                    document.querySelectorAll('div[id^="g_id_"], div[id^="credential_picker_"], #gsi_frame, iframe[id^="gsi_"]').forEach(el => {
                                        foundButtons.push({
                                            element: el,
                                            type: 'identity-api',
                                            id: el.id
                                        });
                                    });
                                } catch (e) {
                                    console.error('Error in Google Identity API search:', e);
                                }
                                
                                // 3. Look for iframes that might contain Google Sign-In
                                try {
                                    document.querySelectorAll('iframe').forEach(iframe => {
                                        const src = iframe.src || '';
                                        const id = iframe.id || '';
                                        if (src.includes('google') || id.includes('google') || id.includes('gsi')) {
                                            foundButtons.push({
                                                element: iframe,
                                                type: 'iframe',
                                                src: src,
                                                id: id
                                            });
                                        }
                                    });
                                } catch (e) {
                                    console.error('Error in iframe search:', e);
                                }
                                
                                return foundButtons;
                            }
                            
                            const buttons = findGoogleLoginOptions();
                            
                            // Now try to click each button until one works
                            let clickResults = [];
                            for (const button of buttons) {
                                try {
                                    // Different click strategies based on element type
                                    if (button.type === 'iframe') {
                                        // For iframes, click in the center of the frame
                                        const iframe = button.element;
                                        const rect = iframe.getBoundingClientRect();
                                        const centerX = rect.left + rect.width / 2;
                                        const centerY = rect.top + rect.height / 2;
                                        
                                        // Try to click at this position
                                        const elementAtPoint = document.elementFromPoint(centerX, centerY);
                                        if (elementAtPoint) {
                                            elementAtPoint.click();
                                            clickResults.push(`Clicked center of ${button.type} at ${centerX},${centerY}`);
                                        }
                                    } else {
                                        // For regular elements, try a direct click
                                        button.element.click();
                                        clickResults.push(`Clicked ${button.type} button`);
                                    }
                                } catch (e) {
                                    clickResults.push(`Failed to click ${button.type}: ${e.message}`);
                                }
                            }
                            
                            return JSON.stringify({
                                buttonsFound: buttons.length,
                                buttonTypes: buttons.map(b => b.type),
                                clickResults: clickResults
                            });
                        "#, vec![]).await;
                        
                        if let Ok(result) = google_button_attempt {
                            info!("Advanced Google button detection result: {:?}", result);
                            // Wait a bit to see if any of our click attempts worked
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            
                            // Check if the URL changed
                            if let Ok(current_url) = driver.current_url().await {
                                info!("URL after advanced Google button detection: {}", current_url);
                                
                                // Check if we've moved to Google authentication
                                if current_url.to_string().contains("accounts.google.com") {
                                    info!("Successfully navigated to Google authentication page!");
                                    
                                    // If we have credentials, let the existing Google login handling take over
                                    if let (Some(_email), Some(_password)) = (google_email, google_password) {
                                        info!("Credentials available - Google login handler will continue automatically");
                                    } else {
                                        info!("No credentials available - please log in manually through the VNC interface");
                                    }
                                }
                            }
                        }
                        
                        // As a final fallback, try to dismiss any login popups
                        let dismiss_attempt = driver.execute(r#"
                            // Try to find and click a close button on any modal
                            const closeButtons = document.querySelectorAll('button[aria-label="Close"], .close-button, [aria-label="close"]');
                            if (closeButtons.length > 0) {
                                closeButtons[0].click();
                                return "Dismissed login popup";
                            }
                            
                            // Try to find the 'Continue' button which might let us browse without logging in
                            const continueButtons = document.querySelectorAll('button, a');
                            for (const btn of continueButtons) {
                                if (btn.textContent && btn.textContent.includes('Continue') && !btn.textContent.includes('Google')) {
                                    btn.click();
                                    return "Clicked 'Continue' button";
                                }
                            }
                            
                            return "No dismiss options found";
                        "#, vec![]).await;
                        
                        if let Ok(result) = dismiss_attempt {
                            info!("Dismiss attempt result: {:?}", result);
                            // Wait to see if it had any effect
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                        
                        return Err(RedditClientError::ApiError(
                            "Standard Reddit login form found, but no credentials provided. Cannot proceed automatically.".to_string()
                        ));
                    },
                    Err(_) => {
                        debug!("No login form found. We might already be at the authorization page or logged in.");
                    }
                }
            }
        }
        
        // At this point we should be either at the Reddit authorization page or already redirected to the callback URL
        
        // Try to find and click the "Allow" button using multiple strategies
        info!("Looking for Reddit authorization button...");
        
        // Wait longer to make sure the page is fully loaded
        info!("Waiting for authorization page to fully load...");
        tokio::time::sleep(Duration::from_secs(8)).await;
        
        // Get the current URL to understand where we are
        let current_url_string = match driver.current_url().await {
            Ok(url) => {
                info!("Current URL: {}", url);
                url.to_string()
            },
            Err(e) => {
                info!("Failed to get current URL: {}", e);
                "unknown".to_string()
            }
        };
        
        // Check if we need to handle the Reddit authorization page
        let on_auth_page = current_url_string.contains("authorize") || 
                          current_url_string.contains("reddit.com") && !current_url_string.contains("accounts.google.com");
        let on_callback = current_url_string.contains("callback") || current_url_string.contains("localhost");

        // Log the current page's title for clarity
        if let Ok(title) = driver.title().await {
            info!("Page title before looking for Allow button: {}", title);
        }

        // Capture and log the page source for debugging if we're on the auth page 
        if on_auth_page {
            info!("On Reddit authorization page. Analyzing page contents...");
            if let Ok(source) = driver.source().await {
                info!("Authorization page source length: {} characters", source.len());
                
                // Output the full page HTML during debugging
                if source.len() < 3000 {
                    debug!("Full authorization page source: {}", source);
                }
            }
        }

        // Find the allow button using various strategies
        let found_allow = if on_callback && current_url_string.contains("localhost") {
            // Only consider it a callback if we're actually at localhost
            // Otherwise, we're just at a Reddit page that happens to have "callback" in the URL
            info!("Already redirected to callback URL: {}", current_url_string);
            true
        } else if !on_auth_page {
            info!("Not on Reddit authorization page - we're at: {}", current_url_string);
            if current_url_string.contains("accounts.google.com") {
                info!("Still on Google login page - login process may need manual intervention");
            } else if current_url_string.contains("login") {
                // We're still on the login page, let's try one more method to find the login button
                info!("Still on login page. Attempting one final login button detection method");
                
                // Using JavaScript to look specifically for sign-in methods
                let final_button_attempt = driver.execute(r#"
                    // Try to find any Google-related buttons especially in Reddit's login UI
                    const divs = document.querySelectorAll('div');
                    
                    // Look for anything related to social login
                    for (const div of divs) {
                        const text = div.textContent || '';
                        if (text.includes('Continue with Google') || 
                            text.includes('Sign in with Google') ||
                            text.includes('Google')) {
                                
                            // Try to find an ancestor that's clickable
                            let current = div;
                            let depth = 0;
                            while (current && depth < 5) {
                                if (current.onclick || 
                                    current.tagName === 'BUTTON' || 
                                    current.tagName === 'A' ||
                                    current.getAttribute('role') === 'button') {
                                    current.click();
                                    return "Found and clicked login element: " + current.tagName;
                                }
                                
                                // Check children for clickable elements
                                const children = current.querySelectorAll('button, a, [role="button"]');
                                if (children.length > 0) {
                                    children[0].click();
                                    return "Found and clicked child element: " + children[0].tagName;
                                }
                                
                                current = current.parentElement;
                                depth++;
                            }
                            
                            // If we couldn't find a clickable parent, try a simulated click
                            try {
                                const rect = div.getBoundingClientRect();
                                const centerX = rect.left + rect.width / 2;
                                const centerY = rect.top + rect.height / 2;
                                
                                // Create and dispatch a click event
                                const clickEvent = new MouseEvent('click', {
                                    view: window,
                                    bubbles: true,
                                    cancelable: true,
                                    clientX: centerX,
                                    clientY: centerY
                                });
                                
                                div.dispatchEvent(clickEvent);
                                return "Simulated click on text element containing Google";
                            } catch (e) {
                                return "Error simulating click: " + e.message;
                            }
                        }
                    }
                    
                    return "No Google login elements found in final attempt";
                "#, vec![]).await;
                
                if let Ok(result) = final_button_attempt {
                    info!("Final button attempt result: {:?}", result);
                    
                    // Wait to see if the click had any effect
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    
                    // Check if our URL changed
                    if let Ok(new_url) = driver.current_url().await {
                        info!("URL after final button attempt: {}", new_url);
                        if new_url.to_string() != current_url_string {
                            info!("Page changed after final button attempt!");
                            // Don't return yet, let the authorization code flow continue
                            info!("Successfully clicked via final attempt, waiting for authorization callback...");
                        }
                    }
                }
            }
            false
        } else {
            // Try multiple strategies for finding the Allow button
            match driver.find(By::XPath("//button[contains(text(), 'Allow')]")).await {
                Ok(allow_button) => {
                    info!("Found 'Allow' button (text-based), clicking...");
                    allow_button.click().await
                        .map_err(|e| RedditClientError::ApiError(format!("Failed to click Allow button: {}", e)))?;
                    true
                },
                Err(_) => match driver.find(By::XPath("//button[contains(@class, 'allow') or contains(@class, 'approve')]")).await {
                    Ok(allow_button) => {
                        info!("Found allow button (class-based), clicking...");
                        allow_button.click().await
                            .map_err(|e| RedditClientError::ApiError(format!("Failed to click allow button: {}", e)))?;
                        true
                    },
                    Err(_) => match driver.find(By::XPath("//input[@type='submit' and (@value='Allow' or @value='Approve')]")).await {
                        Ok(allow_button) => {
                            info!("Found allow input button, clicking...");
                            allow_button.click().await
                                .map_err(|e| RedditClientError::ApiError(format!("Failed to click allow input: {}", e)))?;
                            true
                        },
                        Err(_) => match driver.find(By::Css("button, input[type='submit'], a.button")).await {
                            Ok(button) => {
                                let button_text = button.text().await.unwrap_or_default();
                                info!("Found a generic button with text: {}", button_text);
                                if button_text.contains("Allow") || button_text.contains("Approve") || 
                                   button_text.contains("Authorize") || button_text.contains("Accept") {
                                    info!("Button text contains authorization keywords, clicking...");
                                    button.click().await
                                        .map_err(|e| RedditClientError::ApiError(format!("Failed to click button: {}", e)))?;
                                    true
                                } else {
                                    // Try to find all buttons for debugging
                                    if let Ok(buttons) = driver.find_all(By::XPath("//button")).await {
                                        info!("Found {} buttons on the page", buttons.len());
                                        for (i, btn) in buttons.iter().enumerate() {
                                            if let Ok(text) = btn.text().await {
                                                info!("Button #{} text: {}", i+1, text);
                                                // If any buttons have relevant text, click it
                                                if text.contains("Allow") || text.contains("Approve") || 
                                                   text.contains("Authorize") || text.contains("Accept") {
                                                    info!("Found relevant button: {}", text);
                                                    let _ = btn.click().await; // Ignore result, just try clicking
                                                    // Don't return yet, let the authorization code flow continue
                                                    info!("Successfully clicked allow button, waiting for authorization callback...");
                                                }
                                            }
                                        }
                                    }
                                    false
                                }
                            },
                            Err(_) => {
                                // Try scraping and searching the page source for clues
                                if let Ok(source) = driver.source().await {
                                    if source.contains("Allow") || source.contains("Approve") || 
                                       source.contains("Authorize") || source.contains("Accept") {
                                        info!("Page contains authorization text but button not found with selectors");
                                        // Try JavaScript click as a last resort
                                        match driver.execute("document.querySelector('button, input[type=submit]').click();", vec![]).await {
                                            Ok(_) => {
                                                info!("Attempted JavaScript click on a button");
                                                true
                                            },
                                            Err(e) => {
                                                info!("JavaScript click failed: {}", e);
                                                false
                                            }
                                        }
                                    } else {
                                        info!("No authorization keywords found in page source");
                                        false
                                    }
                                } else {
                                    false
                                }
                            }
                        }
                    }
                }
            }
        };
        
        if !found_allow {
            debug!("Allow button not found. User might already be authorized or automatically redirected.");
        }
        
        // Clean up the browser
        info!("Closing browser...");
        driver.quit().await
            .map_err(|e| RedditClientError::ApiError(format!("Failed to close browser: {}", e)))?;
        
        // Wait for the authorization code from the server
        info!("Waiting for authorization code from callback server...");
        info!("If you're manually testing, copy the code from the URL parameter after completing authorization.");
        info!("Example: If redirected to http://localhost:8080/callback?state=abc123&code=XYZABC, the code is XYZABC");
        
        let auth_result = match rx.recv_timeout(Duration::from_secs(60)) {
            Ok(result) => result,
            Err(_) => {
                // Timed out - provide instructions for manual code entry
                info!("Timed out waiting for authorization code. You may need to manually authorize.");
                info!("==== MANUAL AUTHORIZATION STEPS ====");
                info!("1. Open this URL in a browser: {}", auth_url);
                info!("2. Log in and authorize the application");
                info!("3. You'll be redirected to a URL like: http://localhost:{}/callback?state={}&code=ABC123", port, state);
                info!("4. The code is the value after 'code=' in the URL");
                info!("5. If you can't access the callback URL, copy the entire URL from your browser's address bar");
                info!("=================================");
                
                return Err(RedditClientError::ApiError(
                    "Timed out waiting for authorization. Try running the command again or use the token-create method with manually obtained tokens.".to_string()
                ))
            }
        };
        
        // Process the authorization code
        let code = match auth_result {
            Ok(code) => code,
            Err(e) => {
                return Err(RedditClientError::ApiError(e))
            }
        };
        
        // Wait for the server thread to complete
        let _ = server_thread.join();
        
        // Exchange the code for an access token
        info!("Exchanging authorization code for access token...");
        
        let params = [
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
        ];
        
        // For installed apps, the auth header uses just the client_id followed by a colon
        let auth = base64::encode(format!("{}:", client_id));
        
        let res = self.client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;
            
        // Check for HTTP errors
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await?;
            return Err(RedditClientError::ApiError(
                format!("Token exchange failed: HTTP {}: {}", status, body)
            ));
        }
        
        let json: serde_json::Value = res.json().await?;
        
        // Check for API errors
        if let Some(error) = json["error"].as_str() {
            return Err(RedditClientError::ApiError(
                format!("Token exchange failed: {}", error)
            ));
        }
        
        // Get the access token
        let token = json["access_token"].as_str()
            .ok_or_else(|| RedditClientError::ApiError(
                "Failed to extract access token from response".to_string()
            ))?.to_string();
        
        // Store the token for future use
        self.access_token = Some(token.clone());
        
        // Update token storage
        let now = chrono::Utc::now().timestamp() as u64;
        let expires_in = json["expires_in"].as_u64().unwrap_or(3600);
        let expires_at = now + expires_in;
        
        // Create or update our token storage
        if self.token_storage.is_none() {
            self.token_storage = Some(TokenStorage::new(client_id));
        }
        
        if let Some(storage) = &mut self.token_storage {
            storage.client_id = client_id.to_string();
            storage.access_token = Some(token.clone());
            storage.token_expires_at = Some(expires_at);
            storage.last_updated = now;
            
            // Store the refresh token if provided
            if let Some(refresh_token) = json["refresh_token"].as_str() {
                storage.refresh_token = Some(refresh_token.to_string());
                debug!("Received and stored refresh token");
            }
            
            // Save the token storage
            self.save_token_storage()?;
        }
        
        info!("Headless browser OAuth authentication successful, token obtained");
        
        Ok(token)
    }
    
    pub fn with_user_agent(user_agent: String) -> Self {
        Self {
            client: Self::get_client(&user_agent).unwrap(),
            access_token: None,
            user_agent,
            token_storage: None,
        }
    }
    
    /// Load stored tokens for a client ID if available
    pub fn with_stored_tokens(client_id: &str) -> Self {
        let mut client = Self::new();
        
        if let Some(storage) = Self::load_token_storage(client_id) {
            if storage.is_access_token_valid() {
                // If we have a valid access token, use it
                client.access_token = storage.access_token.clone();
            }
            client.token_storage = Some(storage);
        } else {
            // No stored tokens, create a new storage
            client.token_storage = Some(TokenStorage::new(client_id));
        }
        
        client
    }
    
    /// Set token values manually (useful for headless environments)
    pub fn set_tokens(&mut self, client_id: &str, access_token: &str, refresh_token: Option<&str>, expires_in: u64) -> Result<(), RedditClientError> {
        let now = chrono::Utc::now().timestamp() as u64;
        let expires_at = now + expires_in;
        
        // Create or update token storage
        if self.token_storage.is_none() {
            self.token_storage = Some(TokenStorage::new(client_id));
        }
        
        if let Some(storage) = &mut self.token_storage {
            storage.client_id = client_id.to_string();
            storage.access_token = Some(access_token.to_string());
            storage.token_expires_at = Some(expires_at);
            storage.last_updated = now;
            
            if let Some(refresh) = refresh_token {
                storage.refresh_token = Some(refresh.to_string());
            }
            
            // Save the token storage
            self.save_token_storage()?;
        }
        
        // Set the token for immediate use
        self.access_token = Some(access_token.to_string());
        
        Ok(())
    }
    
    /// Get the directory for storing tokens
    fn get_token_dir() -> PathBuf {
        let mut token_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        token_dir.push(".redrust");
        
        // Create the directory if it doesn't exist
        if !token_dir.exists() {
            fs::create_dir_all(&token_dir).ok();
        }
        
        token_dir
    }
    
    /// Get the path to the token file for a client ID
    fn get_token_path(client_id: &str) -> PathBuf {
        let mut path = Self::get_token_dir();
        path.push(format!("{}.json", client_id));
        path
    }
    
    /// Load token storage from the filesystem
    fn load_token_storage(client_id: &str) -> Option<TokenStorage> {
        let token_path = Self::get_token_path(client_id);
        
        if !token_path.exists() {
            return None;
        }
        
        let mut file = match File::open(&token_path) {
            Ok(file) => file,
            Err(_) => return None,
        };
        
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_err() {
            return None;
        }
        
        match serde_json::from_str::<TokenStorage>(&contents) {
            Ok(storage) => Some(storage),
            Err(e) => {
                debug!("Failed to parse token storage: {}", e);
                None
            }
        }
    }
    
    /// Save token storage to the filesystem
    fn save_token_storage(&self) -> Result<(), RedditClientError> {
        if let Some(storage) = &self.token_storage {
            let token_path = Self::get_token_path(&storage.client_id);
            
            let json = serde_json::to_string_pretty(storage)
                .map_err(|e| RedditClientError::ApiError(
                    format!("Failed to serialize token storage: {}", e)
                ))?;
            
            let mut file = File::create(&token_path)
                .map_err(|e| RedditClientError::ApiError(
                    format!("Failed to create token file: {}", e)
                ))?;
            
            file.write_all(json.as_bytes())
                .map_err(|e| RedditClientError::ApiError(
                    format!("Failed to write token file: {}", e)
                ))?;
            
            debug!("Saved token storage to {}", token_path.display());
        }
        
        Ok(())
    }

    fn get_client(user_agent: &str) -> Result<Client, RedditClientError> {
        Ok(Client::builder()
            .user_agent(user_agent)
            .build()?)
    }

    /// Get an application-only access token for reading public data.
    /// 
    /// This method gets a token that can only be used for reading public data.
    /// It cannot be used for actions that require a user account like posting,
    /// commenting, or voting.
    pub async fn get_access_token(&mut self, client_id: &str) -> Result<String, RedditClientError> {
        let params = [
            ("grant_type", "https://oauth.reddit.com/grants/installed_client"),
            ("device_id", "DO_NOT_TRACK_THIS_DEVICE")
        ];

        // Note: Since there is no client secret, the authorization is created using your client_id followed by a colon.
        let auth = base64::encode(format!("{}:", client_id));

        let res = self.client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;

        let json: serde_json::Value = res.json().await?;
        let token = json["access_token"].as_str()
            .ok_or_else(|| RedditClientError::ApiError(
                "Failed to extract access token from response".to_string()
            ))?.to_string();
        
        // Store the token for future use
        self.access_token = Some(token.clone());
        debug!("Application-only access token successfully obtained");
        
        Ok(token)
    }
    
    /// Authenticate with Reddit using username and password (password flow).
    /// 
    /// This method gets a user-specific token that can be used for actions like posting,
    /// commenting, voting, and other operations that require a user account.
    /// 
    /// # Arguments
    /// * `client_id` - Your Reddit API client ID
    /// * `username` - Reddit username
    /// * `password` - Reddit password
    /// 
    /// # Note
    /// Your Reddit application must be set up as a "script" type app for this to work.
    /// The scope "submit" is included to allow posting.
    /// 
    /// # Important
    /// This method doesn't work with Reddit accounts that use Google OAuth or other
    /// third-party login methods. For those, use authenticate_with_api_credentials() instead.
    
    /// Authenticate with Reddit using the interactive browser OAuth flow.
    /// 
    /// This method works with any Reddit account, including those using Google OAuth.
    /// It will open a web browser where the user can log in with their normal method
    /// and authorize the application.
    /// 
    /// # Arguments
    /// * `client_id` - Your Reddit API client ID for an installed app
    /// * `redirect_port` - The port to use for the localhost redirect (default: 8080)
    /// * `scopes` - The permissions to request (default includes read and submit)
    /// 
    /// # Returns
    /// A Result containing the access token if successful
    /// 
    /// # How this works:
    /// 1. Starts a local web server on localhost to receive the OAuth callback
    /// 2. Opens a browser for the user to log in and authorize the app
    /// 3. Reddit redirects back to localhost with an authorization code
    /// 4. Exchanges this code for an access token
    /// Try to refresh the access token using a stored refresh token
    pub async fn refresh_access_token(&mut self) -> Result<String, RedditClientError> {
        let storage = match &self.token_storage {
            Some(storage) if storage.has_refresh_token() => storage.clone(),
            _ => return Err(RedditClientError::ApiError(
                "No refresh token available".to_string()
            )),
        };
        
        let refresh_token = storage.refresh_token.as_ref().unwrap();
        let client_id = storage.client_id.clone();
        
        debug!("Refreshing access token using refresh token");
        
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];
        
        // For the Authorization header, use just the client_id
        let auth = base64::encode(format!("{}:", client_id));
        
        let res = self.client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;
            
        // Check for HTTP errors
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await?;
            return Err(RedditClientError::ApiError(
                format!("Token refresh failed: HTTP {}: {}", status, body)
            ));
        }
        
        let json: serde_json::Value = res.json().await?;
        
        // Check for API errors
        if let Some(error) = json["error"].as_str() {
            return Err(RedditClientError::ApiError(
                format!("Token refresh failed: {}", error)
            ));
        }
        
        // Get the new access token
        let token = json["access_token"].as_str()
            .ok_or_else(|| RedditClientError::ApiError(
                "Failed to extract access token from response".to_string()
            ))?.to_string();
        
        // Update expiration time if provided
        let expires_in = json["expires_in"].as_u64().unwrap_or(3600);
        let now = chrono::Utc::now().timestamp() as u64;
        let token_expires_at = now + expires_in;
        
        // Update our token storage
        if let Some(storage) = &mut self.token_storage {
            storage.access_token = Some(token.clone());
            storage.token_expires_at = Some(token_expires_at);
            storage.last_updated = now;
            
            // Save the updated token storage
            self.save_token_storage()?;
        }
        
        // Store the token for immediate use
        self.access_token = Some(token.clone());
        debug!("Access token refreshed successfully");
        
        Ok(token)
    }

    /// Authenticate with browser OAuth, but first try to use a stored refresh token
    pub async fn authenticate_with_stored_or_browser(
        &mut self,
        client_id: &str,
        redirect_port: Option<u16>,
        scopes: Option<&str>,
    ) -> Result<String, RedditClientError> {
        // Make sure we have token storage
        if self.token_storage.is_none() {
            self.token_storage = Some(TokenStorage::new(client_id));
        }
        
        // First try to use an existing token if it's still valid
        if let Some(storage) = &self.token_storage {
            if storage.is_access_token_valid() {
                debug!("Using existing valid access token");
                if let Some(token) = &storage.access_token {
                    self.access_token = Some(token.clone());
                    return Ok(token.clone());
                }
            }
            
            // If we have a refresh token, try to use it
            if storage.has_refresh_token() {
                debug!("Trying to refresh access token");
                match self.refresh_access_token().await {
                    Ok(token) => {
                        debug!("Successfully refreshed token");
                        return Ok(token);
                    },
                    Err(e) => {
                        debug!("Failed to refresh token: {}, will try browser auth", e);
                        // Continue to browser auth
                    }
                }
            }
        }
        
        // If we get here, we need browser authentication
        debug!("Proceeding with browser authentication");
        self.authenticate_with_browser_oauth(client_id, redirect_port, scopes).await
    }

    pub async fn authenticate_with_browser_oauth(
        &mut self,
        client_id: &str,
        redirect_port: Option<u16>,
        scopes: Option<&str>,
    ) -> Result<String, RedditClientError> {
        // Setup parameters
        let port = redirect_port.unwrap_or(8080);
        let scopes = scopes.unwrap_or("identity read submit");
        let redirect_uri = format!("http://localhost:{}/callback", port);
        
        // Generate a random state token to prevent CSRF
        let state: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        
        // Create the authorization URL
        let auth_url = format!(
            "https://www.reddit.com/api/v1/authorize?client_id={}&response_type=code&state={}&redirect_uri={}&duration=permanent&scope={}",
            client_id, state, redirect_uri, scopes
        );
        
        // Start the local server to receive the callback
        let server = match Server::http(format!("127.0.0.1:{}", port)) {
            Ok(server) => server,
            Err(e) => return Err(RedditClientError::ApiError(
                format!("Failed to start local server: {}", e)
            )),
        };
        
        // Create a channel to receive the authorization code
        let (tx, rx) = mpsc::channel();
        
        // Clone values for the server thread
        let state_clone = state.clone();
        let tx_clone = tx.clone();
        
        // Start the server in a separate thread
        let server_thread = thread::spawn(move || {
            info!("Waiting for authorization callback on http://localhost:{}/callback", port);
            
            // Set a timeout value - we'll exit the loop after this duration
            let _timeout_duration = Duration::from_secs(300); // 5 minutes
            
            // Handle incoming requests
            for request in server.incoming_requests() {
                let path = request.url();
                
                // Only handle the expected callback path
                if path.starts_with("/callback") {
                    debug!("Received callback: {}", path);
                    
                    // Parse the query parameters
                    let url_str = format!("http://localhost{}", path);
                    let query = match Url::parse(&url_str) {
                        Ok(url) => {
                            // Collect into a HashMap to avoid borrowing issues
                            let pairs: HashMap<String, String> = url.query_pairs()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect();
                            pairs
                        },
                        Err(_) => {
                            // Failed to parse URL, return an error page
                            let response = Response::from_string(
                                "<html><body><h1>Error</h1><p>Invalid callback URL</p></body></html>"
                            ).with_status_code(StatusCode(400));
                            request.respond(response).ok();
                            continue;
                        }
                    };
                    
                    // Check for errors from Reddit
                    if let Some(error) = query.get("error") {
                        tx_clone.send(Err(format!("Authorization error: {}", error))).unwrap();
                        
                        // Return an error page
                        let response = Response::from_string(format!(
                            "<html><body><h1>Authentication Error</h1><p>{}</p></body></html>",
                            error
                        )).with_status_code(StatusCode(400));
                        request.respond(response).ok();
                        break;
                    }
                    
                    // Check for the state parameter
                    match query.get("state") {
                        Some(received_state) if received_state == &state_clone => {
                            // State matches, check for the code
                            if let Some(code) = query.get("code") {
                                tx_clone.send(Ok(code.to_string())).unwrap();
                                
                                // Return a success page
                                let response = Response::from_string(
                                    "<html><body><h1>Authentication Successful</h1><p>You can now close this window and return to the application.</p></body></html>"
                                ).with_status_code(StatusCode(200));
                                request.respond(response).ok();
                                break;
                            } else {
                                tx_clone.send(Err("No authorization code received".to_string())).unwrap();
                                
                                // Return an error page
                                let response = Response::from_string(
                                    "<html><body><h1>Authentication Error</h1><p>No authorization code received</p></body></html>"
                                ).with_status_code(StatusCode(400));
                                request.respond(response).ok();
                                break;
                            }
                        },
                        Some(_) => {
                            tx_clone.send(Err("State mismatch - possible CSRF attack".to_string())).unwrap();
                            
                            // Return an error page
                            let response = Response::from_string(
                                "<html><body><h1>Authentication Error</h1><p>State mismatch - possible CSRF attack</p></body></html>"
                            ).with_status_code(StatusCode(400));
                            request.respond(response).ok();
                            break;
                        },
                        None => {
                            tx_clone.send(Err("No state parameter received".to_string())).unwrap();
                            
                            // Return an error page
                            let response = Response::from_string(
                                "<html><body><h1>Authentication Error</h1><p>No state parameter received</p></body></html>"
                            ).with_status_code(StatusCode(400));
                            request.respond(response).ok();
                            break;
                        }
                    }
                } else {
                    // Not the callback endpoint
                    let response = Response::from_string(
                        "<html><body><h1>404 Not Found</h1></body></html>"
                    ).with_status_code(StatusCode(404));
                    request.respond(response).ok();
                }
            }
        });
        
        // Open the browser to the authorization URL
        info!("Opening browser for Reddit OAuth authorization...");
        if let Err(e) = webbrowser::open(&auth_url) {
            tx.send(Err(format!("Failed to open browser: {}", e))).unwrap();
        }
        
        // Print the URL in case the browser doesn't open
        info!("If your browser doesn't open automatically, please visit this URL:");
        info!("{}", auth_url);
        
        // Wait for the authorization code
        let auth_result = match rx.recv_timeout(Duration::from_secs(300)) {
            Ok(result) => result,
            Err(_) => {
                return Err(RedditClientError::ApiError(
                    "Timed out waiting for authorization".to_string()
                ))
            }
        };
        
        // Process the authorization code
        let code = match auth_result {
            Ok(code) => code,
            Err(e) => {
                return Err(RedditClientError::ApiError(e))
            }
        };
        
        // Wait for the server thread to complete
        let _ = server_thread.join();
        
        // Exchange the code for an access token
        info!("Exchanging authorization code for access token...");
        
        let params = [
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
        ];
        
        // For installed apps, the auth header uses just the client_id followed by a colon
        let auth = base64::encode(format!("{}:", client_id));
        
        let res = self.client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;
            
        // Check for HTTP errors
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await?;
            return Err(RedditClientError::ApiError(
                format!("Token exchange failed: HTTP {}: {}", status, body)
            ));
        }
        
        let json: serde_json::Value = res.json().await?;
        
        // Check for API errors
        if let Some(error) = json["error"].as_str() {
            return Err(RedditClientError::ApiError(
                format!("Token exchange failed: {}", error)
            ));
        }
        
        // Get the access token
        let token = json["access_token"].as_str()
            .ok_or_else(|| RedditClientError::ApiError(
                "Failed to extract access token from response".to_string()
            ))?.to_string();
        
        // Store the refresh token if available
        if let Some(refresh_token) = json["refresh_token"].as_str() {
            debug!("Received refresh token: {}", refresh_token);
            // You could store this for later use
        }
        
        // Store the token for future use
        self.access_token = Some(token.clone());
        
        // Update token storage
        let now = chrono::Utc::now().timestamp() as u64;
        let expires_in = json["expires_in"].as_u64().unwrap_or(3600);
        let expires_at = now + expires_in;
        
        // Create or update our token storage
        if self.token_storage.is_none() {
            self.token_storage = Some(TokenStorage::new(client_id));
        }
        
        if let Some(storage) = &mut self.token_storage {
            storage.client_id = client_id.to_string();
            storage.access_token = Some(token.clone());
            storage.token_expires_at = Some(expires_at);
            storage.last_updated = now;
            
            // Store the refresh token if provided
            if let Some(refresh_token) = json["refresh_token"].as_str() {
                storage.refresh_token = Some(refresh_token.to_string());
                debug!("Received and stored refresh token");
            }
            
            // Save the token storage
            self.save_token_storage()?;
        }
        
        debug!("Browser OAuth authentication successful, token obtained");
        
        Ok(token)
    }

    /// Authenticate with Reddit using API credentials for a script app.
    ///
    /// This method is for Reddit accounts that use any login method (including Google OAuth)
    /// but requires you to create a "script" type app and provide your actual Reddit username
    /// and password.
    ///
    /// # Arguments
    /// * `client_id` - Your Reddit API client ID
    /// * `client_secret` - Your Reddit API client secret
    /// * `username` - Your Reddit username 
    /// * `password` - Your Reddit password
    ///
    /// # How to get these credentials:
    /// 1. Go to https://www.reddit.com/prefs/apps
    /// 2. Click "create another app..." at the bottom
    /// 3. Select "script" as the app type (important!)
    /// 4. Fill in the name and description
    /// 5. For the redirect URI, you can use http://localhost:8080
    /// 6. After creation, the client ID is under the app name
    /// 7. The client secret is listed as "secret"
    pub async fn authenticate_with_api_credentials(
        &mut self,
        client_id: &str,
        client_secret: &str,
        username: &str,
        password: &str
    ) -> Result<String, RedditClientError> {
        // For script apps, you must use the password grant type with your
        // actual Reddit username and password
        let params = [
            ("grant_type", "password"),
            ("username", username),
            ("password", password),
            // Include the scopes needed for posting
            ("scope", "submit identity read")
        ];

        // For the Authorization header, use the client_id and client_secret
        let auth = base64::encode(format!("{}:{}", client_id, client_secret));

        let res = self.client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;
            
        // Check for HTTP errors
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await?;
            return Err(RedditClientError::ApiError(
                format!("Authentication failed: HTTP {}: {}", status, body)
            ));
        }

        let json: serde_json::Value = res.json().await?;
        
        // Check for API errors
        if let Some(error) = json["error"].as_str() {
            return Err(RedditClientError::ApiError(
                format!("Authentication failed: {}", error)
            ));
        }
        
        let token = json["access_token"].as_str()
            .ok_or_else(|| RedditClientError::ApiError(
                "Failed to extract access token from response".to_string()
            ))?.to_string();
        
        // Store the token for future use
        self.access_token = Some(token.clone());
        debug!("API authentication successful, token obtained with scopes: {:?}", json["scope"].as_str());
        
        Ok(token)
    }
    
    pub async fn authenticate_user(
        &mut self,
        client_id: &str,
        username: &str,
        password: &str
    ) -> Result<String, RedditClientError> {
        // The password grant requires these parameters
        let params = [
            ("grant_type", "password"),
            ("username", username),
            ("password", password),
            // Include the scopes needed for posting
            ("scope", "submit identity read")
        ];

        // For script apps, you use client_id as both username and password
        let auth = base64::encode(format!("{}:", client_id));

        let res = self.client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;
            
        // Check for HTTP errors
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await?;
            return Err(RedditClientError::ApiError(
                format!("Authentication failed: HTTP {}: {}", status, body)
            ));
        }

        let json: serde_json::Value = res.json().await?;
        
        // Check for API errors
        if let Some(error) = json["error"].as_str() {
            return Err(RedditClientError::ApiError(
                format!("Authentication failed: {}", error)
            ));
        }
        
        let token = json["access_token"].as_str()
            .ok_or_else(|| RedditClientError::ApiError(
                "Failed to extract access token from response".to_string()
            ))?.to_string();
        
        // Store the token for future use
        self.access_token = Some(token.clone());
        debug!("User authentication successful, token obtained with scopes: {:?}", json["scope"].as_str());
        
        Ok(token)
    }

    pub async fn fetch_new_posts(&self, subreddit: &str, limit: i32) -> Result<RedditRNewResponse, RedditClientError> {
        let url = format!("https://www.reddit.com/r/{}/new.json?limit={}", subreddit, limit);
        let response = self.client.get(&url).send().await?;
        let body = response.text().await?;

        let response: RedditRNewResponse = serde_json::from_str(&body)?;
        Ok(response)
    }
    
    /// Create a new text post in a subreddit.
    /// 
    /// IMPORTANT: This method requires full OAuth user authentication with the 'submit' scope.
    /// The application-only auth from get_access_token() is not sufficient for posting.
    /// 
    /// To post content, you need to:
    /// 1. Create a Reddit OAuth app (script type) at https://www.reddit.com/prefs/apps
    /// 2. Get username and password credentials from your Reddit account
    /// 3. Implement the password OAuth flow with the 'submit' scope
    /// 
    /// This method will attempt to post, but will return a helpful error if the token lacks
    /// the required permissions.
    pub async fn create_post(&self, subreddit: &str, title: &str, text: &str) -> Result<String, RedditClientError> {
        // Ensure we have an access token
        let token = match &self.access_token {
            Some(token) => token,
            None => return Err(RedditClientError::ApiError(
                "No access token available. Call get_access_token() first.".to_string()
            )),
        };

        // Clean up the subreddit name - remove r/ if it's there
        let subreddit_clean = if subreddit.starts_with("r/") {
            &subreddit[2..]
        } else {
            subreddit
        };

        let mut params = HashMap::new();
        params.insert("sr", subreddit_clean);
        params.insert("title", title);
        params.insert("text", text);
        params.insert("kind", "self");  // "self" for text post, "link" for link post
        
        let url = "https://oauth.reddit.com/api/submit";
        
        let response = self.client
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await?;
            
        // Check if request was successful
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(RedditClientError::ApiError(
                format!("Failed to create post: HTTP {}: {}", status, text)
            ));
        }
        
        // Parse the response
        let json: serde_json::Value = response.json().await?;
        debug!("Post creation response: {:?}", json);
        
        // Check for common error messages
        if json["success"].as_bool() == Some(false) {
            // Check for user required error
            if let Some(jquery) = json["jquery"].as_array() {
                for item in jquery {
                    if let Some(call_args) = item[3].as_array() {
                        if call_args.len() > 0 && call_args[0].as_str() == Some(".error.USER_REQUIRED") {
                            return Err(RedditClientError::ApiError(
                                "Reddit requires user authentication with 'submit' scope to create posts. The current authentication method (application-only) only supports reading public data. You need to implement the full OAuth flow with a Reddit account.".to_string()
                            ));
                        }
                    }
                    
                    // Extract error message if present
                    if item[2].as_str() == Some("call") {
                        if let Some(call_args) = item[3].as_array() {
                            if call_args.len() > 0 {
                                if let Some(err_msg) = call_args[0].as_str() {
                                    if err_msg.starts_with("Please") || err_msg.contains("error") {
                                        return Err(RedditClientError::ApiError(
                                            format!("Reddit API error: {}", err_msg)
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // If there's an explicit error in the response, return it
        if let Some(errors) = json["json"]["errors"].as_array() {
            if !errors.is_empty() {
                return Err(RedditClientError::ApiError(
                    format!("Reddit API returned an error: {:?}", errors)
                ));
            }
        }
        
        // Check if the post was successful
        if json["success"].as_bool() == Some(true) {
            // In the success case, look for a redirect URL in the jQuery response
            if let Some(jquery) = json["jquery"].as_array() {
                for item in jquery {
                    // Look for the jquery call with redirect attribute
                    if item[2].as_str() == Some("attr") && item[3].as_str() == Some("redirect") {
                        // The next item contains the URL in the call parameter
                        let next_index = item[1].as_u64().unwrap_or(0) as usize;
                        if next_index < jquery.len() 
                           && jquery[next_index][2].as_str() == Some("call") 
                           && jquery[next_index][3].as_array().is_some() 
                           && jquery[next_index][3].as_array().unwrap().len() > 0 {
                            
                            if let Some(url) = jquery[next_index][3][0].as_str() {
                                return Ok(url.to_string());
                            }
                        }
                    }
                }
            }
        }
        
        // The standard way to extract the URL
        if let Some(url) = json["json"]["data"]["url"].as_str() {
            return Ok(url.to_string());
        }
        
        // For debugging purposes, print the entire response
        debug!("Full response structure: {}", serde_json::to_string_pretty(&json).unwrap_or_default());
        
        // If we got this far, check if we can at least tell if it was successful
        if json["success"].as_bool() == Some(true) {
            // The post was successful but we couldn't extract the URL for some reason
            return Ok("Post was successful, but couldn't extract the URL".to_string());
        }
        
        Err(RedditClientError::ApiError(
            "Failed to create post. Reddit requires user authentication with proper scopes for this operation.".to_string()
        ))
    }
}


