#![feature(async_await, await_macro)]

#![warn(rust_2018_idioms)]

use serde::Deserialize;
use std::net::SocketAddr;
use structopt::StructOpt;
use futures::compat::Future01CompatExt as _;
use tide::{Context, EndpointResult};
use tide::error::ResultExt;
use tide::http::status::StatusCode;
use web_push::{WebPushClient, WebPushMessageBuilder, SubscriptionInfo, VapidSignatureBuilder};
use web_push::ContentEncoding::AesGcm;

// TODO: UNWRAP

#[derive(Deserialize, Debug)]
struct PushRequest {
    vapid: Vapid,
    sub: SubscriptionInfo,
    payload: String,
    ttl: u32,
}

#[derive(Deserialize, Debug)]
struct Vapid {
    subject: String,
    public: String,
    private: String,
}

#[derive(StructOpt, Debug)]
struct Opt {
    /// Listen on this address.
    #[structopt(long = "address", default_value = "127.0.0.1")]
    address: String,
    /// Listen on this port.
    #[structopt(long = "port", default_value = "9054")]
    port: u16,
}

async fn push(mut cx: Context<WebPushClient>) -> EndpointResult<StatusCode> {
    let client = cx.app_data();
    let req: PushRequest = await!(cx.body_json()).client_err()?;

    let mut signature: VapidSignatureBuilder<'_> = unimplemented!();
    signature.add_claim("sub", req.vapid.subject);

    let mut builder = WebPushMessageBuilder::new(&req.sub).unwrap(); // XXX
    builder.set_ttl(req.ttl);
    builder.set_payload(AesGcm, req.payload.as_bytes());
    builder.set_vapid_signature(signature.build().unwrap()); // XXX
    let message = builder.build().unwrap(); // XXX

    await!(client.send(message).compat()).unwrap(); // XXX
    Ok(StatusCode::NO_CONTENT)
}

fn main() {
    let opt = Opt::from_args();
    let bind = SocketAddr::new(opt.address.parse().expect("valid address"), opt.port);

    let client = WebPushClient::new().expect("push client");

    let mut app = tide::App::new(client);
    app.at("/").post(push);
    app.serve(bind).expect("bind");
}
