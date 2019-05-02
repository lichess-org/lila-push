#![feature(async_await, await_macro, try_blocks)]

#![warn(rust_2018_idioms)]

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use structopt::StructOpt;
use futures::compat::Future01CompatExt as _;
use tide::{Context, EndpointResult, response};
use tide::error::ResultExt;
use tide::middleware::RootLogger;
use web_push::{WebPushClient, WebPushMessageBuilder, SubscriptionInfo, VapidSignatureBuilder, WebPushError};
use web_push::ContentEncoding::AesGcm;

#[derive(StructOpt, Debug)]
struct Opt {
    /// PEM file with private VAPID key
    #[structopt(long = "vapid", parse(from_os_str))]
    vapid: PathBuf,
    /// VAPID subject (example: mailto:contact@lichess.org)
    #[structopt(long = "subject")]
    subject: String,

    /// Listen on this address
    #[structopt(long = "address", default_value = "127.0.0.1")]
    address: String,
    /// Listen on this port
    #[structopt(long = "port", default_value = "9054")]
    port: u16,
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

async fn push(mut cx: Context<App>) -> EndpointResult {
    let req: PushRequest = await!(cx.body_json()).client_err()?;
    let app = cx.app_data();

    let mut res: BTreeMap<String, &'static str> = BTreeMap::new();

    for sub in &req.subs {
        let result: Result<(), WebPushError> = try {
            let mut signature = VapidSignatureBuilder::from_pem(Cursor::new(&app.vapid), sub)?;
            signature.add_claim("sub", app.subject.clone());

            let mut builder = WebPushMessageBuilder::new(sub)?;
            builder.set_ttl(req.ttl);
            builder.set_payload(AesGcm, req.payload.as_bytes());
            builder.set_vapid_signature(signature.build()?);
            let message = builder.build()?;

            let timeout = Duration::from_secs(15);
            await!(app.client.send_with_timeout(message, timeout).compat())?;
        };

        res.insert(sub.endpoint.clone(), result.err().map_or("ok", |e| e.short_description()));
    }

    Ok(response::json(res))
}

fn main() {
    let opt = Opt::from_args();
    let bind = SocketAddr::new(opt.address.parse().expect("valid address"), opt.port);

    let app = App {
        client: WebPushClient::new().expect("push client"),
        vapid: fs::read(opt.vapid).expect("vapid key"),
        subject: opt.subject,
    };

    let mut app = tide::App::new(app);
    app.middleware(RootLogger::new());
    app.at("/").post(push);
    app.serve(bind).expect("bind");
}
