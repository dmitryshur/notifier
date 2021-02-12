use actix_web::{http::StatusCode, test, web, App};
use api::{create_handler, AppState, INVALID_INTERVAL, INVALID_SCRIPT, INVALID_URL};
use async_trait::async_trait;
use broker::{Broker, BrokerErrors, Consumer, Exchanges, Messages};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CreateResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

struct MockBroker {
    sent_msgs: Arc<Mutex<HashMap<Exchanges, Vec<Messages>>>>,
}

impl MockBroker {
    fn new() -> Self {
        Self {
            sent_msgs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl Broker for MockBroker {
    async fn publish(&self, exchange: Exchanges, message: Messages) -> Result<(), BrokerErrors> {
        let sent_msgs = Arc::clone(&self.sent_msgs);
        let mut lock = sent_msgs.lock();
        let msgs = lock.get_mut(&exchange);
        match msgs {
            Some(msgs) => msgs.push(message),
            None => {
                lock.insert(exchange, vec![message]);
            }
        }

        Ok(())
    }

    async fn subscribe(&self, _exchange: Exchanges) -> Result<Consumer, BrokerErrors> {
        unimplemented!()
    }
}

fn configure(cfg: &mut web::ServiceConfig) {
    let state = AppState {
        broker: Arc::new(Mutex::new(MockBroker::new())),
    };

    cfg.data(state)
        .route("/create", web::post().to(create_handler::<MockBroker>));
}

#[actix_rt::test]
async fn create_empty_request() {
    let mut app = test::init_service(App::new().configure(configure)).await;
    let request = test::TestRequest::post().uri("/create").to_request();
    let response = test::call_service(&mut app, request).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST, "Response: {:?}", response);
}

#[actix_rt::test]
async fn create_empty_url() {
    let mut app = test::init_service(App::new().configure(configure)).await;
    let body = json!({"url": "", "interval": 5, "script": "qwerty"});
    let request = test::TestRequest::post().uri("/create").set_json(&body).to_request();
    let response = test::call_service(&mut app, request).await;

    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "Response: {:?}",
        response
    );

    let response: CreateResponse = test::read_body_json(response).await;
    assert_eq!(response.error.is_some(), true);
    assert_eq!(response.error.unwrap(), INVALID_URL);
}

#[actix_rt::test]
async fn create_empty_script() {
    let mut app = test::init_service(App::new().configure(configure)).await;
    let body = json!({"url": "https://google.com", "interval": 5, "script": ""});
    let request = test::TestRequest::post().uri("/create").set_json(&body).to_request();
    let response = test::call_service(&mut app, request).await;

    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "Response: {:?}",
        response
    );

    let response: CreateResponse = test::read_body_json(response).await;
    assert_eq!(response.error.is_some(), true);
    assert_eq!(response.error.unwrap(), INVALID_SCRIPT);
}

#[actix_rt::test]
async fn create_invalid_interval() {
    let mut app = test::init_service(App::new().configure(configure)).await;
    let body = json!({"url": "https://google.com", "interval": 2, "script": "qwerty"});
    let request = test::TestRequest::post().uri("/create").set_json(&body).to_request();
    let response = test::call_service(&mut app, request).await;

    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "Response: {:?}",
        response
    );

    let response: CreateResponse = test::read_body_json(response).await;
    assert_eq!(response.error.is_some(), true);
    assert_eq!(response.error.unwrap(), INVALID_INTERVAL);
}

#[actix_rt::test]
async fn create_all_fields_invalid() {
    let mut app = test::init_service(App::new().configure(configure)).await;
    let body = json!({"url": "", "interval": 2, "script": ""});
    let request = test::TestRequest::post().uri("/create").set_json(&body).to_request();
    let response = test::call_service(&mut app, request).await;

    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "Response: {:?}",
        response
    );

    let expected_error = format!(
        "{}. {}. {}",
        String::from(INVALID_URL),
        String::from(INVALID_INTERVAL),
        String::from(INVALID_SCRIPT)
    );
    let response: CreateResponse = test::read_body_json(response).await;
    assert_eq!(response.error.is_some(), true);
    assert_eq!(response.error.unwrap(), expected_error);
}

#[actix_rt::test]
async fn create_success() {
    let broker = Arc::new(Mutex::new(MockBroker::new()));
    let state = AppState {
        broker: Arc::clone(&broker),
    };
    let mut app = test::init_service(App::new().data(state).configure(configure)).await;
    let body = json!({"url": "https://google.com", "interval": 5, "script": "qwerty"});
    let request = test::TestRequest::post().uri("/create").set_json(&body).to_request();
    let response = test::call_service(&mut app, request).await;

    assert_eq!(response.status(), StatusCode::OK, "Response: {:?}", response);

    let response: CreateResponse = test::read_body_json(response).await;
    assert_eq!(response.id.is_some(), true);
    assert_eq!(Uuid::from_str(response.id.clone().unwrap().as_str()).is_ok(), true);

    let broker_lock = broker.lock();
    let sent_msgs_lock = broker_lock.sent_msgs.lock();
    let msgs = sent_msgs_lock.get(&Exchanges::Scheduler);

    assert_eq!(msgs.is_some(), true);
    assert_eq!(msgs.unwrap().len(), 1);

    let sent_msg = msgs.unwrap().get(0).unwrap();
    match sent_msg {
        Messages::Create {
            id,
            interval,
            script,
            url,
        } => {
            assert_eq!(*id, response.id.unwrap());
            assert_eq!(*url, String::from("https://google.com"));
            assert_eq!(*script, String::from("qwerty"));
            assert_eq!(*interval, 5)
        }
        _ => {
            panic!("sent message was not of expected type Messages::Create")
        }
    }
}
