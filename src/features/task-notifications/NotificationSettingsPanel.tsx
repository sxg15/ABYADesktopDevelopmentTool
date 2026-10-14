import { useEffect, useState } from "react";
import { Bell, RefreshCw } from "lucide-react";
import { attentionApi, type NotificationSettings } from "../../shared/notifications";
import { errorMessage } from "../../shared/api";
import type { MessageKey } from "../../i18n";

export function NotificationSettingsPanel({ t, notify }: {
  t: (key: MessageKey) => string;
  notify: (message: string, error?: boolean) => void;
}) {
  const [settings, setSettings] = useState<NotificationSettings>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function refresh() {
    try { setSettings(await attentionApi.settings()); setError(""); }
    catch (e) { setError(errorMessage(e)); }
  }
  useEffect(() => { void refresh(); }, []);
  async function toggle(enabled: boolean) {
    setBusy(true);
    try { setSettings(await attentionApi.setEnabled(enabled)); setError(""); }
    catch (e) { setError(errorMessage(e)); }
    finally { setBusy(false); }
  }
  async function test() {
    setBusy(true);
    try { await attentionApi.test(); notify(t("notificationTestSent")); }
    catch (e) { setError(errorMessage(e)); }
    finally { setBusy(false); }
  }
  const reasons: Record<string, MessageKey> = {
    initializing: "notificationsInitializing",
    disabledForApplication: "notificationsDisabledByWindows",
    disabledForUser: "notificationsDisabledByWindows",
    disabledByGroupPolicy: "notificationsDisabledByPolicy",
    disabledByManifest: "notificationsUnavailable",
    unsupported: "notificationsUnsupported",
    unavailable: "notificationsUnavailable",
  };
  const reason = settings?.reason ? (reasons[settings.reason] ? t(reasons[settings.reason]) : settings.reason) : "";
  return <section className="settings-section">
    <h2>{t("backgroundActionNotifications")}</h2>
    <label className="toggle-row">
      <input type="checkbox" checked={settings?.enabled ?? true} disabled={busy || !settings}
        onChange={event => void toggle(event.target.checked)} />
      <span>{t("enableBackgroundActionNotifications")}</span>
    </label>
    <p>{t("backgroundActionNotificationsHint")}</p>
    <p role="status">{settings?.available ? t("notificationsReady") : t("notificationsUnavailable")}{settings?.testEnvironment ? " · " + t("notificationsTestEnvironment") : ""}</p>
    {(error || reason) && <div className="inline-error" role="alert">{error || reason}</div>}
    <p>{t("notificationsWindowsHint")}</p>
    <div className="settings-actions">
      <button className="secondary-button" onClick={() => void test()} disabled={busy || !settings?.available}>
        <Bell size={16} />{t("testNotification")}
      </button>
      <button className="secondary-button" onClick={() => void refresh()} disabled={busy}>
        <RefreshCw size={16} />{t("refresh")}
      </button>
    </div>
  </section>;
}
