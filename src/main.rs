use std::{
    cmp::max, collections::BTreeMap, fs, io::Cursor, net::SocketAddr, path::PathBuf, time::Duration,
};

use axum::{routing::post, Json, Router};
use clap::{builder::PathBufValueParser, Parser};
use serde::Deserialize;
use tokio::{net::TcpListener, time::timeout};
use web_push::{
    ContentEncoding::Aes128Gcm, HyperWebPushClient, SubscriptionInfo, Urgency,
    VapidSignatureBuilder, WebPushClient, WebPushError, WebPushMessageBuilder,
};

#[derive(Parser, Debug)]
struct Opt {
    /// PEM file with private VAPID key.
    #[arg(long, value_parser = PathBufValueParser::new())]
    vapid: PathBuf,
    /// VAPID subject (example: mailto:contact@lichess.org).
    #[arg(long)]
    subject: String,

    /// Listen on this socket address.
    #[arg(long, default_value = "127.0.0.1:9054")]
    bind: SocketAddr,
}

struct App {
    client: HyperWebPushClient,
    vapid: Vec<u8>,
    subject: String,
}

#[derive(Deserialize, Debug)]
struct PushRequest {
    subs: Vec<SubscriptionInfo>,
    #[serde(flatten)]
    push: Push,
}

#[derive(Deserialize, Debug)]
struct Push {
    payload: String,
    ttl: u32,
    #[serde(default)]
    urgency: Option<Urgency>,
    #[serde(default)]
    topic: Option<String>,
}

async fn push_single(app: &App, sub: &SubscriptionInfo, push: &Push) -> Result<(), WebPushError> {
    let mut signature = VapidSignatureBuilder::from_pem(Cursor::new(&app.vapid), sub)?;
    signature.add_claim("sub", app.subject.clone());

    let mut builder = WebPushMessageBuilder::new(sub);
    builder.set_payload(Aes128Gcm, push.payload.as_bytes());
    builder.set_ttl(push.ttl);
    if let Some(urgency) = push.urgency {
        builder.set_urgency(max(Urgency::Low, urgency)); // fcm does not support very-low
    }
    if let Some(ref topic) = push.topic {
        builder.set_topic(topic.to_owned());
    }
    builder.set_vapid_signature(signature.build()?);

    app.client.send(builder.build()?).await
}

async fn push(
    app: &'static App,
    Json(req): Json<PushRequest>,
) -> Json<BTreeMap<String, &'static str>> {
    let mut res: BTreeMap<String, &'static str> = BTreeMap::new();

    let mut oks = 0usize;
    let mut errs = 0usize;

    for sub in &req.subs {
        res.insert(
            sub.endpoint.clone(),
            match timeout(Duration::from_secs(15), push_single(app, sub, &req.push)).await {
                Ok(Ok(())) => {
                    oks += 1;
                    "ok"
                }
                Ok(Err(e)) => {
                    errs += 1;
                    log::warn!("{}: {:?}", sub.endpoint, e);
                    e.short_description()
                }
                Err(timeout) => {
                    errs += 1;
                    log::error!("{}: {:?}", sub.endpoint, timeout);
                    "timeout"
                }
            },
        );
    }

    log::info!("=> {} ok, {} errors", oks, errs);

    Json(res)
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::new()
            .filter("PUSH_LOG")
            .write_style("PUSH_LOG_STYLE"),
    )
    .format_timestamp(None)
    .format_module_path(false)
    .format_target(false)
    .init();

    let opt = Opt::parse();

    let app: &'static App = Box::leak(Box::new(App {
        client: HyperWebPushClient::new(),
        vapid: fs::read(opt.vapid).expect("vapid key"),
        subject: opt.subject,
    }));

    let app = Router::new().route("/", post(move |req| push(app, req)));

    let listener = TcpListener::bind(&opt.bind).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}
