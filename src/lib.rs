mod auth;
mod error;

pub use crate::auth::AuthenticationInfo;
pub use crate::error::ApiError;
use chrono::{DateTime, Utc};
use http::StatusCode;
use itertools::Itertools;
use reqwest::header::AUTHORIZATION;
use reqwest::{Client, RequestBuilder};
use serde::de::Error as SerdeError;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Number, Value};
use std::cell::RefCell;
use std::error::Error;

type SymbolId = u32;
type OrderId = u32;
type ExecutionId = u32;
type UserId = u32;

/// Version of the API.
const API_VERSION: &str = "v1";

/// Questrade client
pub struct Questrade {
    client: Client,
    auth_info: RefCell<Option<AuthenticationInfo>>,
}

impl Questrade {
    /// Creates a new API instance with the default client.
    pub fn new() -> Self {
        Self::with_client(Client::new())
    }

    /// Creates a new API instance with the specified client
    pub fn with_client(client: Client) -> Self {
        Questrade {
            client,
            auth_info: RefCell::new(None),
        }
    }

    /// Creates a new API instance with the specified auth info.
    pub fn with_authentication(auth_info: AuthenticationInfo, client: Client) -> Self {
        Questrade {
            client,
            auth_info: RefCell::new(Some(auth_info)),
        }
    }

    //region authentication

    /// Authenticates using the supplied token.
    pub async fn authenticate(
        &self,
        refresh_token: &str,
        is_demo: bool,
    ) -> Result<(), Box<dyn Error>> {
        self.auth_info.replace(Some(
            AuthenticationInfo::authenticate(refresh_token, is_demo, &self.client).await?,
        ));

        Ok(())
    }

    /// Retrieves the current authentication info (if set).
    pub fn get_auth_info(&self) -> Option<AuthenticationInfo> {
        self.auth_info.borrow().clone()
    }

    /// Obtains an active authentication token or raises an error
    fn get_active_auth(&self) -> Result<AuthenticationInfo, ApiError> {
        self.auth_info
            .borrow()
            .clone()
            .ok_or(ApiError::NotAuthenticatedError(StatusCode::UNAUTHORIZED))
    }

    //endregion

    //region accounts

    /// List all accounts associated with the authenticated user.
    pub async fn accounts(&self) -> Result<Vec<Account>, Box<dyn Error>> {
        #[derive(Serialize, Deserialize)]
        struct AccountsResponse {
            accounts: Vec<Account>,
        }

        let response = self
            .get_request_builder("accounts")?
            .send()
            .await?
            .error_for_status()
            .map_err(|e| wrap_error(e))?
            .json::<AccountsResponse>()
            .await?;

        Ok(response.accounts)
    }

