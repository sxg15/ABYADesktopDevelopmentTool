//! Native MCP protocol boundary. No subprocess or CLI transport is used here.
mod protocol;
mod sessions;
#[cfg(test)]
mod tests;
pub use protocol::McpGateway;
pub use sessions::McpLeases;
