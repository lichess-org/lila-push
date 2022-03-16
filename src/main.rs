#![feature(try_blocks)]

use std::{collections::BTreeMap, fs, io::Cursor, net::SocketAddr, path::PathBuf, time::Duration};

use axum::{routing::post, Json, Router};
use clap::Parser;
use serde::Deserialize;
use tokio::time::timeout;
use web_push::{
    ContentEncoding::Aes128Gcm, SubscriptionInfo, VapidSignatureBuilder, WebPushClient,
    WebPushError, WebPushMessageBuilder,
};

#[derive(Parser, Debug)]
struct Opt {
    /// PEM file with private VAPID key.
    #[clap(long, parse(from_os_str))]
    vapid: PathBuf,
    /// VAPID subject (example: mailto:contact@lichess.org).
    #[clap(long)]
    subject: String,

    /// Listen on this socket address.
    #[clap(long, default_value = "127.0.0.1:9054")]
    bind: SocketAddr,
}

struct App {
    client: WebPushClient,
    vapid: Vec<u8>,
    subject: String,
}

#[derive(Deserialize, Debug)]
struct PushRequest {
    subs: Vec<SubscriptionInfo>,
    payload: String,
    ttl: u32,
}

async fn push(
    app: &'static App,
    Json(req): Json<PushRequest>,
) -> Json<BTreeMap<String, &'static str>> {
    let mut res: BTreeMap<String, &'static str> = BTreeMap::new();

    for sub in &req.subs {
        let result: Result<(), WebPushError> = try {
            let mut signature = VapidSignatureBuilder::from_pem(Cursor::new(&app.vapid), sub)?;
            signature.add_claim("sub", app.subject.clone());

            let mut builder = WebPushMessageBuilder::new(sub)?;
            builder.set_ttl(req.ttl);
            builder.set_payload(Aes128Gcm, req.payload.as_bytes());
            builder.set_vapid_signature(signature.build()?);
            let message = builder.build()?;

            timeout(Duration::from_secs(15), app.client.send(message))
                .await
                .map_err(|_| WebPushError::Other("timeout".to_owned()))??;
        };

        res.insert(
            sub.endpoint.clone(),
            result.err().map_or("ok", |e| e.short_description()),
        );
    }

    Json(res)
}

#[tokio::main]
async fn main() {
    let opt = Opt::parse();

    let app: &'static App = Box::leak(Box::new(App {
        client: WebPushClient::new().expect("web push client"),
        vapid: fs::read(opt.vapid).expect("vapid key"),
        subject: opt.subject,
    }));

    let app = Router::new().route("/", post(move |req| push(app, req)));

    axum::Server::bind(&opt.bind)
        .serve(app.into_make_service())
        .await
        .expect("bind");
}
