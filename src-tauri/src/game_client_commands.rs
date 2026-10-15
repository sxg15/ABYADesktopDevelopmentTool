//! Tauri transport only; network/storage/archive rules live in game_clients.
use crate::{
    AppState,
    foundation::{AppError, AppResult},
    modules::game_clients::*,
};
use tauri::State;

async fn blocking<T: Send + 'static>(
    action: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(action)
        .await
        .map_err(|_| AppError::internal("Game client worker failed."))?
}

#[tauri::command]
pub fn game_client_state(state: State<'_, AppState>) -> AppResult<GameClientState> {
    state.game_clients.state()
}
#[tauri::command]
pub fn game_client_operation(state: State<'_, AppState>) -> ClientOperation {
    state.game_clients.operation()
}
#[tauri::command]
pub fn game_client_cancel(state: State<'_, AppState>) {
    state.game_clients.cancel();
}
#[tauri::command]
pub async fn game_client_start_login(
    remember: bool,
    state: State<'_, AppState>,
) -> AppResult<LoginAuthorization> {
    let service = state.game_clients.clone();
    blocking(move || service.start_login(remember)).await
}
#[tauri::command]
pub async fn game_client_poll_login(
    id: String,
    state: State<'_, AppState>,
) -> AppResult<LoginPoll> {
    let service = state.game_clients.clone();
    blocking(move || service.poll_login(&id)).await
}
#[tauri::command]
pub fn game_client_cancel_login(id: String, state: State<'_, AppState>) {
    state.game_clients.cancel_login(&id);
}
#[tauri::command]
pub fn game_client_logout(state: State<'_, AppState>) -> AppResult<GameClientState> {
    state.game_clients.logout()
}
#[tauri::command]
pub async fn game_client_releases(state: State<'_, AppState>) -> AppResult<Vec<ClientRelease>> {
    let service = state.game_clients.clone();
    blocking(move || service.releases()).await
}
#[tauri::command]
pub async fn game_client_release(id: u64, state: State<'_, AppState>) -> AppResult<ClientRelease> {
    let service = state.game_clients.clone();
    blocking(move || service.release(id)).await
}
#[tauri::command]
pub async fn game_client_save_release(
    input: ReleaseInput,
    state: State<'_, AppState>,
) -> AppResult<ClientRelease> {
    let service = state.game_clients.clone();
    blocking(move || service.save_release(input)).await
}
#[tauri::command]
pub async fn game_client_delete_release(id: u64, state: State<'_, AppState>) -> AppResult<()> {
    let service = state.game_clients.clone();
    blocking(move || service.delete_release(id)).await
}
#[tauri::command]
pub async fn game_client_delete_asset(
    release_id: u64,
    asset_id: u64,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let service = state.game_clients.clone();
    blocking(move || service.delete_asset(release_id, asset_id)).await
}
#[tauri::command]
pub async fn game_client_upload(
    release_id: u64,
    path: String,
    state: State<'_, AppState>,
) -> AppResult<ReleaseAsset> {
    let service = state.game_clients.clone();
    blocking(move || service.upload(release_id, path)).await
}
#[tauri::command]
pub async fn game_client_install(
    input: InstallInput,
    state: State<'_, AppState>,
) -> AppResult<ClientInstallation> {
    let service = state.game_clients.clone();
    blocking(move || service.install(input)).await
}
#[tauri::command]
pub async fn game_client_check_update(
    state: State<'_, AppState>,
) -> AppResult<Option<ClientUpdate>> {
    let service = state.game_clients.clone();
    blocking(move || service.check_update()).await
}