    /// Retrieve account activities, including cash transactions, dividends, trades, etc.
    pub async fn account_activity(
        &self,
        account_number: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<AccountActivity>, Box<dyn Error>> {
        #[derive(Serialize, Deserialize)]
        struct AccountActivityResponse {
            activities: Vec<AccountActivity>,
        }

        let response = self
            .get_request_builder(format!("accounts/{}/activities", account_number).as_str())?
            .query(&[
                ("startTime", start_time.to_rfc3339()),
                ("endTime", end_time.to_rfc3339()),
            ])
            .send()
            .await?
            .error_for_status()
            .map_err(|e| wrap_error(e))?
            .json::<AccountActivityResponse>()
            .await?;

        Ok(response.activities)
    }

    /// Search for account orders.
    ///
    /// Parameters:
    ///     - `start_time` optional start of time range. Defaults to start of today, 12:00am
    ///     - `end_time` optional end of time range. Defaults to end of today, 11:59pm
    ///     - `state_filter` optionally filters order states
    pub async fn account_orders(
        &self,
        account_number: &str,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        state: Option<OrderStateFilter>,
    ) -> Result<Vec<AccountOrder>, Box<dyn Error>> {
        #[derive(Debug, Serialize, Deserialize)]
        struct AccountOrdersResponse {
            orders: Vec<AccountOrder>,
        }

        let mut query_params: Vec<(&str, String)> = Vec::new();
        if let Some(start_time) = start_time {
            query_params.push(("startTime", start_time.to_rfc3339()))
        }

        if let Some(end_time) = end_time {
            query_params.push(("endTime", end_time.to_rfc3339()))
        }

        if let Some(state) = state {
            let state = match state {
                OrderStateFilter::All => "All",
                OrderStateFilter::Open => "Open",
                OrderStateFilter::Closed => "Closed",
            };

            query_params.push(("stateFilter", state.to_string()))
        }

        let response = self
            .get_request_builder(format!("accounts/{}/orders", account_number).as_str())?
            .query(query_params.as_slice())
            .send()
            .await?
            .error_for_status()
            .map_err(|e| wrap_error(e))?
            .json::<AccountOrdersResponse>()
            .await?;

        Ok(response.orders)
    }

    /// Retrieve details for an order with a specific id
    pub async fn account_order(
        &self,
        account_number: &str,
        order_id: OrderId,
    ) -> Result<Option<AccountOrder>, Box<dyn Error>> {
        #[derive(Serialize, Deserialize)]
        struct AccountOrdersResponse {
            orders: Vec<AccountOrder>,
        }

        let mut response = self
            .get_request_builder(
                format!("accounts/{}/orders/{}", account_number, order_id).as_str(),
            )?
            .send()
            .await?
            .error_for_status()
            .map_err(|e| wrap_error(e))?
            .json::<AccountOrdersResponse>()
            .await?;

        return Ok(response.orders.pop());
    }

    /// Retrieves executions for a specific account.
    ///
    /// Parameters:
    ///     - `start_time` optional start of time range. Defaults to start of today, 12:00am
    ///     - `end_time` optional end of time range. Defaults to end of today, 11:59pm
    pub async fn account_executions(
        &self,
        account_number: &str,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
    ) -> Result<Vec<AccountExecution>, Box<dyn Error>> {
        #[derive(Serialize, Deserialize)]
        struct AccountExecutionsResponse {
            executions: Vec<AccountExecution>,
        }

        let mut query_params: Vec<(&str, String)> = Vec::new();
        if let Some(start_time) = start_time {
            query_params.push(("startTime", start_time.to_rfc3339()))
        }

        if let Some(end_time) = end_time {
            query_params.push(("endTime", end_time.to_rfc3339()))
        }

        let response = self
            .get_request_builder(format!("accounts/{}/executions", account_number).as_str())?
            .query(query_params.as_slice())
            .send()
            .await?
            .error_for_status()
            .map_err(|e| wrap_error(e))?
            .json::<AccountExecutionsResponse>()
            .await?;

        Ok(response.executions)
    }

    /// Retrieves per-currency and combined balances for a specified account.
    pub async fn account_balance(
        &self,
        account_number: &str,
    ) -> Result<AccountBalances, Box<dyn Error>> {
        let response = self
            .get_request_builder(format!("accounts/{}/balances", account_number).as_str())?
            .send()
            .await?
            .error_for_status()
            .map_err(|e| wrap_error(e))?
            .json::<AccountBalances>()
            .await?;

        Ok(response)
    }

    /// Retrieves positions in a specified account.
    pub async fn account_positions(
        &self,
        account_number: &str,
    ) -> Result<Vec<AccountPosition>, Box<dyn Error>> {
        #[derive(Serialize, Deserialize)]
        struct AccountPositionsResponse {
            positions: Vec<AccountPosition>,
        }

        let response = self
            .get_request_builder(format!("accounts/{}/positions", account_number).as_str())?
            .send()
            .await?
            .error_for_status()
            .map_err(|e| wrap_error(e))?
            .json::<AccountPositionsResponse>()
            .await?;

        Ok(response.positions)
    }

    //endregion

    //region markets

    /// Retrieves a single Level 1 market data quote for one or more symbols.
    ///
    /// IMPORTANT NOTE: Questrade user needs to be subscribed to a real-time data package, to
    /// receive market quotes in real-time, otherwise call to get quote is considered snap quote and
    /// limit per market can be quickly reached. Without real-time data package, once limit is
    /// reached, the response will return delayed data.
    /// (Please check "delay" parameter in response always)
    ///
    pub async fn market_quote(&self, ids: &[SymbolId]) -> Result<Vec<MarketQuote>, Box<dyn Error>> {
        #[derive(Serialize, Deserialize)]
        struct MarketQuoteResponse {
            quotes: Vec<MarketQuote>,
        }

        let ids = ids.iter().map(ToString::to_string).join(",");

        let response = self
            .get_request_builder("markets/quotes")?
            .query(&[("ids", ids)])
            .send()
            .await?
            .error_for_status()
            .map_err(|e| wrap_error(e))?
            .json::<MarketQuoteResponse>()
            .await?;

        Ok(response.quotes)
    }

    //endregion

    //region symbols

    /// Searches for the specified symbol.
    ///
    /// params
    /// * `prefix` Prefix of a symbol or any word in the description.
    /// * `offset` Offset in number of records from the beginning of a result set.
    pub async fn symbol_search(
        &self,
        prefix: &str,
        offset: u32,
    ) -> Result<Vec<SearchEquitySymbol>, Box<dyn Error>> {
        #[derive(Serialize, Deserialize)]
        struct SymbolSearchResponse {
            symbols: Vec<SearchEquitySymbol>,
        }

        let response = self
            .get_request_builder("symbols/search")?
            .query(&[("prefix", prefix), ("offset", &offset.to_string())])
            .send()
            .await?
            .error_for_status()
            .map_err(|e| wrap_error(e))?
            .json::<SymbolSearchResponse>()
            .await?;

        Ok(response.symbols)
    }

    //endregion

    /// Retrieves current server time.
    pub async fn time(&self) -> Result<DateTime<Utc>, Box<dyn Error>> {
        #[derive(Serialize, Deserialize)]
        struct TimeResponse {
            time: DateTime<Utc>,
        }

        let response = self
            .get_request_builder("time")?
            .send()
            .await?
            .error_for_status()
            .map_err(|e| wrap_error(e))?
            .json::<TimeResponse>()
            .await?;

        Ok(response.time)
    }

    /// Get a request builder for a `get` request
    fn get_request_builder(&self, url_suffix: &str) -> Result<RequestBuilder, Box<dyn Error>> {
        let auth_info = self.get_active_auth()?;

        Ok(self
            .client
            .get(&format!(
                "{}/{}/{}",
                auth_info.api_server, API_VERSION, url_suffix
            ))
            .header(AUTHORIZATION, format!("Bearer {}", auth_info.access_token)))
    }
}

fn wrap_error(e: reqwest::Error) -> Box<dyn Error> {
    if e.is_status() {
        let status = e.status().unwrap();

        if status == 401 || status == 403 {
            return Box::new(ApiError::NotAuthenticatedError(status));
        }
    }

    Box::new(e)
}

// region accounts

/// Account record
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Account {
    /// Type of the account. (Eg: Cash / Margin)
    #[serde(rename = "type")]
    pub account_type: AccountType,

    /// Eight-digit account number (e.g., "26598145").
    pub number: String,

    /// Status of the account (e.g., Active)
    pub status: AccountStatus,

    /// Whether this is a primary account for the holder.
    #[serde(rename = "isPrimary")]
    pub is_primary: bool,

    /// Whether this account is one that gets billed for various expenses such as inactivity fees, market data, etc.
    #[serde(rename = "isBilling")]
    pub is_billing: bool,

    /// Type of client holding the account (e.g., "Individual").
    #[serde(rename = "clientAccountType")]
    pub client_account_type: ClientAccountType,
}

/// Type of account.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum AccountType {
    /// Cash account.
    Cash,

    ///Margin account.
    Margin,

    ///Tax Free Savings Account.
    TFSA,

    ///Registered Retirement Savings Plan.
    RRSP,

    ///Spousal RRSP.
    SRRSP,

    ///Locked-In RRSP.
    LRRSP,

    ///Locked-In Retirement Account.
    LIRA,

    ///	Life Income Fund.
    LIF,

    ///Retirement Income Fund.
    RIF,

    ///Spousal RIF.
    SRIF,

    ///Locked-In RIF.
    LRIF,

    ///Registered RIF.
    RRIF,

    ///Prescribed RIF.
    PRIF,

    ///Individual Registered Education Savings Plan.
    RESP,

    ///Family RESP.
    FRESP,
}

/// Status of an account.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum AccountStatus {
    Active,

    #[serde(rename = "Suspended (Closed)")]
    SuspendedClosed,

    #[serde(rename = "Suspended (View Only)")]
    SuspendedViewOnly,

    #[serde(rename = "Liquidate Only")]
    Liquidate,

    Closed,
}

/// Type of client this account is associated with.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ClientAccountType {
    ///Account held by an individual.
    Individual,

    ///Account held jointly by several individuals (e.g., spouses).
    Joint,

    /// Non-individual account held by an informal trust.
    #[serde(rename = "Informal Trust")]
    InformalTrust,

    ///Non-individual account held by a corporation.
    Corporation,

    ///Non-individual account held by an investment club.
    #[serde(rename = "Investment Club")]
    InvestmentClub,

    ///Non-individual account held by a formal trust.
    #[serde(rename = "Formal Trust")]
    FormalTrust,

    /// Non-individual account held by a partnership.
    Partnership,

    /// Non-individual account held by a sole proprietorship.
    #[serde(rename = "Sole Proprietorship")]
    SoleProprietorship,

    ///Account held by a family.
    Family,

    /// Non-individual account held by a joint and informal trust.
    #[serde(rename = "Joint and Informal Trust")]
    JointAndInformalTrust,

    ///	Non-individual account held by an institution.
    Institution,
}

