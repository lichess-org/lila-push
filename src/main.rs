#![feature(async_await, await_macro)]

#![warn(rust_2018_idioms)]

use serde::Deserialize;
use std::fs;
use std::io::Cursor;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use structopt::StructOpt;
use futures::compat::Future01CompatExt as _;
use tide::{Context, Response};
use tide::response::IntoResponse;
use tide::http::status::StatusCode;
use web_push::{WebPushClient, WebPushMessageBuilder, SubscriptionInfo, VapidSignatureBuilder, WebPushError};
use web_push::ContentEncoding::AesGcm;

#[derive(StructOpt, Debug)]
struct Opt {
    /// PEM file with private VAPID key.
    #[structopt(long = "vapid", parse(from_os_str))]
    vapid: PathBuf,
    /// VAPID subject (example: mailto:contact@lichess.org).
    #[structopt(long = "subject")]
    subject: String,

    /// Listen on this address.
    #[structopt(long = "address", default_value = "127.0.0.1")]
    address: String,
    /// Listen on this port.
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
    sub: SubscriptionInfo,
    payload: String,
    ttl: u32,
}

struct PushError(WebPushError);

impl From<WebPushError> for PushError {
    fn from(err: WebPushError) -> PushError {
        PushError(err)
    }
}

impl IntoResponse for PushError {
    fn into_response(self) -> Response {
        "error".with_status(StatusCode::BAD_REQUEST).into_response()
    }
}

async fn push(mut cx: Context<App>) -> Result<StatusCode, PushError> {
    let req: PushRequest = await!(cx.body_json()).map_err(|_| WebPushError::BadRequest(None))?;
    let app = cx.app_data();

    let mut signature = VapidSignatureBuilder::from_pem(Cursor::new(&app.vapid), &req.sub)?;
    signature.add_claim("sub", app.subject.clone());

    let mut builder = WebPushMessageBuilder::new(&req.sub)?;
    builder.set_ttl(req.ttl);
    builder.set_payload(AesGcm, req.payload.as_bytes());
    builder.set_vapid_signature(signature.build()?);
    let message = builder.build()?;

    let timeout = Duration::from_secs(15);
    await!(app.client.send_with_timeout(message, timeout).compat())?;
    Ok(StatusCode::NO_CONTENT)
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
    app.at("/").post(push);
    app.serve(bind).expect("bind");
}
