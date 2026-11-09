import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage, type DesktopRelease } from "../shared/api";
import { Modal } from "./Modal";

type Build = Awaited<ReturnType<typeof api.buildInfo>>;
const snoozeKey = "abya.desktop.release.snooze";
const snoozeValue = (n: number) => `${n}/${new Date().toDateString()}`;

export function useDesktopUpdate(build: Build | undefined, en: boolean, notify: (message: string, error?: boolean) => void) {
  const [candidate, setCandidate] = useState<DesktopRelease>();
  const [open, setOpen] = useState(false);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [installError, setInstallError] = useState("");
  const installation = useRef(false);
  const current = useRef({ en, notify });
  current.current = { en, notify };
  const inFlight = useRef(false);
  const manualRequested = useRef(false);
  const mounted = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);

  const check = useCallback(async (manual = false) => {
    if (installation.current) return;
    const message = (zh: string, english: string, error = false) =>
      current.current.notify(current.current.en ? english : zh, error);
    if (build?.verification !== "verified" || build.releaseNumber == null) {
      if (manual) message("请从带更新检查的正式发布包启动。", "Start the verified portable build to check for updates.");
      return;
    }
    manualRequested.current ||= manual;
    if (inFlight.current) return;
    inFlight.current = true;
    setChecking(true);
    try {
      const release = await api.latestDesktopRelease();
      if (!mounted.current) return;
      if (!release || release.releaseNumber <= build.releaseNumber) {
        setCandidate(undefined);
        setOpen(false);
        if (manualRequested.current) message("当前已是最新发布包。", "Your desktop package is up to date.");
      } else if (manualRequested.current || localStorage.getItem(snoozeKey) !== snoozeValue(release.releaseNumber)) {
        setCandidate(release);
        if (manualRequested.current) setOpen(true);
      }
    } catch {
      if (mounted.current && manualRequested.current) message("无法读取 GitHub 发布信息，请检查网络后重试。", "Could not read the GitHub release. Check your connection and retry.", true);
    } finally {
      inFlight.current = manualRequested.current = false;
      if (mounted.current) setChecking(false);
    }
  }, [build]);
  useEffect(() => {
    void check();
    const timer = window.setInterval(() => void check(), 15 * 60_000);
    const online = () => void check();
    window.addEventListener("online", online);
    return () => { window.clearInterval(timer); window.removeEventListener("online", online); };
  }, [check]);

  useEffect(() => {
    if (!candidate || open) return;
    let valid = true;
    let timer: number;
    async function waitForIdle() {
      const busy = await api.desktopHasActiveTurn().catch(() => true);
      if (!valid) return;
      if (busy) timer = window.setTimeout(waitForIdle, 5000);
      else setOpen(true);
    }
    void waitForIdle();
    return () => { valid = false; window.clearTimeout(timer); };
  }, [candidate, open]);

  function later() {
    if (installation.current) return;
    setInstallError("");
    if (candidate) localStorage.setItem(snoozeKey, snoozeValue(candidate.releaseNumber));
    setCandidate(undefined);
    setOpen(false);
  }
  const title = en ? "Desktop update available" : "桌面工具有新版本";
  async function install() {
    if (!candidate || installation.current) return;
    installation.current = true;
    setInstalling(true);
    setInstallError("");
    try { await api.installDesktopRelease(candidate); }
    catch (error) {
      installation.current = false;
      setInstalling(false);
      setInstallError(errorMessage(error));
    }
  }
  const reminder = candidate && !open && <button className="secondary-button" onClick={() => setOpen(true)}>{title} · #{candidate.releaseNumber}</button>;
  const dialog = candidate && open && <Modal title={title} onClose={later}>
    <p>{en ? "Current" : "当前"} {build?.version} #{build?.releaseNumber} → {candidate.version} #{candidate.releaseNumber}</p>
    <p>{candidate.notes}</p>
    <p>{en ? "Save your work. The app will update this Git checkout, restart here and adapt your projects." : "请先保存手头的工作。App 将通过 GitHub 更新当前目录，在原位置重启并自动适配已有项目。"}</p>
    {installError && <p role="alert">{installError}</p>}
    <div className="modal-actions"><button className="secondary-button" disabled={installing} onClick={later}>{en ? "Remind me later" : "稍后提醒"}</button>
    <button className="primary-button" disabled={installing} onClick={() => void install()}>
      {installing ? (en ? "Preparing update…" : "正在准备更新…") : (en ? "Update in place and restart" : "原地更新并重启")}</button></div>
  </Modal>;
  return { check, checking, reminder, dialog };
}