/// An activity that occurred in an account
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct AccountActivity {
    /// Trade date.
    #[serde(rename = "tradeDate")]
    pub trade_date: DateTime<Utc>,

    /// Date of the transaction.
    #[serde(rename = "transactionDate")]
    pub transaction_date: DateTime<Utc>,

    /// Date the trade was settled.
    #[serde(rename = "settlementDate")]
    pub settlement_date: DateTime<Utc>,

    /// Activity action.
    pub action: String,

    /// Symbol name.
    pub symbol: String,

    /// Internal unique symbol identifier.
    #[serde(rename = "symbolId")]
    pub symbol_id: SymbolId,

    /// Textual description of the activity
    pub description: String,

    /// Activity currency (ISO format).
    pub currency: String,

    /// Number of items exchanged in the activity
    pub quantity: Number,

    /// Price of the items
    pub price: Number,

    /// Gross amount of the action, before fees
    #[serde(rename = "grossAmount")]
    pub gross_amount: Number,

    /// Questrade commission amount
    pub commission: Number,

    /// Net amount of the action, after fees
    #[serde(rename = "netAmount")]
    pub net_amount: Number,

    /// Type of activity.
    #[serde(rename = "type")]
    pub activity_type: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct AccountOrder {
    /// Internal order identifier.
    pub id: OrderId,

    /// Symbol that follows Questrade symbology (e.g., "TD.TO").
    pub symbol: String,

    /// Internal symbol identifier.
    #[serde(rename = "symbolId")]
    pub symbol_id: SymbolId,

    /// Total quantity of the order.
    #[serde(rename = "totalQuantity")]
    pub total_quantity: Number,

    /// Unfilled portion of the order quantity.
    #[serde(rename = "openQuantity")]
    #[serde(deserialize_with = "deserialize_nullable_number")]
    pub open_quantity: Number,

    /// Filled portion of the order quantity.
    #[serde(rename = "filledQuantity")]
    #[serde(deserialize_with = "deserialize_nullable_number")]
    pub filled_quantity: Number,

    /// Unfilled portion of the order quantity after cancellation.
    #[serde(rename = "canceledQuantity")]
    #[serde(deserialize_with = "deserialize_nullable_number")]
    pub canceled_quantity: Number,

    /// Client view of the order side (e.g., "Buy-To-Open").
    pub side: OrderSide,

    /// Order price type (e.g., "Market").
    #[serde(rename = "orderType")]
    #[serde(alias = "type")]
    pub order_type: OrderType,

    /// Limit price.
    #[serde(rename = "limitPrice")]
    pub limit_price: Option<Number>,

    /// Stop price.
    #[serde(rename = "stopPrice")]
    pub stop_price: Option<Number>,

    /// Specifies all-or-none special instruction.
    #[serde(rename = "isAllOrNone")]
    pub is_all_or_none: bool,

    /// Specifies Anonymous special instruction.
    #[serde(rename = "isAnonymous")]
    pub is_anonymous: bool,

    /// Specifies Iceberg special instruction.
    #[serde(rename = "icebergQuantity")]
    pub iceberg_quantity: Option<Number>,

    /// Specifies Minimum special instruction.
    #[serde(rename = "minQuantity")]
    pub min_quantity: Option<Number>,

    /// Average price of all executions received for this order.
    #[serde(rename = "avgExecPrice")]
    pub avg_execution_price: Option<Number>,

    /// Price of the last execution received for the order in question.
    #[serde(rename = "lastExecPrice")]
    pub last_execution_price: Option<Number>,

    /// Identifies the software / gateway where the order originated
    pub source: String,

    #[serde(rename = "timeInForce")]
    pub time_in_force: OrderTimeInForce,

    /// Good-Till-Date marker and date parameter
    #[serde(rename = "gtdDate")]
    pub good_till_date: Option<DateTime<Utc>>,

    /// Current order state
    pub state: OrderState,

    /// Human readable order rejection reason message.
    #[serde(rename = "clientReasonStr")]
    #[serde(alias = "rejectionReason")]
    #[serde(deserialize_with = "serde_with::rust::string_empty_as_none::deserialize")]
    pub rejection_reason: Option<String>,

    /// Internal identifier of a chain to which the order belongs.
    #[serde(rename = "chainId")]
    pub chain_id: OrderId,

    /// Order creation time.
    #[serde(rename = "creationTime")]
    pub creation_time: DateTime<Utc>,

    /// Time of the last update.
    #[serde(rename = "updateTime")]
    pub update_time: DateTime<Utc>,

    /// Notes that may have been manually added by Questrade staff.
    #[serde(deserialize_with = "serde_with::rust::string_empty_as_none::deserialize")]
    pub notes: Option<String>,

    #[serde(rename = "primaryRoute")]
    pub primary_route: String,

    #[serde(rename = "secondaryRoute")]
    #[serde(deserialize_with = "serde_with::rust::string_empty_as_none::deserialize")]
    pub secondary_route: Option<String>,

    /// Order route name.
    #[serde(rename = "orderRoute")]
    pub order_route: String,

    /// Venue where non-marketable portion of the order was booked.
    #[serde(rename = "venueHoldingOrder")]
    #[serde(deserialize_with = "serde_with::rust::string_empty_as_none::deserialize")]
    pub venue_holding_order: Option<String>,

    /// Total commission amount charged for this order.
    #[serde(rename = "comissionCharged")]
    #[serde(deserialize_with = "deserialize_nullable_number")]
    pub commission_charged: Number,

    /// Identifier assigned to this order by exchange where it was routed.
    #[serde(rename = "exchangeOrderId")]
    pub exchange_order_id: String,

    /// Whether user that placed the order is a significant shareholder.
    #[serde(rename = "isSignificantShareHolder")]
    pub is_significant_shareholder: bool,

    /// Whether user that placed the order is an insider.
    #[serde(rename = "isInsider")]
    pub is_insider: bool,

    /// Whether limit offset is specified in dollars (vs. percent).
    #[serde(rename = "isLimitOffsetInDollar")]
    pub is_limit_offset_in_dollars: bool,

    /// Internal identifier of user that placed the order.
    #[serde(rename = "userId")]
    pub user_id: UserId,

    /// Commission for placing the order via the Trade Desk over the phone.
    #[serde(rename = "placementCommission")]
    #[serde(deserialize_with = "deserialize_nullable_number")]
    pub placement_commission: Number,

    // /// List of OrderLeg elements.
    // TODO: legs,
    /// Multi-leg strategy to which the order belongs.
    #[serde(rename = "strategyType")]
    pub strategy_type: String,

    /// Stop price at which order was triggered.
    #[serde(rename = "triggerStopPrice")]
    pub trigger_stop_price: Option<Number>,

    /// Internal identifier of the order group.
    #[serde(rename = "orderGroupId")]
    pub order_group_id: OrderId,

    /// Bracket Order class. Primary, Profit or Loss.
    #[serde(rename = "orderClass")]
    pub order_class: Option<String>,
}

fn deserialize_nullable_number<'de, D>(deserializer: D) -> Result<Number, D::Error>
where
    D: Deserializer<'de>,
{
    let number: Option<Number> = Deserialize::deserialize(deserializer)?;

    match number {
        Some(num) => Ok(num),
        None => match json!(0) {
            Value::Number(n) => Ok(n),
            _ => Err(D::Error::custom(format!(
                "json!(0) did not return a Value::Number",
            ))),
        },
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum OrderSide {
    Buy,

    Sell,

    /// Sell short
    Short,

    #[serde(rename = "Cov")]
    Cover,

    #[serde(rename = "BTO")]
    BuyToOpen,

    #[serde(rename = "STC")]
    SellToClose,

    #[serde(rename = "STO")]
    SellToOpen,

    #[serde(rename = "BTC")]
    BuyToClose,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
    TrailStopInPercentage,
    TrailStopInDollar,
    TrailStopLimitInPercentage,
    TrailStopLimitInDollar,
    LimitOnOpen,
    LimitOnClose,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum OrderTimeInForce {
    Day,
    GoodTillCanceled,
    GoodTillExtendedDay,
    GoodTillDate,
    ImmediateOrCancel,
    FillOrKill,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum OrderState {
    Failed,
    Pending,
    Accepted,
    Rejected,
    CancelPending,
    Canceled,
    PartialCanceled,
    Partial,
    Executed,
    ReplacePending,
    Replaced,
    Stopped,
    Suspended,
    Expired,
    Queued,
    Triggered,
    Activated,
    PendingRiskReview,
    ContingentOrder,
}

#[derive(Clone, PartialEq, Debug)]
pub enum OrderStateFilter {
    All,
    Open,
    Closed,
}

/// An account execution.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct AccountExecution {
    ///Internal identifier of the execution.
    pub id: ExecutionId,

    /// Internal identifier of the order to which the execution belongs.
    #[serde(rename = "orderId")]
    pub order_id: OrderId,

    /// Symbol that follows Questrade symbology (e.g., "TD.TO").
    pub symbol: String,

    /// Internal symbol identifier.
    #[serde(rename = "symbolId")]
    pub symbol_id: SymbolId,

    /// Execution quantity.
    #[serde(rename = "quantity")]
    pub quantity: Number,

    /// Client view of the order side (e.g., "Buy-To-Open").
    pub side: OrderSide,

    /// Execution price.
    pub price: Number,

    /// Internal identifier of the order chain to which the execution belongs.
    #[serde(rename = "orderChainId")]
    pub order_chain_id: OrderId,

    /// Execution timestamp.
    pub timestamp: DateTime<Utc>,

    /// Notes that may have been manually added by Questrade staff.
    #[serde(deserialize_with = "serde_with::rust::string_empty_as_none::deserialize")]
    pub notes: Option<String>,

    /// Questrade commission.
    pub commission: Number,

    /// Liquidity fee charged by execution venue.
    #[serde(rename = "executionFee")]
    pub execution_fee: Number,

    /// SEC fee charged on all sales of US securities.
    #[serde(rename = "secFee")]
    pub sec_fee: Number,

    /// Additional execution fee charged by TSX (if applicable).
    #[serde(rename = "canadianExecutionFee")]
    pub canadian_execution_fee: Number,

    /// Internal identifierof the parent order.
    #[serde(rename = "parentId")]
    pub parent_id: OrderId,
}

/// Account balance for specific currency.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct AccountBalance {
    /// Currency of the balance figure.
    pub currency: Currency,

    /// Balance amount.
    pub cash: Number,

    /// Market value of all securities in the account in a given currency.
    #[serde(rename = "marketValue")]
    pub market_value: Number,

    /// Equity as a difference between cash and marketValue properties.
    #[serde(rename = "totalEquity")]
    pub total_equity: Number,

    /// Buying power for that particular currency side of the account.
    #[serde(rename = "buyingPower")]
    pub buying_power: Number,

    /// Maintenance excess for that particular side of the account.
    #[serde(rename = "maintenanceExcess")]
    pub maintenance_excess: Number,

    /// Whether real-time data was used to calculate the above balance.
    #[serde(rename = "isRealTime")]
    pub is_real_time: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Currency {
    CAD,
    USD,
}

/// Account balances.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct AccountBalances {
    #[serde(rename = "perCurrencyBalances")]
    pub per_currency_balances: Vec<AccountBalance>,

    #[serde(rename = "combinedBalances")]
    pub combined_balances: Vec<AccountBalance>,

    #[serde(rename = "sodPerCurrencyBalances")]
    pub sod_per_currency_balances: Vec<AccountBalance>,

    #[serde(rename = "sodCombinedBalances")]
    pub sod_combined_balances: Vec<AccountBalance>,
}

/// Account Position.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct AccountPosition {
    /// Symbol that follows Questrade symbology (e.g., "TD.TO").
    pub symbol: String,

    /// Internal symbol identifier.
    #[serde(rename = "symbolId")]
    pub symbol_id: SymbolId,

    /// Position quantity remaining open.
    #[serde(rename = "openQuantity")]
    pub open_quantity: Number,

    /// Portion of the position that was closed today.
    #[serde(rename = "closedQuantity")]
    pub closed_quantity: Number,

    /// Market value of the position (quantity x price).
    #[serde(rename = "currentMarketValue")]
    pub current_market_value: Number,

    /// Current price of the position symbol.
    #[serde(rename = "currentPrice")]
    pub current_price: Number,

    /// Average price paid for all executions constituting the position.
    #[serde(rename = "averageEntryPrice")]
    pub average_entry_price: Number,

    /// Realized profit/loss on this position.
    #[serde(rename = "closedPnl")]
    pub closed_profit_and_loss: Number,

    /// Unrealized profit/loss on this position.
    #[serde(rename = "openPnl")]
    pub open_profit_and_loss: Number,

    /// Total cost of the position.
    #[serde(rename = "totalCost")]
    pub total_cost: Number,

    /// Designates whether real-time quote was used to compute PnL.
    #[serde(rename = "isRealTime")]
    pub is_real_time: bool,

    /// Designates whether a symbol is currently undergoing a reorg.
    #[serde(rename = "isUnderReorg")]
    pub is_under_reorg: bool,
}

// endregion

// region markets

/// Spot quote for a certain Equity
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct MarketQuote {
    /// Symbol name following Questrade’s symbology.
    pub symbol: String,

    /// Internal symbol identifier.
    #[serde(rename = "symbolId")]
    pub symbol_id: SymbolId,

    /// Market tier.
    #[serde(deserialize_with = "serde_with::rust::string_empty_as_none::deserialize")]
    pub tier: Option<String>, //FIXME - enumeration

    /// Bid price.
    #[serde(rename = "bidPrice")]
    pub bid_price: Option<Number>,

    /// Bid quantity.
    #[serde(rename = "bidSize")]
    pub bid_size: u32,

    /// Ask price.
    #[serde(rename = "askPrice")]
    pub ask_price: Option<Number>,

    /// Ask quantity.
    #[serde(rename = "askSize")]
    pub ask_size: u32,

    /// Price of the last trade during regular trade hours.
    /// The closing price.
    #[serde(rename = "lastTradePriceTrHrs")]
    pub last_trade_price_tr_hrs: Number,

    /// Price of the last trade.
    ///
    /// May include after-hours trading.
    #[serde(rename = "lastTradePrice")]
    pub last_trade_price: Number,

    /// Quantity of the last trade.
    #[serde(rename = "lastTradeSize")]
    pub last_trade_size: u32,

    /// Trade direction.
    #[serde(rename = "lastTradeTick")]
    pub last_trade_tick: TickType,

    /// Daily trading volume
    pub volume: u32,

    /// Opening trade price.
    #[serde(rename = "openPrice")]
    pub open_price: Number,

    /// Daily high price.
    #[serde(rename = "highPrice")]
    pub high_price: Number,

    /// Daily low price.
    #[serde(rename = "lowPrice")]
    pub low_price: Number,

    /// Whether a quote is delayed or real-time.
    ///
    /// If `true` then the quote is delayed 15 minutes
    #[serde(deserialize_with = "deserialize_delay")]
    pub delay: bool,

    /// Whether trading in the symbol is currently halted.
    #[serde(rename = "isHalted")]
    pub is_halted: bool,
}

fn deserialize_delay<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let delay: u8 = Deserialize::deserialize(deserializer)?;

    match delay {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(D::Error::custom(format!(
            "expected delay to be '0' or '1'. Got: {}",
            delay
        ))),
    }
}

/// Equity details from a search query
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct SearchEquitySymbol {
    /// Symbol name. (EG: BMO)
    pub symbol: String,

    /// Internal unique symbol identifier.
    #[serde(rename = "symbolId")]
    pub symbol_id: SymbolId,

    /// Symbol description.
    pub description: String,

    /// Symbol security type.
    #[serde(rename = "securityType")]
    pub security_type: SecurityType,

    /// Primary listing exchange of the symbol.
    #[serde(rename = "listingExchange")]
    pub listing_exchange: ListingExchange,

    /// Whether a symbol has live market data.
    #[serde(rename = "isQuotable")]
    pub is_quotable: bool,

    /// Whether a symbol is tradable on the platform.
    #[serde(rename = "isTradable")]
    pub is_tradable: bool,

    /// Symbol currency.
    pub currency: Currency,
}

/// Exchange where a security is listed
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum ListingExchange {
    /// Toronto Stock Exchange.
    TSX,

    /// Toronto Stock Exchange Index.
    TSXI,

    /// Toronto Venture Exchange.
    TSXV,

    /// Canadian National Stock Exchange.
    CNSX,

    /// Montreal Exchange.
    MX,

    /// NASDAQ.
    NASDAQ,

    /// NASDAQ Index Feed.
    NASDAQI,

    /// New York Stock Exchange.
    NYSE,

    /// NYSE AMERICAN.
    NYSEAM,

    /// NYSE Global Index Feed.
    NYSEGIF,

    /// NYSE Arca.
    ARCA,

    /// Option Reporting Authority.
    OPRA,

    /// Pink Sheets.
    #[serde(rename = "PINX")]
    PinkSheets,

    /// OTC Bulletin Board.
    OTCBB,

    /// BATS Exchange
    BATS,

    /// Dow Jones Industrial Average
    #[serde(rename = "DJI")]
    DowJonesAverage,

    /// S&P 500
    #[serde(rename = "S&P")]
    SP,

    /// NEO Exchange
    NEO,

    /// Russell Indexes
    RUSSELL,

    /// Absent exchange
    #[serde(rename = "")]
    None,
}

/// Type of security
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum SecurityType {
    /// Common and preferred equities, ETFs, ETNs, units, ADRs, etc.
    Stock,

    /// Equity and index options.
    Option,

    /// Debentures, notes, bonds, both corporate and government.
    Bond,

    /// Equity or bond rights and warrants.
    Right,

    /// Physical gold (coins, wafers, bars).
    Gold,

    /// Canadian or US mutual funds.
    MutualFund,

    /// Stock indices (e.g., Dow Jones).
    Index,
}

/// Direction of trading.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum TickType {
    /// Designates an uptick.
    Up,

    /// Designates an downtick.
    Down,

    /// Designates a tick that took place at the same price as a previous one.
    Equal,
}

