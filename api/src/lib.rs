use actix_web::{self, body::Body, dev, error, http::StatusCode, web, HttpResponse};
use broker::{Broker, BrokerErrors, Exchanges, Messages};
use log::warn;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::ops::RangeInclusive;
use std::sync::Arc;

const MIN_INTERVAL: u64 = 5;
const MAX_INTERVAL: u64 = 604_800; // Week in seconds
const INTERVAL_RANGE: RangeInclusive<u64> = MIN_INTERVAL..=MAX_INTERVAL;

pub const INVALID_INTERVAL: &str = "Interval must be in range 5-604,800 (week in seconds) and a multiple of 5";
pub const INVALID_URL: &str = "URL must not be empty and should be valid";
pub const INVALID_SCRIPT: &str = "Script can't be empty";

#[derive(Debug)]
pub enum ApiErrors {
    Server(BrokerErrors),
    Validation(Vec<&'static str>),
}

impl std::error::Error for ApiErrors {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Server(error) => Some(error),
            Self::Validation(_) => None,
        }
    }
}

impl fmt::Display for ApiErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Server(error) => write!(f, "Internal server error. {}", error),
            Self::Validation(errors) => {
                let err = errors.join("\n");
                f.write_str(&err)
            }
        }
    }
}

impl From<BrokerErrors> for ApiErrors {
    fn from(error: BrokerErrors) -> Self {
        Self::Server(error)
    }
}

// TODO logs are needed
impl error::ResponseError for ApiErrors {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Server(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }

    fn error_response(&self) -> HttpResponse<Body> {
        let res;

        match self {
            Self::Server(_) => {
                res = CreateResponse {
                    id: None,
                    error: Some(String::from("Internal server error. try again.")),
                }
            }
            Self::Validation(errors) => {
                res = CreateResponse {
                    id: None,
                    error: Some(errors.join(". ")),
                }
            }
        }

        dev::HttpResponseBuilder::new(self.status_code()).json(res)
    }
}

pub trait Validate {
    type Error: std::error::Error + Into<ApiErrors>;

    fn validate(&self) -> Result<(), Self::Error>;
}

#[derive(Deserialize)]
pub struct CreateRequest {
    url: String,
    interval: u64,
    script: String,
}

impl Validate for CreateRequest {
    type Error = ApiErrors;

    fn validate(&self) -> Result<(), Self::Error> {
        let mut errors = Vec::new();

        // Validation URL is problematic. if the URL is invalid, so be it. the scraper will simply
        // fail
        if self.url.is_empty() {
            errors.push(INVALID_URL)
        }

        if self.interval % 5 != 0 || !INTERVAL_RANGE.contains(&self.interval) {
            errors.push(INVALID_INTERVAL)
        }

        if self.script.is_empty() {
            errors.push(INVALID_SCRIPT)
        }

        if !errors.is_empty() {
            return Err(ApiErrors::Validation(errors));
        }

        Ok(())
    }
}

#[derive(Serialize)]
struct CreateResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub struct AppState<T>
where
    T: Broker,
{
    pub broker: Arc<Mutex<T>>,
}

pub async fn create_handler<T>(
    body: web::Json<CreateRequest>,
    state: web::Data<AppState<T>>,
) -> Result<HttpResponse, ApiErrors>
where
    T: Broker,
{
    let body = body.into_inner();
    body.validate()?;

    let id = uuid::Uuid::new_v4();
    let msg = Messages::Create {
        id: id.to_string(),
        url: body.url,
        script: body.script,
        interval: body.interval,
    };
    state.broker.lock().publish(Exchanges::Scheduler, msg).await?;

    Ok(HttpResponse::Ok().json(CreateResponse {
        id: Some(id.to_string()),
        error: None,
    }))
}
