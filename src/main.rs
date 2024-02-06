#![forbid(unsafe_code)]

use anyhow::{anyhow, Result};
use env_logger::{Builder as LogBuilder, Env};
use futures::{
    future::{self, Either},
    pin_mut,
};
use rust_decimal::Decimal;
use time::OffsetDateTime;
use tokio::{
    runtime::Builder,
    task::{self},
};
use tokio_util::sync::CancellationToken;

use std::io::Write;
use std::{env, str::FromStr};

#[macro_use]
extern crate log;

mod explorer;

const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    init_env_logger();
    info!("ethereum_transfer_searcher started, version: {CARGO_PKG_VERSION}",);

    let args_iter = env::args();
    let arg = args_iter
        .skip(1)
        .next()
        .ok_or_else(|| anyhow!("invalid argument"))?;
    let eth_min_value = Decimal::from_str(&arg)?;

    info!("Command-line argument: {eth_min_value}");

    let rt = Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();

    rt.block_on(async {
        let cancel_token = CancellationToken::new();
        let cloned_token = cancel_token.clone();

        let exit_hanlde = task::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            cloned_token.cancel();
        });

        let fut = explorer::explorer(eth_min_value, cancel_token.clone());

        pin_mut!(fut);
        let f = future::select(exit_hanlde, fut);

        match f.await {
            Either::Left((err, _)) => {
                if let Err(e) = err {
                    error!("{e}");
                }
            }
            Either::Right(_) => {
                debug!("all jobs completed");
            }
        }

        cancel_token.drop_guard();

        Ok(()) as Result<()>
    })
}

fn init_env_logger() {
    let logger_env = Env::default().default_filter_or("info, ets=debug");

    LogBuilder::from_env(logger_env)
        .format(|buf, record| {
            let datetime = OffsetDateTime::now_utc();
            let month = datetime.month() as u8;
            let day = datetime.day();
            let hour = datetime.hour();
            let minute = datetime.minute();
            let second = datetime.second();
            let ms = datetime.millisecond();

            let args = record.args();
            let level = record.level();
            let target = record.target();
            let line = record.line().unwrap_or_default();

            writeln!(buf, "[{month:02}.{day:02} {hour:02}:{minute:02}:{second:02}.{ms:03} {level:5} {target}:{line:03}] {args}")
        })
        .init();
}
