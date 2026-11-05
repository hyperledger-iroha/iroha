//! Complete canonical signed-block histories for native integration assertions.

use eyre::{Result, WrapErr, ensure, eyre};
use futures_util::{Stream, StreamExt};
use iroha::{blocking::Client, data_model::block::SignedBlock};
use std::num::NonZeroU64;

/// Replay the contacted peer's complete applied prefix, newest block first.
///
/// One existing client request budget covers status, authenticated subscription,
/// every full signed block and stream closure. A partial or reordered prefix is
/// an error; the reader never substitutes status metadata for a block body.
///
/// # Errors
/// Returns errors from authentication, transport, incomplete history or the
/// original request deadline. The contacted chain must already contain genesis.
pub async fn canonical_history(client: &Client) -> Result<Vec<SignedBlock>> {
    let budget = client.client().torii_request_timeout();
    ensure!(
        !budget.is_zero(),
        "canonical history needs a bounded client"
    );
    tokio::time::timeout(budget, async {
        let height = client.client().status().get().await?.blocks;
        ensure!(height > 0, "canonical history omitted genesis");
        let mut stream = client
            .account_client()
            .blocks()
            .subscribe(NonZeroU64::MIN)
            .await?;
        let result = read_prefix(&mut stream, height).await;
        let closed = stream.close().await;
        let mut blocks = result?;
        closed?;
        blocks.reverse();
        Ok(blocks)
    })
    .await
    .wrap_err("canonical history exceeded the original client request deadline")?
}

async fn read_prefix(
    stream: &mut (impl Stream<Item = iroha::Result<SignedBlock>> + Unpin),
    height: u64,
) -> Result<Vec<SignedBlock>> {
    ensure!(height > 0, "canonical history omitted genesis");
    let mut blocks = Vec::new();
    for expected in 1..=height {
        let block = stream
            .next()
            .await
            .ok_or_else(|| eyre!("canonical history ended before height {expected}"))??;
        ensure!(
            block.header().height().get() == expected,
            "canonical history skipped or substituted height {expected}"
        );
        blocks.push(block);
    }
    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::data_model::block::{BlockHeader, builder::BlockBuilder};
    use iroha_crypto::{Algorithm, KeyPair};

    #[tokio::test]
    async fn canonical_prefix_preserves_full_wire_and_rejects_incomplete_or_reordered_history()
    -> Result<()> {
        let key = KeyPair::try_from_seed(vec![0x63; 32], Algorithm::Ed25519)?;
        let block = |height| {
            BlockBuilder::new(BlockHeader::new(
                NonZeroU64::new(height).unwrap(),
                None,
                None,
                height * 7,
                0,
            ))
            .build_with_signature(0, key.private_key())
        };
        let expected = [block(1), block(2)];
        let mut stream = futures_util::stream::iter(expected.iter().cloned().map(Ok));
        let actual = read_prefix(&mut stream, 2).await?;
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_eq!(actual.encode_wire()?, expected.encode_wire()?);
        }
        for heights in [vec![], vec![1], vec![2, 1], vec![1, 1], vec![1, 3]] {
            let mut stream =
                futures_util::stream::iter(heights.into_iter().map(|height| Ok(block(height))));
            assert!(read_prefix(&mut stream, 2).await.is_err());
        }
        let mut stream = futures_util::stream::iter([Ok(block(1))]);
        assert!(read_prefix(&mut stream, 0).await.is_err());
        let mut stream = futures_util::stream::iter([Err(iroha::Error::Timeout {
            operation: "stream.receive",
        })]);
        assert!(read_prefix(&mut stream, 1).await.is_err());
        Ok(())
    }
}
