use super::{AppError, AppResult};
use std::time::Duration;
// 本机 MCP 连接不经过系统代理，也不跟随重定向泄露凭据。
pub fn blocking_client(timeout: Duration) -> AppResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(timeout)
        .build()
        .map_err(AppError::internal)
}
pub fn async_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .build()
        .map_err(AppError::internal)
}