// endregion

#[cfg(test)]
mod tests {
    use crate::auth::AuthenticationInfo;
    use crate::{
        Account, AccountBalance, AccountBalances, AccountExecution, AccountOrder, AccountPosition,
        AccountStatus, AccountType, ClientAccountType, Currency, ListingExchange, MarketQuote,
        OrderSide, OrderState, OrderTimeInForce, OrderType, Questrade, SearchEquitySymbol,
        SecurityType, TickType,
    };
    use chrono::{FixedOffset, TimeZone, Utc};
    use reqwest::Client;
    use std::error::Error;
    use std::time::Instant;

    use mockito;
    use mockito::{mock, Matcher};
    use serde_json::{json, Number, Value};
    use std::fs::read_to_string;

    trait AsNumber {
        fn to_number(self) -> Number;
    }

    impl AsNumber for Value {
        fn to_number(self) -> Number {
            match self {
                Value::Number(n) => n,
                _ => panic!("Not a number"),
            }
        }
    }

    fn get_api() -> Questrade {
        let auth_info = AuthenticationInfo {
            access_token: "mock-access-token".to_string(),
            api_server: mockito::server_url(),
            refresh_token: "".to_string(),
            expires_at: Instant::now(),
            is_demo: false,
        };

        Questrade::with_authentication(auth_info, Client::new())
    }

