use reqwest::header::{ACCEPT, CONTENT_LENGTH};
use reqwest::Client;
use serde::Deserialize;
use std::error::Error;
use std::ops::Add;
use std::time::{Duration, Instant};

/// Authentication token information.
#[derive(Clone, PartialEq, Debug)]
pub struct AuthenticationInfo {
    /// Token used to refresh access token.
    pub refresh_token: String,

    /// Token to use for queries.
    pub access_token: String,

    /// Timestamp when access token expires.
    pub expires_at: Instant,

    /// API server to connect to for queries.
    pub api_server: String,

    /// Flag to indicate a practice account is in used.
    pub is_demo: bool,
}

impl AuthenticationInfo {
    /// Authenticates using the specified token and client
    pub async fn authenticate(
        refresh_token: &str,
        is_demo: bool,
        client: &Client,
    ) -> Result<AuthenticationInfo, Box<dyn Error>> {
        Self::refresh_access_token(refresh_token, is_demo, client).await
    }

    async fn refresh(&self, client: &Client) -> Result<AuthenticationInfo, Box<dyn Error>> {
        Self::refresh_access_token(self.refresh_token.as_str(), self.is_demo, client).await
    }

    async fn refresh_access_token(
        refresh_token: &str,
        is_demo: bool,
        client: &Client,
    ) -> Result<AuthenticationInfo, Box<dyn Error>> {
        #[derive(Deserialize, Clone, PartialEq, Debug)]
        pub struct AuthenticationInfoResponse {
            pub refresh_token: String,
            pub access_token: String,
            pub expires_in: u64,
            pub api_server: String,
        }

        let url = get_url(is_demo);

        let response = client
            .post(url)
            .query(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ])
            .header(CONTENT_LENGTH, 0)
            .header(ACCEPT, "application/json")
            .send()
            .await?
            .error_for_status()?
            .json::<AuthenticationInfoResponse>()
            .await?;

        Ok(AuthenticationInfo {
            refresh_token: response.refresh_token,
            access_token: response.access_token,
            expires_at: Instant::now().add(Duration::from_secs(response.expires_in)),
            api_server: response.api_server.trim_end_matches('/').into(),
            is_demo,
        })
    }
}

/// Gets the authentication url
#[inline]
fn get_url(is_demo: bool) -> &'static str {
    if is_demo {
        "https://practicelogin.questrade.com/oauth2/token"
    } else {
        "https://login.questrade.com/oauth2/token"
    }
}
