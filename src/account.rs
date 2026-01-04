use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};

use bon::Builder;
use log::debug;
use once_cell::sync::Lazy;
use rand::Rng;

use regex::Regex;
use reqwest::header::{COOKIE, HeaderMap, HeaderValue};
use reqwest::{Client, IntoUrl, Method, Proxy, RequestBuilder};
use serde_json::{Value, json};
use sysinfo::{ProcessRefreshKind, RefreshKind, System};
use urlencoding::encode;

use crate::GamePass;
use crate::User;
use crate::app_error::RobloxError;
use crate::{Badge, PrivateServer};
use crate::{Game, PrivateServerDetails};

static RE_ROBLOX_PROTOCOL_LINK: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"roblox://navigation/share_links\?code=([a-f0-9]+)&type=([A-Za-z]+)").unwrap()
});

static RE_SHARE_LINK_CODE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https://www\.roblox\.com/share\?code=([a-f0-9]+)&type=([A-Za-z]+)").unwrap()
});

static RE_OG_URL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"code%3D([a-f0-9]+)%26type%3D([A-Za-z]+)").unwrap());

static RE_CANONICAL_LINK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"/share-links\?code=([a-f0-9]+)&type=([A-Za-z]+)").unwrap());

static RE_DATA_LINK_ID: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"data-link-id="([a-f0-9]+)"\s+data-link-type="([A-Za-z]+)""#).unwrap()
});

static RE_ANY_CODE: Lazy<Regex> = Lazy::new(|| Regex::new(r"code=([a-f0-9]{32})").unwrap());

static RE_ANY_TYPE: Lazy<Regex> = Lazy::new(|| Regex::new(r"type=([A-Za-z]+)").unwrap());

static RE_PRIVATE_CODE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"privateServerLinkCode=(\d{20,})").unwrap());

#[derive(Debug, Clone)]
pub struct Account {
    pub token: String,
    pub client: Client,
    pub id: u64,
    pub pid: Option<u32>,
}

impl PartialEq for Account {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Account {}

impl PartialOrd for Account {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Account {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

#[derive(Default)]
pub struct AccountBuilder {
    token: Option<String>,
    proxy: Option<Proxy>,
    custom_client: Option<Client>,
}

impl AccountBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.proxy = Some(proxy);
        self
    }

    pub fn client(mut self, client: Client) -> Self {
        self.custom_client = Some(client);
        self
    }

    pub async fn build(self) -> Result<Account, RobloxError> {
        let token = self.token.ok_or(RobloxError::AccountInvalid)?;

        let client = if let Some(custom) = self.custom_client {
            custom
        } else {
            let mut builder = Client::builder();

            if let Some(proxy) = self.proxy {
                builder = builder.proxy(proxy);
            }

            builder.build().map_err(|e| RobloxError::Reqwest(e))?
        };

        let mut acc = Account {
            token,
            client,
            id: 0,
            pid: None,
        };

        if !acc.is_valid().await? {
            return Err(RobloxError::AccountInvalid);
        }

        acc.id = acc.get_account_id().await?;

        Ok(acc)
    }
}

impl Account {
    pub async fn builder() -> AccountBuilder {
        AccountBuilder::new()
    }

    pub async fn new<T>(token: T) -> Result<Self, RobloxError>
    where
        T: Into<String>,
    {
        let mut acc = Self {
            token: token.into(),
            client: Client::new(),
            id: 1,
            pid: None,
        };

        if !acc.is_valid().await? {
            return Err(RobloxError::AccountInvalid);
        }

        acc.id = acc.get_account_id().await?;

        Ok(acc)
    }

    pub fn id(&self) -> String {
        self.id.to_string()
    }

    pub fn make_request<U: IntoUrl>(&self, url: U, method: Method) -> RequestBuilder {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&format!(".ROBLOSECURITY={}", self.token)).unwrap(),
        );
        headers.insert(
            "referer",
            HeaderValue::from_static("https://www.roblox.com/"),
        );
        self.client
            .request(method, url)
            .headers(headers)
            .header("referer", "https://www.roblox.com/")
            .header("origin", "https://www.roblox.com")
            .header("accept", "application/json, text/plain, */*")
            .header("content-type", "application/json")
            .header("accept-language", "en-US,en;q=0.9")
            .header(
                "user-agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            ).body("{}")
    }

    async fn fetch_xsrf_token(&self) -> Result<String, RobloxError> {
        let response = self
            .make_request("https://auth.roblox.com/v2/logout", Method::POST)
            .send()
            .await?;

        let status = response.status();
        if !status.is_client_error() && status.as_u16() != 403 {
            return Err(RobloxError::UnexpectedStatus(status.as_u16()));
        }

        let csrf_header =
            response
                .headers()
                .get("x-csrf-token")
                .ok_or(RobloxError::InvalidCsrfToken(
                    "Header x-csrf-token not found".to_string(),
                ))?;

        let token = csrf_header
            .to_str()
            .map_err(|e| {
                RobloxError::InvalidCsrfToken(format!("Failed to parse x-csrf-token: {}", e))
            })?
            .to_string();

        Ok(token)
    }

