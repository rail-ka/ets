use anyhow::{Context, Result};
use ethers::prelude::*;
use futures::{
    future::{select, Either},
    pin_mut,
};
use rust_decimal::Decimal;
use tokio_util::sync::CancellationToken;

const PUBLIC_NODE: &str = "https://eth.llamarpc.com";
const PUBLIC_WSS_NODE: &str = "wss://eth.llamarpc.com";

pub(crate) async fn explorer(min_value: Decimal, token: CancellationToken) -> Result<()> {
    debug!("min_value: {min_value}");

    let provider = Provider::<Http>::try_from(PUBLIC_NODE)?;

    let last_block = provider.get_block_number().await?;

    info!("last block number: {last_block}");

    info!("transactions in last block {last_block}:");

    let block = provider
        .get_block(last_block)
        .await?
        .context("cannof find block")?;

    handle_block(block, &provider, min_value).await?;

    let ws_provider = Provider::<Ws>::connect(PUBLIC_WSS_NODE).await?;

    let mut subscriber = ws_provider.subscribe_pending_txs().await?;

    info!("subscribed to new transactions");

    let fut = async {
        while let Some(tx) = subscriber.next().await {
            if token.is_cancelled() {
                break;
            }
            if let Some(tx) = provider.get_transaction(tx).await? {
                transaction_handle(tx, min_value);
            }
        }

        Ok(()) as Result<()>
    };
    pin_mut!(fut);

    let cancel_fut = token.cancelled();
    pin_mut!(cancel_fut);

    let f = select(cancel_fut, fut);

    match f.await {
        Either::Left(_) => {}
        Either::Right(_) => {
            debug!("all jobs completed");
        }
    }

    Ok(())
}

async fn handle_block(
    block: Block<H256>,
    provider: &Provider<Http>,
    min_value: Decimal,
) -> Result<()> {
    for tx in block.transactions {
        let tx = provider
            .get_transaction(tx)
            .await?
            .context("cannot find tx")?;

        transaction_handle(tx, min_value);
    }
    Ok(())
}

fn transaction_handle(tx: Transaction, min_value: Decimal) {
    let value = tx.value;
    let sender = tx.from;
    let Some(receiver) = tx.to else {
        return;
    };

    let value = value.as_u128() as i128;
    let mut value = Decimal::from_i128_with_scale(value, 18);

    if value < min_value {
        return;
    }

    value.normalize_assign();

    info!("{sender} -> {receiver} | {value}");
}
