use std::{
    cmp::max, collections::BTreeMap, fs::File, net::SocketAddr, path::PathBuf, time::Duration,
};

use axum::{routing::post, Json, Router};
use clap::{builder::PathBufValueParser, Parser};
use listenfd::ListenFd;
use serde::Deserialize;
use tokio::{net::TcpListener, time::timeout};
use web_push::{
    ContentEncoding::Aes128Gcm, HyperWebPushClient, PartialVapidSignatureBuilder, SubscriptionInfo,
    Urgency, VapidSignatureBuilder, WebPushClient, WebPushError, WebPushMessageBuilder,
};

#[derive(Parser, Debug)]
struct Opt {
    /// PEM file with private VAPID key.
    #[arg(long, value_parser = PathBufValueParser::new())]
    vapid: PathBuf,
    /// VAPID subject (example: mailto:contact@lichess.org).
    #[arg(long, alias = "subject")]
    vapid_subject: String,

    /// Listen on this socket address.
    #[arg(long, default_value = "127.0.0.1:9054")]
    bind: SocketAddr,
}

struct App {
    client: HyperWebPushClient,
    vapid_subject: String,
    vapid_signature_builder: PartialVapidSignatureBuilder,
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
    let mut builder = WebPushMessageBuilder::new(sub);
    builder.set_payload(Aes128Gcm, push.payload.as_bytes());
    builder.set_ttl(push.ttl);
    if let Some(urgency) = push.urgency {
        builder.set_urgency(max(Urgency::Low, urgency)); // fcm does not support very-low
    }
    if let Some(ref topic) = push.topic {
        builder.set_topic(topic.to_owned());
    }

    let mut signature_builder = app.vapid_signature_builder.clone().add_sub_info(sub);
    signature_builder.add_claim("sub", app.vapid_subject.clone());
    builder.set_vapid_signature(signature_builder.build()?);

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
            match timeout(Duration::from_secs(10), push_single(app, sub, &req.push)).await {
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
        vapid_subject: opt.vapid_subject,
        vapid_signature_builder: VapidSignatureBuilder::from_pem_no_sub(
            File::open(opt.vapid).expect("open vapid pem"),
        )
        .expect("read vapid pem"),
    }));

    let app = Router::new().route("/", post(move |req| push(app, req)));

    let listener = match ListenFd::from_env()
        .take_tcp_listener(0)
        .expect("tcp listener")
    {
        Some(std_listener) => {
            std_listener.set_nonblocking(true).expect("set nonblocking");
            TcpListener::from_std(std_listener).expect("listener")
        }
        None => TcpListener::bind(&opt.bind).await.expect("bind"),
    };

    axum::serve(listener, app).await.expect("serve");
}