    // region account
    #[tokio::test]
    async fn accounts() -> Result<(), Box<dyn Error>> {
        let _m = mock("GET", "/v1/accounts")
            .with_status(200)
            .with_header("content-type", "text/json")
            .with_body(read_to_string("test/response/accounts.json")?)
            .create();

        let result = get_api().accounts().await;

        assert_eq!(
            result?,
            vec![
                Account {
                    account_type: AccountType::Margin,
                    number: "123456".to_string(),
                    status: AccountStatus::Active,
                    is_primary: false,
                    is_billing: false,
                    client_account_type: ClientAccountType::Joint,
                },
                Account {
                    account_type: AccountType::Cash,
                    number: "26598145".to_string(),
                    status: AccountStatus::Active,
                    is_primary: true,
                    is_billing: true,
                    client_account_type: ClientAccountType::Individual,
                },
            ]
        );

        Ok(())
    }

    #[tokio::test]
    async fn account_orders() -> Result<(), Box<dyn Error>> {
        let _m = mock("GET", "/v1/accounts/123456/orders")
            .with_status(200)
            .with_header("content-type", "text/json")
            .with_body(read_to_string("test/response/account-orders.json")?)
            .create();

        let result = get_api().account_orders("123456", None, None, None).await;

        assert_eq!(
            result?,
            vec![
                AccountOrder {
                    id: 173577870,
                    symbol: "AAPL".to_string(),
                    symbol_id: 8049,
                    total_quantity: json!(100).to_number(),
                    open_quantity: json!(100).to_number(),
                    filled_quantity: json!(0).to_number(),
                    canceled_quantity: json!(0).to_number(),
                    side: OrderSide::Buy,
                    order_type: OrderType::Limit,
                    limit_price: Some(json!(500.95).to_number()),
                    stop_price: None,
                    is_all_or_none: false,
                    is_anonymous: false,
                    iceberg_quantity: None,
                    min_quantity: None,
                    avg_execution_price: None,
                    last_execution_price: None,
                    source: "TradingAPI".to_string(),
                    time_in_force: OrderTimeInForce::Day,
                    good_till_date: None,
                    state: OrderState::Canceled,
                    rejection_reason: None,
                    chain_id: 173577870,
                    creation_time: FixedOffset::west(4 * 3600)
                        .ymd(2014, 10, 23)
                        .and_hms_micro(20, 3, 41, 636000)
                        .with_timezone(&Utc),
                    update_time: FixedOffset::west(4 * 3600)
                        .ymd(2014, 10, 23)
                        .and_hms_micro(20, 3, 42, 890000)
                        .with_timezone(&Utc),
                    notes: None,
                    primary_route: "AUTO".to_string(),
                    secondary_route: None,
                    order_route: "LAMP".to_string(),
                    venue_holding_order: None,
                    commission_charged: json!(0).to_number(),
                    exchange_order_id: "XS173577870".to_string(),
                    is_significant_shareholder: false,
                    is_insider: false,
                    is_limit_offset_in_dollars: false,
                    user_id: 3000124,
                    placement_commission: json!(0).to_number(),
                    strategy_type: "SingleLeg".to_string(),
                    trigger_stop_price: None,
                    order_group_id: 0,
                    order_class: None
                },
                AccountOrder {
                    id: 173567569,
                    symbol: "XSP".to_string(),
                    symbol_id: 12873,
                    total_quantity: json!(3).to_number(),
                    open_quantity: json!(0).to_number(),
                    filled_quantity: json!(0).to_number(),
                    canceled_quantity: json!(0).to_number(),
                    side: OrderSide::Buy,
                    order_type: OrderType::Limit,
                    limit_price: Some(json!(35.05).to_number()),
                    stop_price: None,
                    is_all_or_none: false,
                    is_anonymous: false,
                    iceberg_quantity: None,
                    min_quantity: None,
                    avg_execution_price: None,
                    last_execution_price: None,
                    source: "QuestradeIQEdge".to_string(),
                    time_in_force: OrderTimeInForce::Day,
                    good_till_date: None,
                    state: OrderState::Replaced,
                    rejection_reason: None,
                    chain_id: 173567569,
                    creation_time: FixedOffset::west(4 * 3600)
                        .ymd(2015, 08, 12)
                        .and_hms_micro(11, 2, 37, 86000)
                        .with_timezone(&Utc),
                    update_time: FixedOffset::west(4 * 3600)
                        .ymd(2015, 08, 12)
                        .and_hms_micro(11, 2, 41, 241000)
                        .with_timezone(&Utc),
                    notes: None,
                    primary_route: "AUTO".to_string(),
                    secondary_route: Some("AUTO".to_string()),
                    order_route: "ITSR".to_string(),
                    venue_holding_order: None,
                    commission_charged: json!(0).to_number(),
                    exchange_order_id: "XS173577869".to_string(),
                    is_significant_shareholder: false,
                    is_insider: false,
                    is_limit_offset_in_dollars: false,
                    user_id: 3000124,
                    placement_commission: json!(0).to_number(),
                    strategy_type: "SingleLeg".to_string(),
                    trigger_stop_price: None,
                    order_group_id: 0,
                    order_class: None
                },
                AccountOrder {
                    id: 173567570,
                    symbol: "XSP".to_string(),
                    symbol_id: 12873,
                    total_quantity: json!(3).to_number(),
                    open_quantity: json!(0).to_number(),
                    filled_quantity: json!(3).to_number(),
                    canceled_quantity: json!(0).to_number(),
                    side: OrderSide::Buy,
                    order_type: OrderType::Limit,
                    limit_price: Some(json!(15.52).to_number()),
                    stop_price: None,
                    is_all_or_none: false,
                    is_anonymous: false,
                    iceberg_quantity: None,
                    min_quantity: None,
                    avg_execution_price: Some(json!(15.52).to_number()),
                    last_execution_price: None,
                    source: "QuestradeIQEdge".to_string(),
                    time_in_force: OrderTimeInForce::Day,
                    good_till_date: None,
                    state: OrderState::Executed,
                    rejection_reason: None,
                    chain_id: 173567570,
                    creation_time: FixedOffset::west(4 * 3600)
                        .ymd(2015, 08, 12)
                        .and_hms_micro(11, 3, 37, 86000)
                        .with_timezone(&Utc),
                    update_time: FixedOffset::west(4 * 3600)
                        .ymd(2015, 08, 12)
                        .and_hms_micro(11, 03, 41, 241000)
                        .with_timezone(&Utc),
                    notes: None,
                    primary_route: "AUTO".to_string(),
                    secondary_route: Some("AUTO".to_string()),
                    order_route: "ITSR".to_string(),
                    venue_holding_order: Some("ITSR".to_string()),
                    commission_charged: json!(0.0105).to_number(),
                    exchange_order_id: "XS173577870".to_string(),
                    is_significant_shareholder: false,
                    is_insider: false,
                    is_limit_offset_in_dollars: false,
                    user_id: 3000124,
                    placement_commission: json!(0).to_number(),
                    strategy_type: "SingleLeg".to_string(),
                    trigger_stop_price: None,
                    order_group_id: 0,
                    order_class: None
                }
            ]
        );

        Ok(())
    }

