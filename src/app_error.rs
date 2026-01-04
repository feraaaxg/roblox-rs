use thiserror::Error;

#[derive(Debug, Error)]
pub enum RobloxError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("System time error: {0}")]
    Time(#[from] std::time::SystemTimeError),
    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),
    #[error("Unexpected status code: {0}")]
    UnexpectedStatus(u16),

    #[error("Invalid authentication token: {0}")]
    InvalidToken(String),
    #[error("Authentication token expired")]
    TokenExpired,
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("X-CSRF-Token header not found in response")]
    MissingCsrfToken,
    #[error("Invalid X-CSRF-Token: {0}")]
    InvalidCsrfToken(String),
    #[error("X-CSRF-Token not found: {0}")]
    CsrfTokenNotFound(String),
    #[error("rbx-authentication-ticket header not found")]
    MissingAuthenticationTicket,
    #[error("Invalid authentication ticket: {0}")]
    InvalidAuthenticationTicket(String),

    #[error("HTTP request failed with status {status}: {message}")]
    HttpError { status: u16, message: String },
    #[error("API request failed: {0}")]
    ApiError(String),
    #[error("Rate limit exceeded. Please try again later")]
    RateLimitExceeded,
    #[error("Roblox API returned an error: {0}")]
    RobloxApiError(String),

    #[error("Account not found: {0}")]
    AccountNotFound(String),
    #[error("Invalid account ID: {0}")]
    InvalidAccountId(String),
    #[error("Account initialization failed: {0}")]
    AccountInitFailed(String),
    #[error("Account is not valid")]
    AccountInvalid,
    #[error("Failed to get account info: {0}")]
    AccountInfoError(String),

    #[error("Invalid place ID: {0}")]
    InvalidPlaceId(String),
    #[error("Game not found: {0}")]
    GameNotFound(String),
    #[error("Failed to get job ID for place: {0}")]
    JobIdError(String),
    #[error("Invalid private server link: {0}")]
    InvalidPrivateServerLink(String),
    #[error("Failed to parse link code from URL: {0}")]
    LinkCodeParseError(String),
    #[error("Roblox is not installed on this system")]
    RobloxNotInstalled,
    #[error("Failed to find Roblox version directory: {0}")]
    VersionNotFound(String),
    #[error("RobloxPlayerBeta.exe not found at path: {0}")]
    RobloxExecutableNotFound(String),
    #[error("Failed to get username: {0}")]
    UsernameError(String),
    #[error("Failed to launch Roblox: {0}")]
    LaunchFailed(String),
    #[error("Roblox process is already running")]
    ProcessAlreadyRunning,
    #[error("Failed to kill Roblox process: {0}")]
    ProcessKillFailed(String),
    #[error("Process not found")]
    ProcessNotFound,

    #[error("Missing required field in response: {0}")]
    MissingField(String),
    #[error("Invalid field type in response: {0}")]
    InvalidFieldType(String),
    #[error("Failed to parse response data: {0}")]
    ParseError(String),
    #[error("Empty response from API")]
    EmptyResponse,
    #[error("Invalid cookie format: {0}")]
    InvalidCookieFormat(String),
    #[error("Cookie validation failed: {0}")]
    CookieValidationFailed(String),

    #[error("Failed to get Robux balance: {0}")]
    RobuxBalanceError(String),
    #[error("Failed to get transaction data: {0}")]
    TransactionDataError(String),
    #[error("Failed to get group Robux: {0}")]
    GroupRobuxError(String),

    #[error("Failed to get favorite games: {0}")]
    FavoriteGamesError(String),
    #[error("Failed to get gamepasses: {0}")]
    GamepassesError(String),
    #[error("Failed to get badges: {0}")]
    BadgesError(String),

    #[error("Failed verified age found {0}")]
    VerifiedAgeError(String),
    #[error("Country code error {0}")]
    CountryCodeError(String),

    #[error("Failed to fetch pending friend requests: {0}")]
    FriendRequestsFetchFailed(String),
    #[error("Failed to accept friend request from user {0}: {1}")]
    FriendRequestAcceptFailed(u64, String),
    #[error("Failed to decline friend request from user {0}: {1}")]
    FriendRequestDeclineFailed(u64, String),
    #[error("Failed to decline all friend requests: {0}")]
    DeclineAllRequestsFailed(String),
    #[error("Failed to fetch friends list: {0}")]
    FriendsListFetchFailed(String),
    #[error("Failed to fetch friends count: {0}")]
    FriendsCountFetchFailed(String),
    #[error("Failed to send friend request to user {0}: {1}")]
    FriendRequestSendFailed(u64, String),
    #[error("Failed to unfriend user {0}: {1}")]
    UnfriendFailed(u64, String),
    #[error("Invalid user ID for friend operation: {0}")]
    InvalidFriendUserId(String),

    #[error("{0}")]
    Other(String),
    #[error("{0}")]
    InvalidLink(String),
    #[error("Access code not found")]
    AccessCodeNotFound,
    #[error("Unsupported Launch Type")]
    UnsupportedLaunchType,
    #[error("ProtocolLinkNotFound")]
    ProtocolLinkNotFound,
    #[error("{0}")]
    ShareLinkParseFailed(String),
    #[error("{0}")]
    ShareLinkFetchFailed(String),
    #[error("{0}")]
    InvalidProtocolLink(String),
    #[error("{0}")]
    InvalidShareLink(String),

    #[error("Not found data in data")]
    NotFoundData,
}

impl RobloxError {
    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField(field.into())
    }

    pub fn http_error(status: u16, message: impl Into<String>) -> Self {
        Self::HttpError {
            status,
            message: message.into(),
        }
    }

    pub fn account_not_found(account_id: impl Into<String>) -> Self {
        Self::AccountNotFound(account_id.into())
    }

    pub fn game_not_found(game_id: impl Into<String>) -> Self {
        Self::GameNotFound(game_id.into())
    }

    pub fn invalid_place_id(place_id: impl Into<String>) -> Self {
        Self::InvalidPlaceId(place_id.into())
    }

    pub fn friend_request_accept_failed(user_id: u64, reason: impl Into<String>) -> Self {
        Self::FriendRequestAcceptFailed(user_id, reason.into())
    }

    pub fn friend_request_decline_failed(user_id: u64, reason: impl Into<String>) -> Self {
        Self::FriendRequestDeclineFailed(user_id, reason.into())
    }

    pub fn friend_request_send_failed(user_id: u64, reason: impl Into<String>) -> Self {
        Self::FriendRequestSendFailed(user_id, reason.into())
    }

    pub fn unfriend_failed(user_id: u64, reason: impl Into<String>) -> Self {
        Self::UnfriendFailed(user_id, reason.into())
    }

    pub fn invalid_friend_user_id(id: impl Into<String>) -> Self {
        Self::InvalidFriendUserId(id.into())
    }
}
