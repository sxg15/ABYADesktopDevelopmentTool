import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "./api";
import type { MessageKey } from "../i18n";

export interface GameClientSelection {
  source: "local" | "remote";
  localExecutablePath: string;
  installationId: string;
  executableRelativePath: string;
  storageRoot: string;
}
export interface GitHubAccount { login: string; canManage: boolean; remembered: boolean }
export interface ReleaseAsset { id: number; name: string; size: number; digest: string | null; updatedAt: string; state: string }
export interface ClientRelease {
  id: number; tagName: string; name: string | null; body: string | null; draft: boolean;
  prerelease: boolean; immutable: boolean; publishedAt: string | null; assets: ReleaseAsset[];
}
export interface ClientInstallation {
  id: string; releaseId: number; tagName: string; prerelease: boolean; publishedAt: string | null;
  assetId: number; assetName: string; assetUpdatedAt: string; sha256: string; digestVerified: boolean;
  directory: string; storageRoot?: string; executableCandidates: string[]; installedAt: string;
}
export interface ClientOperation {
  active: boolean; kind: string; stage: string; completedBytes: number; totalBytes: number; errorCode: string | null;
}
export interface GameClientState {
  repository: string; account: GitHubAccount | null; installations: ClientInstallation[];
  defaultStorageRoot: string; operation: ClientOperation;
  authorization?: LoginAuthorization | null;
}
export interface LoginAuthorization { id: string; userCode: string; verificationUri: string; expiresAt: string; pollAfterSeconds: number }
export interface LoginPoll { status: "pending" | "authenticated"; authorization: LoginAuthorization | null }
export interface ClientUpdate { releaseId: number; tagName: string; currentTag: string }
export interface ReleaseInput { id: number | null; tagName: string; name: string; body: string; draft: boolean; prerelease: boolean }

export const emptyClientSelection = (): GameClientSelection => ({
  source: "local", localExecutablePath: "", installationId: "", executableRelativePath: "", storageRoot: "",
});
export const clientApi = {
  state: () => invoke<GameClientState>("game_client_state"),
  operation: () => invoke<ClientOperation>("game_client_operation"),
  cancel: () => invoke<void>("game_client_cancel"),
  startLogin: (remember: boolean) => invoke<LoginAuthorization>("game_client_start_login", { remember }),
  pollLogin: (id: string) => invoke<LoginPoll>("game_client_poll_login", { id }),
  cancelLogin: (id: string) => invoke<void>("game_client_cancel_login", { id }),
  logout: () => invoke<GameClientState>("game_client_logout"),
  releases: () => invoke<ClientRelease[]>("game_client_releases"),
  release: (id: number) => invoke<ClientRelease>("game_client_release", { id }),
  saveRelease: (input: ReleaseInput) => invoke<ClientRelease>("game_client_save_release", { input }),
  deleteRelease: (id: number) => invoke<void>("game_client_delete_release", { id }),
  deleteAsset: (releaseId: number, assetId: number) => invoke<void>("game_client_delete_asset", { releaseId, assetId }),
  upload: (releaseId: number, path: string) => invoke<ReleaseAsset>("game_client_upload", { releaseId, path }),
  install: (releaseId: number, assetId: number, storageRoot: string) => invoke<ClientInstallation>("game_client_install", { input: { releaseId, assetId, storageRoot } }),
  checkUpdate: () => invoke<ClientUpdate | null>("game_client_check_update"),
};

export function clientError(error: unknown, t: (key: MessageKey) => string): string {
  const code = typeof error === "object" && error !== null && "code" in error ? String(error.code) : "";
  const keys: Record<string, MessageKey> = {
    clientUnauthorized: "gcErrorAuth", clientReauthRequired: "gcErrorAuth", clientLoginRequired: "gcErrorAuth", secretStorage: "gcErrorAuth",
    clientOAuthDenied: "gcOAuthDenied", clientOAuthExpired: "gcOAuthExpired", clientOAuthCancelled: "gcOAuthCancelled",
    clientOAuthUnavailable: "gcOAuthUnavailable", clientOAuthScope: "gcOAuthScope",
    clientForbidden: "gcErrorPermission", clientNotFound: "gcErrorNotFound", clientRateLimit: "gcErrorRate",
    clientNetwork: "gcErrorNetwork", clientServer: "gcErrorNetwork", clientResponse: "gcErrorNetwork",
    clientBusy: "gcErrorBusy", clientCancelled: "gcCancelled", clientImmutable: "gcErrorImmutable",
    clientDuplicateAsset: "gcErrorDuplicate", clientConflict: "gcErrorConflict", clientReleaseInput: "gcErrorInput",
    clientAssetMissing: "gcErrorMissing", clientInstallationMissing: "gcErrorInstallation", clientChooseExecutable: "gcChooseExe",
    clientPackageFormat: "gcErrorPackage", clientNoExecutable: "gcErrorNoExe", clientIntegrity: "gcErrorIntegrity",
    clientUnsafePath: "gcErrorPath", clientUnsafeArchive: "gcErrorArchive", clientArchiveLimit: "gcErrorArchive",
    clientDownloadHost: "gcErrorHost", clientDownloadResponse: "gcErrorDownloadResponse", clientUploadSize: "gcErrorSize", clientTooManyResults: "gcErrorMany",
  };
  return keys[code] ? t(keys[code]) : errorMessage(error);
}

export function installable(asset: ReleaseAsset): boolean {
  return asset.state === "uploaded" && /\.zip$/i.test(asset.name) && !/macos|osx|linux|android/i.test(asset.name);
}
export function bytes(size: number): string {
  if (size >= 1024 ** 3) return `${(size / 1024 ** 3).toFixed(2)} GB`;
  if (size >= 1024 ** 2) return `${(size / 1024 ** 2).toFixed(1)} MB`;
  return size >= 1024 ? `${(size / 1024).toFixed(1)} KB` : `${size} B`;
}