    #[tokio::test]
    async fn account_order() -> Result<(), Box<dyn Error>> {
        let _m = mock("GET", "/v1/accounts/123456/orders/173577870")
            .with_status(200)
            .with_header("content-type", "text/json")
            .with_body(read_to_string(
                "test/response/account-order-173577870.json",
            )?)
            .create();

        let result = get_api().account_order("123456", 173577870).await;

        assert_eq!(
            result?,
            Some(AccountOrder {
                id: 173577870,
                symbol: "AAPL".to_string(),
                symbol_id: 8049,
                total_quantity: json!(100).to_number(),
                open_quantity: json!(100).to_number(),
                filled_quantity: json!(0).to_number(),
                canceled_quantity: json!(0).to_number(),
                side: OrderSide::Buy,
                order_type: OrderType::Limit,
                limit_price: Some(json!(500.95).to_number()),
                stop_price: None,
                is_all_or_none: false,
                is_anonymous: false,
                iceberg_quantity: None,
                min_quantity: None,
                avg_execution_price: None,
                last_execution_price: None,
                source: "TradingAPI".to_string(),
                time_in_force: OrderTimeInForce::Day,
                good_till_date: None,
                state: OrderState::Canceled,
                rejection_reason: None,
                chain_id: 173577870,
                creation_time: FixedOffset::west(4 * 3600)
                    .ymd(2014, 10, 23)
                    .and_hms_micro(20, 3, 41, 636000)
                    .with_timezone(&Utc),
                update_time: FixedOffset::west(4 * 3600)
                    .ymd(2014, 10, 23)
                    .and_hms_micro(20, 3, 42, 890000)
                    .with_timezone(&Utc),
                notes: None,
                primary_route: "AUTO".to_string(),
                secondary_route: None,
                order_route: "LAMP".to_string(),
                venue_holding_order: None,
                commission_charged: json!(0).to_number(),
                exchange_order_id: "XS173577870".to_string(),
                is_significant_shareholder: false,
                is_insider: false,
                is_limit_offset_in_dollars: false,
                user_id: 3000124,
                placement_commission: json!(0).to_number(),
                strategy_type: "SingleLeg".to_string(),
                trigger_stop_price: None,
                order_group_id: 0,
                order_class: None
            })
        );

        Ok(())
    }