    pub async fn is_valid(&self) -> Result<bool, RobloxError> {
        let response = self
            .make_request(
                "https://users.roblox.com/v1/users/authenticated",
                Method::GET,
            )
            .send()
            .await
            .map_err(|e| RobloxError::CookieValidationFailed(e.to_string()))?;
        Ok(response.status().is_success())
    }

    pub async fn get_account_id(&self) -> Result<u64, RobloxError> {
        let response = self
            .make_request("https://www.roblox.com/my/settings/json", Method::GET)
            .send()
            .await
            .map_err(|e| RobloxError::Reqwest(e))?;

        let status = response.status();

        if status == 403 {
            let error_text = response.text().await.unwrap_or_default();
            return Err(RobloxError::InvalidAccountId(format!(
                "403 Forbidden: {}",
                error_text.chars().take(200).collect::<String>()
            )));
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(RobloxError::InvalidAccountId(format!(
                "Request failed with status: {}. Ответ: {}",
                status,
                error_text.chars().take(200).collect::<String>()
            )));
        }
        let json: Value = response.json().await.map_err(|e| RobloxError::Reqwest(e))?;
        debug!("response json: {:?} ", json);
        match json.get("UserId") {
            Some(Value::Number(n)) => Ok(n.as_u64().ok_or_else(|| {
                RobloxError::InvalidAccountId("Error to fetch account id".to_string())
            })?),
            _ => Err(RobloxError::InvalidAccountId(
                "UserId field not found or invalid".to_string(),
            )),
        }
    }

    pub async fn get_authentication_ticket(&self) -> Result<String, RobloxError> {
        let token = self.fetch_xsrf_token().await?;
        let response_builder = self
            .make_request(
                "https://auth.roblox.com/v1/authentication-ticket/",
                Method::POST,
            )
            .header("x-csrf-token", &token)
            .body("{}");
        debug!("request: {:?}", response_builder);
        let response = response_builder.send().await?;

        let status = response.status();
        log::debug!("authentication ticket response status: {}", status);

        if status == 403 {
            let error_text = response.text().await.unwrap_or_default();
            log::error!("403 Forbidde");
            return Err(RobloxError::AuthenticationFailed(format!(
                "403 Forbidden: {}",
                error_text.chars().take(200).collect::<String>()
            )));
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(RobloxError::AuthenticationFailed(format!(
                "Failed to get authentication ticket (status: {}): {}",
                status,
                error_text.chars().take(200).collect::<String>()
            )));
        }

        response
            .headers()
            .get("rbx-authentication-ticket")
            .ok_or_else(|| {
                RobloxError::AuthenticationFailed(
                    "rbx-authentication-ticket header not found".to_string(),
                )
            })?
            .to_str()
            .map(|s| s.to_string())
            .map_err(|_e| {
                RobloxError::AuthenticationFailed("Invalid authentication ticket".to_string())
            })
    }

    pub async fn get_account_info(&self) -> Result<Value, RobloxError> {
        let response = self
            .make_request("https://www.roblox.com/my/settings/json", Method::GET)
            .send()
            .await
            .map_err(|e| RobloxError::AccountInfoError(format!("request failed: {:?}", e)))?;
        let val: Value = response.json().await.map_err(|_| {
            RobloxError::AccountInfoError("Failed to parse account info".to_string())
        })?;
        Ok(val)
    }

