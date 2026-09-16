use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use pay3flow_backend::acquirer::ACQUIRERS;
use pay3flow_backend::activitypub::actor::ActorIdentity;
use pay3flow_backend::activitypub::model::{accept_follow, follow_activity, solver_answer};
use pay3flow_backend::activitypub::signature::sign_headers;

/// Each fake acquirer is its own ActivityPub actor-solver: own actor IRI, own
/// key, own Follow handshake + `purpose="offer"` proposal, and an inbox that
/// answers dispatched tickets with a mock solution. No database is used.
#[derive(Clone)]
struct FakeActor {
    identity: ActorIdentity,
}

#[derive(Clone)]
struct FakeState {
    origin: String,
    fmatch_inbox: String,
    fmatch_actor_id: String,
    resource: String,
    actors: Arc<HashMap<String, FakeActor>>,
    posted: Arc<Mutex<std::collections::HashSet<String>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let http_addr = std::env::var("HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:9090".into());
    let origin = std::env::var("AP_ORIGIN").unwrap_or_else(|_| "http://localhost:9090".into());
    let fmatch_inbox = std::env::var("FMATCH_INBOX")
        .unwrap_or_else(|_| "http://localhost:7277/inbox/actra".into());
    let fmatch_actor_id = std::env::var("FMATCH_ACTOR_ID")
        .unwrap_or_else(|_| "http://localhost:7277/actor/actra".into());
    let key_path = std::env::var("AP_KEY_PATH").unwrap_or_else(|_| "/app/data/fake-key.pem".into());

    let resource = format!("{origin}/marketplace/resources/acquiring");
    let shared_inbox = format!("{origin}/inbox");

    let mut actors: HashMap<String, FakeActor> = HashMap::new();
    for seed in ACQUIRERS {
        let key = format!("{key_path}.{}", seed.slug);
        let identity = ActorIdentity::load_or_create(&key, &origin, seed.slug)?;
        actors.insert(seed.slug.to_string(), FakeActor { identity });
    }

    let state = FakeState {
        origin,
        fmatch_inbox,
        fmatch_actor_id,
        resource,
        actors: Arc::new(actors),
        posted: Arc::new(Mutex::new(std::collections::HashSet::new())),
    };

    let seed_state = state.clone();
    let shared_inbox_seed = shared_inbox.clone();
    tokio::spawn(async move {
        for seed in ACQUIRERS {
            let actor = seed_state
                .actors
                .get(seed.slug)
                .map(|a| a.identity.clone())
                .expect("fake actor exists");
            let follow = follow_activity(
                &actor.actor_id,
                &actor.actor_id,
                &seed_state.fmatch_actor_id,
            );
            let _ = deliver_signed(&seed_state, &actor, &seed_state.fmatch_inbox, &follow).await;
            let offer = seed.to_proposal(&actor.actor_id, &seed_state.resource, &shared_inbox_seed);
            let _ = deliver_signed(
                &seed_state,
                &actor,
                &seed_state.fmatch_inbox,
                &offer.to_activity(),
            )
            .await;
            tracing::info!(actor = seed.slug, "fake solver seeded");
        }
    });

    tracing::info!(origin = %state.origin, "fake solvers listening on http://{}", http_addr);

    let app = Router::new()
        .route("/actor/:handle", get(actor_document))
        .route("/inbox", post(inbox))
        .route("/inbox/:handle", post(inbox))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&http_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn actor_document(
    State(state): State<FakeState>,
    Path(handle): Path<String>,
) -> axum::response::Response {
    match state.actors.get(&handle) {
        Some(actor) => Json(actor.identity.to_document()).into_response(),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({"error": "fake actor not found"})),
        )
            .into_response(),
    }
}

/// Fake inbox: answer Follow with Accept and a dispatched Ticket with a mock
/// solution (mirrors the real backend solver inbox so either can be selected).
async fn inbox(State(state): State<FakeState>, body: String) -> axum::response::Response {
    let activity: Value = match serde_json::from_str(&body) {
        Ok(activity) => activity,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({"error": "bad json"})),
            )
                .into_response()
        }
    };
    let atype = activity.get("type").and_then(Value::as_str).unwrap_or("");
    let object_type = activity
        .get("object")
        .and_then(|o| o.get("type"))
        .and_then(Value::as_str);

    match (atype, object_type) {
        ("Follow", _) => {
            let actor_id = state
                .actors
                .values()
                .next()
                .map(|a| a.identity.actor_id.clone())
                .unwrap_or_default();
            Json(accept_follow(&activity, &actor_id)).into_response()
        }
        ("Create", Some("Ticket")) => {
            let object = activity.get("object").cloned().unwrap_or_default();
            let task_ref = ["taskRef", "task_ref", "task", "id"]
                .iter()
                .find_map(|key| object.get(*key).and_then(Value::as_str))
                .unwrap_or("unknown-task")
                .to_string();
            let actor_id = state
                .actors
                .values()
                .next()
                .map(|a| a.identity.actor_id.clone())
                .unwrap_or_default();
            Json(solver_answer(
                &actor_id,
                &task_ref,
                activity.get("id").and_then(Value::as_str).unwrap_or(""),
                &format!("Mock solution for task {task_ref}: booked by fake solver"),
            ))
            .into_response()
        }
        _ => Json(json!({"status": "accepted"})).into_response(),
    }
}

async fn deliver_signed(
    state: &FakeState,
    identity: &ActorIdentity,
    inbox: &str,
    activity: &Value,
) -> anyhow::Result<()> {
    let activity_id = activity
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    {
        let mut posted = state.posted.lock().await;
        if !posted.insert(activity_id.clone()) {
            return Ok(());
        }
    }

    let parsed: reqwest::Url = inbox.parse()?;
    let host = parsed
        .host_str()
        .map(|h| match parsed.port() {
            Some(port) => format!("{h}:{port}"),
            None => h.to_string(),
        })
        .unwrap_or_default();
    let uri = parsed.path().to_string();
    let body = serde_json::to_vec(activity)?;
    let headers = sign_headers(identity, "POST", &uri, &host, &body, chrono::Utc::now())?;

    let client = reqwest::Client::new();
    let mut request = client.post(inbox).body(body);
    for (key, value) in headers {
        request = request.header(key, value);
    }
    let response = request
        .header(
            axum::http::header::CONTENT_TYPE,
            pay3flow_backend::activitypub::ACTIVITY_JSON,
        )
        .send()
        .await?;
    tracing::debug!(activity = %activity_id, status = ?response.status(), "delivered");
    Ok(())
}
