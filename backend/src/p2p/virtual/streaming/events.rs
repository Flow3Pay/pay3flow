use tokio::sync::mpsc;

use crate::p2p::P2pRouteSearchResponse;

use super::super::model::RouteBatch;

const ROUTE_BATCH_CHANNEL_CAPACITY: usize = 16;

pub(in crate::p2p) fn route_batch_channel() -> (mpsc::Sender<RouteBatch>, mpsc::Receiver<RouteBatch>)
{
    mpsc::channel(ROUTE_BATCH_CHANNEL_CAPACITY)
}

pub(in crate::p2p) async fn send_batch(
    sender: &mpsc::Sender<RouteBatch>,
    batch: RouteBatch,
) -> bool {
    sender.send(batch).await.is_ok()
}

pub(in crate::p2p) async fn publish_snapshot(
    updates: Option<&mpsc::Sender<P2pRouteSearchResponse>>,
    response: P2pRouteSearchResponse,
) -> bool {
    match updates {
        Some(updates) => updates.send(response).await.is_ok(),
        None => true,
    }
}