    #[tokio::test]
    async fn account_order_empty() -> Result<(), Box<dyn Error>> {
        let _m = mock("GET", "/v1/accounts/123456/orders/123456")
            .with_status(200)
            .with_header("content-type", "text/json")
            .with_body(read_to_string("test/response/account-order-empty.json")?)
            .create();

        let result = get_api().account_order("123456", 123456).await;

        assert_eq!(result?, None);

        Ok(())
    }

    #[tokio::test]
    async fn account_executions() -> Result<(), Box<dyn Error>> {
        let _m = mock("GET", "/v1/accounts/26598145/executions")
            .with_status(200)
            .with_header("content-type", "text/json")
            .with_body(read_to_string("test/response/account-executions.json")?)
            .create();

        let result = get_api().account_executions("26598145", None, None).await;

        assert_eq!(
            result?,
            vec![
                AccountExecution {
                    id: 53817310,
                    order_id: 177106005,
                    symbol: "AAPL".to_string(),
                    symbol_id: 8049,
                    quantity: json!(10).to_number(),
                    side: OrderSide::Buy,
                    price: json!(536.87).to_number(),
                    order_chain_id: 17710600,
                    timestamp: FixedOffset::west(4 * 3600)
                        .ymd(2014, 03, 31)
                        .and_hms(13, 38, 29)
                        .with_timezone(&Utc),
                    notes: None,
                    commission: json!(4.95).to_number(),
                    execution_fee: json!(0).to_number(),
                    sec_fee: json!(0).to_number(),
                    canadian_execution_fee: json!(0).to_number(),
                    parent_id: 0
                },
                AccountExecution {
                    id: 710654134,
                    order_id: 700046545,
                    symbol: "XSP.TO".to_string(),
                    symbol_id: 23963,
                    quantity: json!(3).to_number(),
                    side: OrderSide::Buy,
                    price: json!(36.52).to_number(),
                    order_chain_id: 700065471,
                    timestamp: FixedOffset::west(4 * 3600)
                        .ymd(2015, 08, 19)
                        .and_hms(11, 03, 41)
                        .with_timezone(&Utc),
                    notes: None,
                    commission: json!(0).to_number(),
                    execution_fee: json!(0.0105).to_number(),
                    sec_fee: json!(0).to_number(),
                    canadian_execution_fee: json!(0).to_number(),
                    parent_id: 710651321
                }
            ]
        );

        Ok(())
    }

    #[tokio::test]
    async fn account_balance() -> Result<(), Box<dyn Error>> {
        let _m = mock("GET", "/v1/accounts/26598145/balances")
            .with_status(200)
            .with_header("content-type", "text/json")
            .with_body(read_to_string("test/response/account-balances.json")?)
            .create();

        let result = get_api().account_balance("26598145").await;

        assert_eq!(
            result?,
            AccountBalances {
                per_currency_balances: vec![
                    AccountBalance {
                        currency: Currency::CAD,
                        cash: json!(322.7015).to_number(),
                        market_value: json!(6239.64).to_number(),
                        total_equity: json!(6562.3415).to_number(),
                        buying_power: json!(15473.182995).to_number(),
                        maintenance_excess: json!(4646.6015).to_number(),
                        is_real_time: true
                    },
                    AccountBalance {
                        currency: Currency::USD,
                        cash: json!(0).to_number(),
                        market_value: json!(0).to_number(),
                        total_equity: json!(0).to_number(),
                        buying_power: json!(0).to_number(),
                        maintenance_excess: json!(0).to_number(),
                        is_real_time: true
                    }
                ],
                combined_balances: vec![
                    AccountBalance {
                        currency: Currency::CAD,
                        cash: json!(322.7015).to_number(),
                        market_value: json!(6239.64).to_number(),
                        total_equity: json!(6562.3415).to_number(),
                        buying_power: json!(15473.182995).to_number(),
                        maintenance_excess: json!(4646.6015).to_number(),
                        is_real_time: true
                    },
                    AccountBalance {
                        currency: Currency::USD,
                        cash: json!(242.541526).to_number(),
                        market_value: json!(4689.695603).to_number(),
                        total_equity: json!(4932.237129).to_number(),
                        buying_power: json!(11629.600147).to_number(),
                        maintenance_excess: json!(3492.372416).to_number(),
                        is_real_time: true
                    }
                ],
                sod_per_currency_balances: vec![
                    AccountBalance {
                        currency: Currency::CAD,
                        cash: json!(322.7015).to_number(),
                        market_value: json!(6177).to_number(),
                        total_equity: json!(6499.7015).to_number(),
                        buying_power: json!(15473.182995).to_number(),
                        maintenance_excess: json!(4646.6015).to_number(),
                        is_real_time: true
                    },
                    AccountBalance {
                        currency: Currency::USD,
                        cash: json!(0).to_number(),
                        market_value: json!(0).to_number(),
                        total_equity: json!(0).to_number(),
                        buying_power: json!(0).to_number(),
                        maintenance_excess: json!(0).to_number(),
                        is_real_time: true
                    }
                ],
                sod_combined_balances: vec![
                    AccountBalance {
                        currency: Currency::CAD,
                        cash: json!(322.7015).to_number(),
                        market_value: json!(6177).to_number(),
                        total_equity: json!(6499.7015).to_number(),
                        buying_power: json!(15473.182995).to_number(),
                        maintenance_excess: json!(4646.6015).to_number(),
                        is_real_time: true
                    },
                    AccountBalance {
                        currency: Currency::USD,
                        cash: json!(242.541526).to_number(),
                        market_value: json!(4642.615558).to_number(),
                        total_equity: json!(4885.157084).to_number(),
                        buying_power: json!(11629.600147).to_number(),
                        maintenance_excess: json!(3492.372416).to_number(),
                        is_real_time: true
                    }
                ]
            }
        );

        Ok(())
    }

