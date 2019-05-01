#![feature(async_await, await_macro)]

#![warn(rust_2018_idioms)]

use std::net::SocketAddr;
use structopt::StructOpt;
use tide::{Context, EndpointResult};
use tide::http::status::StatusCode;

#[derive(StructOpt, Debug)]
struct Opt {
    /// Listen on this address.
    #[structopt(long = "address", default_value = "127.0.0.1")]
    address: String,
    /// Listen on this port.
    #[structopt(long = "port", default_value = "9000")]
    port: u16,
}

async fn push(_cx: Context<()>) -> EndpointResult<StatusCode> {
    Ok(StatusCode::BAD_REQUEST)
}

fn main() {
    let opt = Opt::from_args();
    let bind = SocketAddr::new(opt.address.parse().expect("valid address"), opt.port);

    let mut app = tide::App::new(());
    app.at("/").post(push);
    app.serve(bind).expect("bind");
}