    pub async fn get_name(&mut self) -> Result<String, RobloxError> {
        let info = self.get_account_info().await?;
        Ok(info
            .get("Name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string())
    }

    pub async fn get_display_name(&mut self) -> Result<String, RobloxError> {
        let info = self.get_account_info().await?;
        Ok(info
            .get("DisplayName")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string())
    }

    pub async fn get_register_data(&mut self) -> Result<String, RobloxError> {
        let info = self.get_account_info().await?;
        Ok(info
            .get("AccountAgeInDays")
            .and_then(Value::as_str)
            .unwrap_or("0")
            .to_string())
    }

    pub async fn is_premium(&mut self) -> Result<bool, RobloxError> {
        let info = self.get_account_info().await?;
        Ok(info
            .get("IsPremium")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    pub async fn can_trade(&mut self) -> Result<bool, RobloxError> {
        let info = self.get_account_info().await?;
        Ok(info
            .get("CanTrade")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    pub async fn email_set(&mut self) -> Result<bool, RobloxError> {
        let info = self.get_account_info().await?;
        Ok(info
            .get("MyAccountSecurityModel")
            .and_then(|v| v.get("IsEmailSet"))
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    pub async fn email_verified(&mut self) -> Result<bool, RobloxError> {
        let info = self.get_account_info().await?;
        Ok(info
            .get("MyAccountSecurityModel")
            .and_then(|v| v.get("IsEmailVerified"))
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    pub async fn two_factor_authentication(&mut self) -> Result<bool, RobloxError> {
        let info = self.get_account_info().await?;
        Ok(info
            .get("MyAccountSecurityModel")
            .and_then(|v| v.get("IsTwoStepVerificationEnabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    pub async fn above_13(&mut self) -> Result<bool, RobloxError> {
        let info = self.get_account_info().await?;
        Ok(info
            .get("UserAbove13")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    pub async fn get_robux_balance(&self) -> Result<u64, RobloxError> {
        let response = self
            .make_request(
                &format!("https://economy.roblox.com/v1/users/{}/currency", self.id),
                Method::GET,
            )
            .send()
            .await
            .map_err(|_| RobloxError::RobuxBalanceError("failed to fetch Robux balance".into()))?;
        let json: Value = response.json().await?;
        let val = json["robux"]
            .as_u64()
            .ok_or_else(|| RobloxError::RobuxBalanceError("robux field missing".into()))?;
        Ok(val)
    }

    async fn get_transaction_data(&self) -> Result<Value, RobloxError> {
        let response = self
            .make_request(
                &format!(
                    "https://economy.roblox.com/v2/users/{}/transaction-totals?timeFrame=Year&transactionType=summary",
                    self.id
                ),
                Method::GET,
            )
            .send()
            .await
            .map_err(|_| RobloxError::TransactionDataError("failed to fetch transaction data".into()))?;
        let json: Value = response.json().await.map_err(|_| {
            RobloxError::TransactionDataError("failed to parse transaction data".into())
        })?;
        Ok(json)
    }

    pub async fn get_pending_robux(&self) -> Result<u64, RobloxError> {
        let data = self.get_transaction_data().await?;
        let val = data["pendingRobuxTotal"]
            .as_u64()
            .ok_or_else(|| RobloxError::RobuxBalanceError("pendingRobuxTotal missing".into()))?;

        Ok(val)
    }

    pub async fn get_total_robux_for_year(&self) -> Result<u64, RobloxError> {
        let data = self.get_transaction_data().await?;
        let val = data["outgoingRobuxTotal"]
            .as_i64()
            .map(|v| v.unsigned_abs())
            .ok_or_else(|| RobloxError::RobuxBalanceError("outgoingRobuxTotal missing".into()))?;

        Ok(val)
    }

    pub async fn get_total_robux_all_time(&self) -> Result<u64, RobloxError> {
        let response = self
            .make_request(
                &format!(
                    "https://economy.roblox.com/v2/users/{}/transactions?transactionType=2",
                    self.id
                ),
                Method::GET,
            )
            .send()
            .await?;
        let json: Value = response.json().await?;
        let total = json
            .get("data")
            .and_then(|data| data.as_array())
            .map_or(0, |items| {
                items
                    .iter()
                    .filter_map(|item| item.get("recentAveragePrice").and_then(Value::as_u64))
                    .sum()
            });

        Ok(total)
    }

    pub async fn get_rap_robux(&self) -> Result<u64, RobloxError> {
        let response = self
            .make_request(
                &format!(
                    "https://inventory.roblox.com/v1/users/{}/assets/collectibles",
                    self.id
                ),
                Method::GET,
            )
            .send()
            .await?;
        let json: Value = response.json().await?;
        let total_rap = json
            .get("data")
            .and_then(|data| data.as_array())
            .map_or(0, |items| {
                items
                    .iter()
                    .filter_map(|item| item.get("recentAveragePrice").and_then(Value::as_u64))
                    .sum()
            });

        Ok(total_rap)
    }

    pub async fn get_group_robux_pending(&self, group_id: &str) -> Result<u64, RobloxError> {
        let response = self
            .make_request(
                &format!(
                    "https://apis.roblox.com/transaction-records/v1/groups/{}/revenue/summary/year",
                    group_id
                ),
                Method::GET,
            )
            .send()
            .await?;
        let json: Value = response.json().await?;
        json["pendingRobux"].as_u64().ok_or_else(|| {
            RobloxError::RobuxBalanceError("pendingRobux field not found".to_string())
        })
    }

    pub async fn get_group_robux(&mut self, group_id: &str) -> Result<u64, RobloxError> {
        let response = self
            .make_request(
                &format!("https://economy.roblox.com/v1/groups/{}/currency", group_id),
                Method::GET,
            )
            .send()
            .await?;
        let json: Value = response.json().await?;
        let val = json["robux"]
            .as_u64()
            .ok_or_else(|| RobloxError::GroupRobuxError("robux field not found".to_string()))?;
        Ok(val)
    }

    pub async fn get_billing_robux(&self) -> Result<u64, RobloxError> {
        let response = self
            .make_request("https://billing.roblox.com/v1/credit", Method::GET)
            .send()
            .await?;
        let json: Value = response.json().await?;
        let val = json["robuxAmount"]
            .as_u64()
            .ok_or_else(|| RobloxError::RobuxBalanceError("robuxAmount field not found".into()))?;
        Ok(val)
    }

    pub async fn verified_age(&self) -> Result<bool, RobloxError> {
        let response = self
            .make_request(
                "https://apis.roblox.com/age-verification-service/v1/age-verification/verified-age",
                Method::GET,
            )
            .send()
            .await?;
        let json: Value = response.json().await?;
        let is_verified = json["isVerified"]
            .as_bool()
            .ok_or_else(|| RobloxError::VerifiedAgeError("isVerified field not found".into()))?;
        Ok(is_verified)
    }

    pub async fn get_country(&self) -> Result<String, RobloxError> {
        let response = self
            .make_request(
                "https://users.roblox.com/v1/users/authenticated/country-code",
                Method::GET,
            )
            .send()
            .await?;
        let json: Value = response.json().await?;
        let country = json["countryCode"]
            .as_str()
            .ok_or_else(|| RobloxError::CountryCodeError("countryCode field not found".into()))?
            .to_string();
        Ok(country)
    }

    pub async fn get_favorite_games(&self) -> Result<Vec<Game>, RobloxError> {
        let response = self
            .make_request(
                &format!(
                    "https://games.roblox.com/v2/users/{}/favorite/games",
                    self.id
                ),
                Method::GET,
            )
            .send()
            .await?;
        let json: Value = response.json().await?;
        let games: Vec<Game> = json["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|game_data| {
                Some(Game {
                    id: game_data.get("id")?.as_u64()?,
                    name: game_data.get("name")?.as_str()?.to_string(),
                    description: game_data.get("description")?.as_str()?.to_string(),
                    creator_id: game_data.get("creator")?.get("id")?.as_u64()?,
                    creator_name: game_data.get("creator")?.get("name")?.as_str()?.to_string(),
                    creator_type: game_data.get("creator")?.get("type")?.as_str()?.to_string(),
                    root_place_id: game_data.get("rootPlace")?.get("id")?.as_u64()?,
                    place_visits: game_data.get("placeVisits")?.as_u64()?,
                })
            })
            .collect();
        Ok(games)
    }

    pub async fn get_gamepasses(&self) -> Result<Vec<GamePass>, RobloxError> {
        let response = self
            .make_request(
                &format!(
                    "https://apis.roblox.com/game-passes/v1/users/{}/game-passes",
                    self.id
                ),
                Method::GET,
            )
            .send()
            .await?;
        let json: Value = response.json().await?;
        let passes: Vec<GamePass> = json["gamePasses"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|gp| {
                Some(GamePass {
                    game_pass_id: gp.get("gamePassId")?.as_u64()?,
                    name: gp.get("name")?.as_str()?.to_string(),
                    description: gp.get("description")?.as_str()?.to_string(),
                    is_for_sale: gp.get("isForSale")?.as_bool()?,
                    price: gp.get("price").and_then(Value::as_u64).map(|p| p as u32),
                    creator_id: gp.get("creator")?.get("creatorId")?.as_u64()?,
                    creator_name: gp.get("creator")?.get("name")?.as_str()?.to_string(),
                    creator_type: gp.get("creator")?.get("creatorType")?.as_str()?.to_string(),
                })
            })
            .collect();
        Ok(passes)
    }

    pub async fn get_badges(&self) -> Result<Vec<Badge>, RobloxError> {
        let response = self
            .make_request(
                &format!("https://badges.roblox.com/v1/users/{}/badges", self.id),
                Method::GET,
            )
            .send()
            .await?;
        let json: Value = response.json().await?;
        let badges: Vec<Badge> = json["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|badge| {
                Some(Badge {
                    id: badge.get("id")?.as_u64()?,
                    name: badge.get("name")?.as_str()?.to_string(),
                    typed: badge.get("awarder")?.get("type")?.as_str()?.to_string(),
                    display_name: badge.get("displayName")?.as_str()?.to_string(),
                    enabled: badge.get("enabled")?.as_bool()?,
                })
            })
            .collect();
        Ok(badges)
    }

    pub async fn get_cards_count(&self) -> Result<u8, RobloxError> {
        let response = self
            .make_request(
                "https://apis.roblox.com/payments-gateway/v1/payment-profiles",
                Method::GET,
            )
            .send()
            .await?;
        let json: Value = response.json().await?;
        let val = json.as_array().map(|arr| arr.len() as u8).unwrap_or(0);
        Ok(val)
    }

    pub async fn get_pending_friend(&self) -> Result<Vec<User>, RobloxError> {
        let response = self
            .make_request(
                "https://friends.roblox.com/v1/my/friends/requests",
                Method::GET,
            )
            .send()
            .await
            .map_err(|e| RobloxError::FriendRequestsFetchFailed(format!("{:?}", e)))?;
        if !response.status().is_success() {
            return Err(RobloxError::FriendRequestsFetchFailed(format!(
                "status {}",
                response.status()
            )));
        }
        let json: Value = response.json().await.map_err(|e| {
            RobloxError::ParseError(format!("failed to parse pending requests: {:?}", e))
        })?;
        let requests: Vec<User> = json["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|user_data| {
                Some(User {
                    id: user_data.get("id")?.as_u64()?,
                    name: user_data.get("name")?.as_str()?.to_string(),
                    display_name: user_data.get("displayName")?.as_str()?.to_string(),
                })
            })
            .collect();
        Ok(requests)
    }

    pub async fn decline_friend(&self, requester_id: u64) -> Result<(), RobloxError> {
        let csrf = self.fetch_xsrf_token().await?;
        let url = format!(
            "https://friends.roblox.com/v1/users/{}/decline-friend-request",
            requester_id
        );
        let response = self
            .make_request(&url, Method::POST)
            .header("x-csrf-token", &csrf)
            .body("{}")
            .send()
            .await
            .map_err(|e| {
                RobloxError::friend_request_decline_failed(requester_id, format!("{:?}", e))
            })?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(RobloxError::friend_request_decline_failed(
                requester_id,
                format!("status {}", response.status()),
            ))
        }
    }

    pub async fn accept_friend(&self, requester_id: u64) -> Result<(), RobloxError> {
        let csrf = self.fetch_xsrf_token().await?;
        let url = format!(
            "https://friends.roblox.com/v1/users/{}/accept-friend-request",
            requester_id
        );
        let response = self
            .make_request(&url, Method::POST)
            .header("x-csrf-token", &csrf)
            .body("{}")
            .send()
            .await
            .map_err(|e| {
                RobloxError::friend_request_accept_failed(requester_id, format!("{:?}", e))
            })?;
        debug!("response: {:#?}", response);
        if response.status().is_success() {
            Ok(())
        } else {
            Err(RobloxError::friend_request_accept_failed(
                requester_id,
                format!("status {}", response.status()),
            ))
        }
    }

    pub async fn decline_all_friend(&self) -> Result<(), RobloxError> {
        let csrf = self.fetch_xsrf_token().await?;
        let url = "https://friends.roblox.com/v1/user/friend-requests/decline-all".to_string();
        let response = self
            .make_request(&url, Method::POST)
            .header("x-csrf-token", &csrf)
            .body("{}")
            .send()
            .await
            .map_err(|e| RobloxError::DeclineAllRequestsFailed(format!("{:?}", e)))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(RobloxError::DeclineAllRequestsFailed(format!(
                "status {}",
                response.status()
            )))
        }
    }

    pub async fn get_friends(&self) -> Result<Vec<User>, RobloxError> {
        let url = format!("https://friends.roblox.com/v1/users/{}/friends", self.id);
        let response = self
            .make_request(&url, Method::GET)
            .send()
            .await
            .map_err(|e| RobloxError::FriendsListFetchFailed(format!("{:?}", e)))?;
        if !response.status().is_success() {
            return Err(RobloxError::FriendsListFetchFailed(format!(
                "status {}",
                response.status()
            )));
        }
        let json: Value = response
            .json()
            .await
            .map_err(|e| RobloxError::ParseError(format!("failed to parse friends: {:?}", e)))?;
        let friends: Vec<User> = json["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|user_data| {
                Some(User {
                    id: user_data.get("id")?.as_u64()?,
                    name: user_data.get("name")?.as_str()?.to_string(),
                    display_name: user_data.get("displayName")?.as_str()?.to_string(),
                })
            })
            .collect();
        Ok(friends)
    }

    pub async fn get_friends_count(&self) -> Result<u64, RobloxError> {
        let url = format!(
            "https://friends.roblox.com/v1/users/{}/friends/count",
            self.id
        );
        let response = self
            .make_request(&url, Method::GET)
            .send()
            .await
            .map_err(|e| RobloxError::FriendsCountFetchFailed(format!("{:?}", e)))?;
        if !response.status().is_success() {
            return Err(RobloxError::FriendsCountFetchFailed(format!(
                "status {}",
                response.status()
            )));
        }
        let json: Value = response.json().await.map_err(|e| {
            RobloxError::ParseError(format!("failed to parse friends count: {:?}", e))
        })?;
        let count = json["count"]
            .as_u64()
            .ok_or(RobloxError::MissingField("count field missing".into()))?;
        Ok(count)
    }

    pub async fn send_friend(&self, target_id: u64) -> Result<(), RobloxError> {
        let csrf = self.fetch_xsrf_token().await?;
        let url = format!(
            "https://friends.roblox.com/v1/users/{}/request-friendship",
            target_id
        );
        let response = self
            .make_request(&url, Method::POST)
            .header("x-csrf-token", &csrf)
            .body("{}")
            .send()
            .await
            .map_err(|e| RobloxError::friend_request_send_failed(target_id, format!("{:?}", e)))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(RobloxError::friend_request_send_failed(
                target_id,
                format!("status {}", response.status()),
            ))
        }
    }

    pub async fn unfriend(&self, friend_id: u64) -> Result<(), RobloxError> {
        let csrf = self.fetch_xsrf_token().await?;
        let url = format!("https://friends.roblox.com/v1/users/{}/unfriend", friend_id);
        let response = self
            .make_request(&url, Method::POST)
            .header("x-csrf-token", &csrf)
            .body("{}")
            .send()
            .await
            .map_err(|e| RobloxError::unfriend_failed(friend_id, format!("{:?}", e)))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(RobloxError::unfriend_failed(
                friend_id,
                format!("status {}", response.status()),
            ))
        }
    }

    pub async fn get_friend_count(&self) -> Result<u64, RobloxError> {
        let requests = self.get_pending_friend().await?;
        let count = requests.len() as u64;
        Ok(count)
    }

    pub async fn get_sent_friend(&self) -> Result<Vec<User>, RobloxError> {
        let url = "https://friends.roblox.com/v1/my/friends/requests/outgoing".to_string();
        let response = self
            .make_request(&url, Method::GET)
            .send()
            .await
            .map_err(|e| RobloxError::Other(format!("failed to fetch sent requests: {:?}", e)))?;
        if !response.status().is_success() {
            return Err(RobloxError::Other(format!("status {}", response.status())));
        }
        let json: Value = response.json().await.map_err(|e| {
            RobloxError::ParseError(format!("failed to parse sent requests: {:?}", e))
        })?;
        let requests: Vec<User> = json["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|user_data| {
                Some(User {
                    id: user_data.get("id")?.as_u64()?,
                    name: user_data.get("name")?.as_str()?.to_string(),
                    display_name: user_data.get("displayName")?.as_str()?.to_string(),
                })
            })
            .collect();
        Ok(requests)
    }

    pub async fn cancel_sent_friend(&self, target_id: u64) -> Result<(), RobloxError> {
        let csrf = self.fetch_xsrf_token().await?;
        let url = format!(
            "https://friends.roblox.com/v1/users/{}/decline-friend-request",
            target_id
        );
        let response = self
            .make_request(&url, Method::POST)
            .header("x-csrf-token", &csrf)
            .body("{}")
            .send()
            .await
            .map_err(|e| RobloxError::Other(format!("failed to cancel sent request: {:?}", e)))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(RobloxError::Other(format!(
                "cancel failed with status: {}",
                response.status()
            )))
        }
    }

    pub async fn get_my_private_servers(&self) -> Result<Vec<PrivateServer>, RobloxError> {
        let req = self.make_request(
            "https://games.roblox.com/v1/private-servers/my-private-servers?cursor=&itemsPerPage=200",
             Method::GET
             ).send().await?;

        let json = req.json::<Value>().await?;

        let data_array = json.get("data").ok_or_else(|| RobloxError::NotFoundData)?;

        let servers: Vec<PrivateServer> = serde_json::from_value(data_array.clone())
            .map_err(|e| RobloxError::ParseError(e.to_string()))?;

        Ok(servers)
    }

    pub async fn get_private_server_details(
        &self,
        private_server_id: u64,
    ) -> Result<PrivateServerDetails, RobloxError> {
        let req = self
            .make_request(
                format!(
                    "https://games.roblox.com/v1/vip-servers/{}",
                    private_server_id
                ),
                Method::GET,
            )
            .send()
            .await?;

        let json = req.json::<PrivateServerDetails>().await?;

        Ok(json)
    }

    pub async fn launch(&mut self, data: LaunchData) -> Result<(), RobloxError> {
        if let Some(private_code) = &data.private_code {
            let launch_type = get_launch_type(&private_code);

            match launch_type {
                LaunchType::PrivateServer => {
                    self.launch_private_server(data.place_id, private_code.clone())
                        .await
                }
                LaunchType::ShareLink => {
                    let protocol_link = self.get_protocol_link_from_share(&private_code).await?;
                    self.launch_with_protocol(protocol_link).await
                }
                LaunchType::Standard => self.launch_standard(data.place_id, data.job_id).await,
            }
        } else if let Some(job_id) = &data.job_id {
            self.launch_standard(data.place_id, Some(job_id.clone()))
                .await
        } else {
            self.launch_standard(data.place_id, None).await
        }
    }

    async fn launch_private_server(
        &mut self,
        place_id: u64,
        link: String,
    ) -> Result<(), RobloxError> {
        let launch_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        let browser_tracker_id = format!(
            "{}{}",
            rand::rng().random_range(100_000..=175_000),
            rand::rng().random_range(100_000..=900_000)
        );

        let roblox_path = Account::get_version()? + "\\RobloxPlayerBeta.exe";

        if !Path::new(&roblox_path).exists() {
            return Err(RobloxError::RobloxExecutableNotFound(roblox_path));
        }

        let link_code = Account::get_link_code(link);

        let launch_url = format!(
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestPrivateGame&placeId={}&linkCode={}",
            place_id, link_code
        );

        let encoded_url = encode(&launch_url).into_owned();

        let arguments = format!(
            "roblox-player:1+launchmode:play+gameinfo:{}+launchtime:{}+placelauncherurl:{}+browsertrackerid:{}+robloxLocale:en_us+gameLocale:en_us+channel:+LaunchExp:InApp",
            self.get_authentication_ticket().await?,
            launch_time,
            encoded_url,
            browser_tracker_id
        );

        self.execute_roblox(&roblox_path, &arguments).await
    }

    async fn launch_with_protocol(&mut self, protocol_link: String) -> Result<(), RobloxError> {
        if !protocol_link.starts_with("roblox://") {
            return Err(RobloxError::InvalidProtocolLink(protocol_link));
        }

        let roblox_path = Account::get_version()? + "\\RobloxPlayerBeta.exe";

        if !Path::new(&roblox_path).exists() {
            return Err(RobloxError::RobloxExecutableNotFound(roblox_path));
        }

        let arguments = protocol_link;

        self.execute_roblox(&roblox_path, &arguments).await
    }

    pub async fn launch_standard(
        &mut self,
        place_id: u64,
        job_id: Option<String>,
    ) -> Result<(), RobloxError> {
        let launch_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        let browser_tracker_id = format!(
            "{}{}",
            rand::rng().random_range(100_000..=175_000),
            rand::rng().random_range(100_000..=900_000)
        );

        let roblox_path = Account::get_version()? + "\\RobloxPlayerBeta.exe";

        if !Path::new(&roblox_path).exists() {
            return Err(RobloxError::RobloxExecutableNotFound(roblox_path));
        }

        let job_id_param = match job_id {
            Some(id) => {
                format!("&gameId={}", id)
            }
            None => {
                let job_id = self.job_id(place_id.to_string()).await?;
                if !job_id.is_empty() {
                    format!("&gameId={}", job_id)
                } else {
                    String::new()
                }
            }
        };

        let launch_url = format!(
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame&placeId={}{}&isPlayTogetherGame=false&isTeleport=true",
            place_id, job_id_param
        );

        let encoded_url = encode(&launch_url).into_owned();

        let arguments = format!(
            "roblox-player:1+launchmode:play+gameinfo:{}+launchtime:{}+placelauncherurl:{}+browsertrackerid:{}+robloxLocale:en_us+gameLocale:en_us+channel:+LaunchExp:InApp",
            self.get_authentication_ticket().await?,
            launch_time,
            encoded_url,
            browser_tracker_id
        );

        self.execute_roblox(&roblox_path, &arguments).await
    }

    async fn execute_roblox(
        &mut self,
        roblox_path: &str,
        arguments: &str,
    ) -> Result<(), RobloxError> {
        use std::process::Command;

        if self.is_active() {
            return Err(RobloxError::ProcessAlreadyRunning);
        }

        let process = Command::new(roblox_path)
            .arg(arguments)
            .spawn()
            .map_err(|e| {
                RobloxError::LaunchFailed(format!("Failed to spawn Roblox process: {}", e))
            })?;

        self.pid = Some(process.id());

        Ok(())
    }

    async fn get_protocol_link_from_share(&self, share_link: &str) -> Result<String, RobloxError> {
        if share_link.starts_with("roblox://") {
            return Ok(share_link.to_string());
        }

        if share_link.contains("/share?") {
            let response = self
                .client
                .get(share_link)
                .header(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                )
                .send()
                .await
                .map_err(|e| RobloxError::ShareLinkFetchFailed(e.to_string()))?;

            if !response.status().is_success() {
                return Err(RobloxError::ShareLinkFetchFailed(format!(
                    "HTTP статус: {}",
                    response.status()
                )));
            }

            let html = response
                .text()
                .await
                .map_err(|e| RobloxError::ShareLinkParseFailed(e.to_string()))?;

            if let Some((code, link_type)) = extract_share_link_info(&html) {
                let protocol_link = create_roblox_protocol_link(&code, &link_type);

                return Ok(protocol_link);
            }

            return Err(RobloxError::ProtocolLinkNotFound);
        }

        Err(RobloxError::InvalidShareLink(share_link.to_string()))
    }

    pub async fn job_id(&self, place_id: String) -> Result<String, RobloxError> {
        let url = format!(
            "https://games.roblox.com/v1/games/{}/servers/0?sortOrder=1&excludeFullGames=true&limit=25",
            place_id
        );

        let request = self.make_request(&url, reqwest::Method::GET);
        let response = request.send().await.map_err(|e| {
            RobloxError::ApiError(format!(
                "Failed to fetch servers for place {}: {}",
                place_id, e
            ))
        })?;

        if !response.status().is_success() {
            return Err(RobloxError::HttpError {
                status: response.status().as_u16(),
                message: format!("Failed to get job ID for place {}", place_id),
            });
        }

        let json: Value = response.json().await.map_err(|e| {
            RobloxError::ParseError(format!("Failed to parse JSON response: {}", e))
        })?;

        if let Some(id) = json
            .get("data")
            .and_then(|d| d.get(7))
            .and_then(|entry| entry.get("id"))
            .and_then(|id| id.as_str())
        {
            return Ok(id.to_string());
        }

        let retry_request = self.make_request(&url, reqwest::Method::GET);
        let retry_response = retry_request.send().await.map_err(|e| {
            RobloxError::ApiError(format!(
                "Retry failed to fetch servers for place {}: {}",
                place_id, e
            ))
        })?;

        if !retry_response.status().is_success() {
            return Err(RobloxError::HttpError {
                status: retry_response.status().as_u16(),
                message: format!("Failed to get job ID for place {} on retry", place_id),
            });
        }

        let retry_json: Value = retry_response.json().await.map_err(|e| {
            RobloxError::ParseError(format!("Failed to parse JSON response (retry): {}", e))
        })?;

        let id = retry_json
            .get("data")
            .and_then(|d| d.get(4))
            .and_then(|entry| entry.get("id"))
            .and_then(|id| id.as_str())
            .ok_or_else(|| RobloxError::MissingField("id in data[4]".to_string()))?;

        Ok(id.to_string())
    }

    pub fn get_link_code(private_server_link: String) -> String {
        let url = private_server_link;

        RE_PRIVATE_CODE
            .captures(url.as_str())
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    }

    pub fn get_version() -> Result<String, RobloxError> {
        let username = whoami::username();
        if username.is_empty() {
            return Err(RobloxError::UsernameError(
                "Unable to get username".to_string(),
            ));
        }
        #[cfg(not(feature = "fishtrap"))]
        let x1 = format!("C:\\Users\\{}\\AppData\\Local\\Roblox\\Versions", username);

        #[cfg(feature = "fishtrap")]
        let x1 = format!(
            "C:\\Users\\{}\\AppData\\Local\\Fishstrap\\Versions",
            username
        );

        let new_path = x1.to_owned();

        let all_dirs: Vec<_> = fs::read_dir(&new_path)
            .map_err(|e| {
                if e.kind() == io::ErrorKind::NotFound {
                    RobloxError::RobloxNotInstalled
                } else {
                    RobloxError::VersionNotFound(format!(
                        "Failed to read Roblox versions directory: {}",
                        e
                    ))
                }
            })?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.is_dir()
                    && path.file_name().map_or(false, |name| {
                        name.to_str().map_or(false, |s| s.contains("version-"))
                    })
                {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        if all_dirs.is_empty() {
            return Err(RobloxError::VersionNotFound(format!(
                "No directories containing 'version-' found in {}",
                new_path
            )));
        }

        let latest_dir = all_dirs
            .into_iter()
            .max_by_key(|path| {
                fs::metadata(path)
                    .and_then(|meta| meta.created())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            })
            .ok_or_else(|| {
                RobloxError::VersionNotFound("Failed to determine latest directory".to_string())
            })?;

        let latest_dir_str = latest_dir
            .to_str()
            .ok_or_else(|| {
                RobloxError::VersionNotFound("Failed to convert path to string".to_string())
            })?
            .to_string();

        Ok(latest_dir_str)
    }

    fn is_active(&self) -> bool {
        self.pid.is_some()
    }

    pub fn close(&mut self) -> Result<(), RobloxError> {
        if let Some(pid) = self.pid {
            #[cfg(target_os = "windows")]
            {
                use std::process::Command;

                let output = Command::new("taskkill")
                    .args(&["/F", "/PID", &pid.to_string()])
                    .output()
                    .map_err(|e| RobloxError::ProcessKillFailed(e.to_string()))?;

                if output.status.success() {
                    self.pid = None;
                    Ok(())
                } else {
                    use std::process::Command;
                    let check_output = Command::new("tasklist")
                        .args(&["/FI", &format!("PID eq {}", pid)])
                        .output();

                    let is_running = check_output
                        .map(|out| String::from_utf8_lossy(&out.stdout).contains(&pid.to_string()))
                        .unwrap_or(false);

                    if is_running {
                        Err(RobloxError::ProcessKillFailed(format!(
                            "Failed to kill process {}",
                            pid
                        )))
                    } else {
                        self.pid = None;
                        Ok(())
                    }
                }
            }
        } else {
            Ok(())
        }
    }
}

pub fn extract_share_link_info(html: &str) -> Option<(String, String)> {
    if let Some(caps) = RE_ROBLOX_PROTOCOL_LINK.captures(html) {
        return Some((caps[1].to_string(), caps[2].to_string()));
    }

    if let Some(caps) = RE_DATA_LINK_ID.captures(html) {
        println!("caps: {:?}", &caps);
        return Some((caps[1].to_string(), caps[2].to_string()));
    }
    if let Some(caps) = RE_OG_URL.captures(html) {
        return Some((caps[1].to_string(), caps[2].to_string()));
    }

    if let Some(caps) = RE_CANONICAL_LINK.captures(html) {
        return Some((caps[1].to_string(), caps[2].to_string()));
    }

    let code = RE_ANY_CODE.captures(html).map(|caps| caps[1].to_string());
    let link_type = RE_ANY_TYPE.captures(html).map(|caps| caps[1].to_string());

    if let (Some(code), Some(link_type)) = (code, link_type) {
        return Some((code, link_type));
    }

    None
}

pub fn create_roblox_protocol_link(code: &str, link_type: &str) -> String {
    format!(
        "roblox://navigation/share_links?code={}&type={}",
        code, link_type
    )
}

pub fn get_link_code(link: String) -> String {
    if link.contains("privateServerLinkCode") {
        RE_PRIVATE_CODE
            .captures(&link)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    } else if link.contains("roblox://") {
        RE_ROBLOX_PROTOCOL_LINK
            .captures(&link)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    } else if link.contains("/share?") {
        RE_SHARE_LINK_CODE
            .captures(&link)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    } else {
        String::new()
    }
}

pub fn get_launch_type(url: &str) -> LaunchType {
    if url.contains("privateServerLinkCode") {
        LaunchType::PrivateServer
    } else if url.contains("/share?") || url.contains("/share-links?") {
        LaunchType::ShareLink
    } else {
        LaunchType::Standard
    }
}

#[derive(Clone, Builder)]
pub struct LaunchData {
    place_id: u64,
    private_code: Option<String>,
    job_id: Option<String>,
    custom_path: Option<String>,
}

impl LaunchData {
    pub fn new(
        place_id: u64,
        private_code: Option<String>,
        job_id: Option<String>,
        custom_path: Option<String>,
    ) -> LaunchData {
        Self {
            place_id,
            private_code,
            job_id,
            custom_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LaunchType {
    PrivateServer,
    ShareLink,
    Standard,
}