    #[tokio::test]
    async fn account_positions() -> Result<(), Box<dyn Error>> {
        let _m = mock("GET", "/v1/accounts/26598145/positions")
            .with_status(200)
            .with_header("content-type", "text/json")
            .with_body(read_to_string("test/response/account-positions.json")?)
            .create();

        let result = get_api().account_positions("26598145").await;

        assert_eq!(
            result?,
            vec![
                AccountPosition {
                    symbol: "THI.TO".to_string(),
                    symbol_id: 38738,
                    open_quantity: json!(100).to_number(),
                    closed_quantity: json!(0).to_number(),
                    current_market_value: json!(6017).to_number(),
                    current_price: json!(60.17).to_number(),
                    average_entry_price: json!(60.23).to_number(),
                    closed_profit_and_loss: json!(0).to_number(),
                    open_profit_and_loss: json!(-6).to_number(),
                    total_cost: json!(6023).to_number(),
                    is_real_time: true,
                    is_under_reorg: false
                },
                AccountPosition {
                    symbol: "XSP.TO".to_string(),
                    symbol_id: 38738,
                    open_quantity: json!(100).to_number(),
                    closed_quantity: json!(0).to_number(),
                    current_market_value: json!(3571).to_number(),
                    current_price: json!(35.71).to_number(),
                    average_entry_price: json!(32.831898).to_number(),
                    closed_profit_and_loss: json!(0).to_number(),
                    open_profit_and_loss: json!(500.789748).to_number(),
                    total_cost: json!(3070.750252).to_number(),
                    is_real_time: false,
                    is_under_reorg: false
                },
            ]
        );

        Ok(())
    }

    // endregion

    // region market
    #[tokio::test]
    async fn market_quote() -> Result<(), Box<dyn Error>> {
        let _m = mock("GET", "/v1/markets/quotes")
            .match_query(Matcher::UrlEncoded("ids".into(), "2434553,27725609".into()))
            .with_status(200)
            .with_header("content-type", "text/json")
            .with_body(read_to_string("test/response/market-quotes.json")?)
            .create();

        let result = get_api().market_quote(&[2434553, 27725609]).await;

        assert_eq!(
            result?,
            vec![
                MarketQuote {
                    symbol: "XMU.TO".to_string(),
                    symbol_id: 2434553,
                    tier: None,
                    bid_price: Some(json!(57.01).to_number()),
                    bid_size: 24,
                    ask_price: Some(json!(57.13).to_number()),
                    ask_size: 33,
                    last_trade_price_tr_hrs: json!(57.15).to_number(),
                    last_trade_price: json!(57.15).to_number(),
                    last_trade_size: 100,
                    last_trade_tick: TickType::Up,
                    volume: 2728,
                    open_price: json!(55.76).to_number(),
                    high_price: json!(57.15).to_number(),
                    low_price: json!(55.76).to_number(),
                    delay: false,
                    is_halted: false
                },
                MarketQuote {
                    symbol: "XMU.U.TO".to_string(),
                    symbol_id: 27725609,
                    tier: None,
                    bid_price: Some(json!(42.65).to_number()),
                    bid_size: 10,
                    ask_price: Some(json!(42.79).to_number()),
                    ask_size: 10,
                    last_trade_price_tr_hrs: json!(44.22).to_number(),
                    last_trade_price: json!(44.22).to_number(),
                    last_trade_size: 0,
                    last_trade_tick: TickType::Equal,
                    volume: 0,
                    open_price: json!(0).to_number(),
                    high_price: json!(0).to_number(),
                    low_price: json!(0).to_number(),
                    delay: false,
                    is_halted: false
                }
            ]
        );

        Ok(())
    }

    #[tokio::test]
    async fn symbol_search() -> Result<(), Box<dyn Error>> {
        let _m = mock("GET", "/v1/symbols/search?prefix=V&offset=0")
            .with_status(200)
            .with_header("content-type", "text/json")
            .with_body(read_to_string("test/response/symbol-search.json")?)
            .create();

        let result = get_api().symbol_search("V", 0).await;

        assert_eq!(
            result?,
            vec![
                SearchEquitySymbol {
                    symbol: "V".into(),
                    symbol_id: 40825,
                    description: "VISA INC".into(),
                    security_type: SecurityType::Stock,
                    listing_exchange: ListingExchange::NYSE,
                    is_quotable: true,
                    is_tradable: true,
                    currency: Currency::USD
                },
                SearchEquitySymbol {
                    symbol: "VA.TO".into(),
                    symbol_id: 11419773,
                    description: "VANGUARD FTSE DEV ASIA PAC ALL CAP IDX".into(),
                    security_type: SecurityType::Stock,
                    listing_exchange: ListingExchange::TSX,
                    is_quotable: true,
                    is_tradable: true,
                    currency: Currency::CAD
                },
                SearchEquitySymbol {
                    symbol: "VABB".into(),
                    symbol_id: 40790,
                    description: "VIRGINIA BANK BANKSHARES INC".into(),
                    security_type: SecurityType::Stock,
                    listing_exchange: ListingExchange::PinkSheets,
                    is_quotable: true,
                    is_tradable: true,
                    currency: Currency::USD
                },
                SearchEquitySymbol {
                    symbol: "VAC".into(),
                    symbol_id: 1261992,
                    description: "MARRIOTT VACATIONS WORLDWIDE CORP".into(),
                    security_type: SecurityType::Stock,
                    listing_exchange: ListingExchange::NYSE,
                    is_quotable: true,
                    is_tradable: true,
                    currency: Currency::USD
                },
                SearchEquitySymbol {
                    symbol: "VACNY".into(),
                    symbol_id: 20491473,
                    description: "VAT GROUP AG".into(),
                    security_type: SecurityType::Stock,
                    listing_exchange: ListingExchange::PinkSheets,
                    is_quotable: true,
                    is_tradable: true,
                    currency: Currency::USD
                },
                SearchEquitySymbol {
                    symbol: "VACQU".into(),
                    symbol_id: 32441174,
                    description: "VECTOR ACQUISITION CORP UNITS(1 ORD A & 1/3 WT)30/09/2027".into(),
                    security_type: SecurityType::Stock,
                    listing_exchange: ListingExchange::NASDAQ,
                    is_quotable: true,
                    is_tradable: true,
                    currency: Currency::USD
                },
                SearchEquitySymbol {
                    symbol: "VAEEM.IN".into(),
                    symbol_id: 1630037,
                    description: "CBOE VXEEM Ask Index".into(),
                    security_type: SecurityType::Index,
                    listing_exchange: ListingExchange::SP,
                    is_quotable: true,
                    is_tradable: false,
                    currency: Currency::USD
                }
            ]
        );

        Ok(())
    }

    // endregion
}
