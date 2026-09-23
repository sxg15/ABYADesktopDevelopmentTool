use super::{ConnectionRegistration, GameHello, LogBatch, LogBatchAck};
use crate::foundation::AppResult;
use std::net::SocketAddr;

pub trait GameConnectionHandler: Send + Sync {
    fn connected(
        &self,
        hello: &GameHello,
        remote_address: SocketAddr,
    ) -> AppResult<ConnectionRegistration>;

    fn disconnected(&self, instance_id: &str);

    fn ingest_logs(&self, instance_id: &str, batch: &LogBatch) -> AppResult<LogBatchAck>;
}
