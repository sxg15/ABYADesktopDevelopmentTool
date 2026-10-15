import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Copy, ExternalLink, Github, LogOut } from "lucide-react";
import type { MessageKey } from "../../i18n";
import { clientApi, clientError, type GameClientState, type LoginAuthorization } from "../../shared/gameClients";

export function GitHubAccountSettings({ t, focus, onChanged }: {
  t: (key: MessageKey) => string; focus?: boolean; onChanged: () => void;
}) {
  const [state, setState] = useState<GameClientState>();
  const [authorization, setAuthorization] = useState<LoginAuthorization | null>(null);
  const [remember, setRemember] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [remaining, setRemaining] = useState(0);
  const attempt = useRef<string | null>(null);
  const section = useRef<HTMLElement>(null);
  const mounted = useRef(false);
  const onChangedRef = useRef(onChanged); onChangedRef.current = onChanged;
  const translate = useRef(t); translate.current = t;

  useEffect(() => {
    mounted.current = true;
    void clientApi.state().then(next => {
      if (!mounted.current) return;
      setState(next);
      if (next.authorization) { attempt.current = next.authorization.id; setAuthorization(next.authorization); }
    }).catch(error => { if (mounted.current) setError(clientError(error, translate.current)); });
    return () => { mounted.current = false; };
  }, []);
  useEffect(() => {
    if (focus) { section.current?.scrollIntoView?.({ block: "start" }); section.current?.focus(); }
  }, [focus]);

  useEffect(() => {
    if (!authorization) return;
    const current = authorization;
    let alive = true;
    let timer: ReturnType<typeof setTimeout>;
    const updateTime = () => setRemaining(Math.max(0, Math.ceil((Date.parse(current.expiresAt) - Date.now()) / 1000)));
    updateTime();
    const clock = setInterval(updateTime, 1000);
    async function poll() {
      if (!alive || attempt.current !== current.id) return;
      if (Date.parse(current.expiresAt) <= Date.now()) {
        attempt.current = null; setAuthorization(null); setError(translate.current("gcOAuthExpired"));
        void clientApi.cancelLogin(current.id).catch(() => undefined); return;
      }
      let delay = current.pollAfterSeconds;
      try {
        const result = await clientApi.pollLogin(current.id);
        if (!alive || attempt.current !== current.id) return;
        if (result.status === "authenticated") {
          attempt.current = null; setAuthorization(null); setError(""); setNotice(translate.current("gcSignedIn"));
          const next = await clientApi.state();
          if (mounted.current) { setState(next); onChangedRef.current(); }
          return;
        }
        delay = result.authorization?.pollAfterSeconds ?? delay;
        setError("");
      } catch (error) {
        if (!alive || attempt.current !== current.id) return;
        setError(clientError(error, translate.current));
        const code = typeof error === "object" && error !== null && "code" in error ? error.code : "";
        if (code !== "clientNetwork" && code !== "clientRateLimit") {
          attempt.current = null; setAuthorization(null); return;
        }
        delay = Math.max(delay, 10);
      }
      if (alive) timer = setTimeout(poll, Math.max(1, delay) * 1000);
    }
    timer = setTimeout(poll, Math.max(1, current.pollAfterSeconds) * 1000);
    return () => { alive = false; clearTimeout(timer); clearInterval(clock); };
  }, [authorization?.id]);

  async function openBrowser() {
    try { await openUrl("https://github.com/login/device"); }
    catch { setError(t("gcBrowserFailed")); }
  }
  async function start() {
    setBusy(true); setError(""); setNotice("");
    try {
      const next = await clientApi.startLogin(remember);
      if (!mounted.current) return; // Native attempt can be resumed when settings is reopened.
      attempt.current = next.id; setAuthorization(next);
      await openBrowser();
    } catch (error) { if (mounted.current) setError(clientError(error, t)); }
    finally { if (mounted.current) setBusy(false); }
  }
  async function cancel() {
    const id = attempt.current;
    if (!id) return;
    attempt.current = null; setAuthorization(null); setBusy(true);
    try {
      await clientApi.cancelLogin(id); setError(""); setNotice(t("gcOAuthCancelled"));
    } catch (error) { setError(clientError(error, t)); }
    finally { setBusy(false); }
  }
  async function logout() {
    attempt.current = null; setAuthorization(null); setBusy(true); setError(""); setNotice("");
    try {
      const next = await clientApi.logout(); attempt.current = null; setAuthorization(null); setState(next); onChangedRef.current();
    } catch (error) { setError(clientError(error, t)); }
    finally { setBusy(false); }
  }

  return <section className="settings-section client-account-settings" tabIndex={-1} ref={section} aria-label={t("gcAccount")}>
    <div className="settings-section-header"><h2><Github size={19} /> {t("gcAccount")}</h2>
      {state?.account && <button className="secondary-button" disabled={busy} onClick={() => void logout()}><LogOut size={15} />{t("gcLogout")}</button>}
    </div>
    <p>{state?.account ? `${t("gcSignedInAs")}: ${state.account.login}` : t("gcNotLoggedIn")}</p>
    {state?.account && <div className="client-badges"><span className="client-badge">{t(state.account.canManage ? "gcCanManage" : "gcReadOnly")}</span>
      {!state.account.remembered && <span className="client-badge">{t("gcSessionOnly")}</span>}</div>}
    <div className="client-login">
      {!authorization ? <>
        {!state?.account && <p>{t("gcLoginFirst")}</p>}
        <label className="toggle-row"><input type="checkbox" checked={remember} disabled={busy || !state} onChange={event => setRemember(event.target.checked)} />{t("gcRemember")}</label>
        <button className="primary-button" disabled={busy || !state} onClick={() => void start()}><Github size={17} />{busy ? t("gcConnecting") : t(state?.account ? "gcSignInAgain" : "gcLogin")}</button>
        <p>{t("gcOAuthHelp")}</p><p>{t("gcCredentialSafety")}</p>
      </> : <div className="client-device-login">
        <p>{t("gcDeviceInstructions")}</p>
        <label className="field"><span>{t("gcDeviceCode")}</span><input className="client-device-code" readOnly value={authorization.userCode} onFocus={event => event.target.select()} /></label>
        <div className="client-actions">
          <button className="secondary-button" onClick={() => void navigator.clipboard.writeText(authorization.userCode).then(() => setNotice(t("gcCodeCopied"))).catch(() => setError(t("gcCopyCodeHelp")))}><Copy size={15} />{t("gcCopyCode")}</button>
          <button className="primary-button" onClick={() => void openBrowser()}><ExternalLink size={15} />{t("gcOpenBrowser")}</button>
          <button className="secondary-button" disabled={busy} onClick={() => void cancel()}>{t("gcCancelLogin")}</button>
        </div>
        <p role="status">{t("gcWaitingAuthorization")} · {t("gcCodeExpires")}: {Math.floor(remaining / 60)}:{String(remaining % 60).padStart(2, "0")}</p>
        <p>{t("gcDeviceReturn")}</p>
      </div>}
      {notice && <p role="status">{notice}</p>}
      {error && <div className="inline-error" role="alert">{error}</div>}
    </div>
  </section>;
}
