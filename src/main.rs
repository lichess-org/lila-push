use std::{collections::BTreeMap, fs, io::Cursor, net::SocketAddr, path::PathBuf, time::Duration};

use axum::{routing::post, Json, Router};
use clap::{builder::PathBufValueParser, Parser};
use serde::Deserialize;
use tokio::time::timeout;
use web_push::{
    ContentEncoding::Aes128Gcm, HyperWebPushClient, SubscriptionInfo, VapidSignatureBuilder,
    WebPushClient, WebPushError, WebPushMessageBuilder,
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
}

async fn push_single(app: &App, sub: &SubscriptionInfo, push: &Push) -> Result<(), WebPushError> {
    let mut signature = VapidSignatureBuilder::from_pem(Cursor::new(&app.vapid), sub)?;
    signature.add_claim("sub", app.subject.clone());

    let mut builder = WebPushMessageBuilder::new(sub);
    builder.set_ttl(push.ttl);
    builder.set_payload(Aes128Gcm, push.payload.as_bytes());
    builder.set_vapid_signature(signature.build()?);

    timeout(Duration::from_secs(15), app.client.send(builder.build()?))
        .await
        .map_err(|_| WebPushError::Other("timeout".to_owned()))?
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
            match push_single(app, sub, &req.push).await {
                Ok(()) => {
                    oks += 1;
                    "ok"
                }
                Err(e) => {
                    errs += 1;
                    log::warn!("{}: {}", sub.endpoint, e.short_description());
                    e.short_description()
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

    axum::Server::bind(&opt.bind)
        .serve(app.into_make_service())
        .await
        .expect("bind");
}
