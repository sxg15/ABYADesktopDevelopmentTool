import { useCallback, useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, type DesktopRelease } from "../shared/api";
import { Modal } from "./Modal";

type Build = Awaited<ReturnType<typeof api.buildInfo>>;
const snoozeKey = "abya.desktop.release.snooze";
const snoozeValue = (n: number) => `${n}/${new Date().toDateString()}`;

export function useDesktopUpdate(build: Build | undefined, en: boolean, notify: (message: string, error?: boolean) => void) {
  const [candidate, setCandidate] = useState<DesktopRelease>();
  const [open, setOpen] = useState(false);
  const [checking, setChecking] = useState(false);
  const current = useRef({ en, notify });
  current.current = { en, notify };
  const inFlight = useRef(false);
  const manualRequested = useRef(false);
  const mounted = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);

  const check = useCallback(async (manual = false) => {
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
    if (candidate) localStorage.setItem(snoozeKey, snoozeValue(candidate.releaseNumber));
    setCandidate(undefined);
    setOpen(false);
  }
  const title = en ? "Desktop update available" : "桌面工具有新版本";
  const reminder = candidate && !open && <button className="secondary-button" onClick={() => setOpen(true)}>{title} · #{candidate.releaseNumber}</button>;
  const dialog = candidate && open && <Modal title={title} onClose={later}>
    <p>{en ? "Current" : "当前"} {build?.version} #{build?.releaseNumber} → {candidate.version} #{candidate.releaseNumber}</p>
    <p>{candidate.notes}</p>
    <p>{en ? "Extract the complete ZIP to a new folder and use the included launcher after closing the old app." : "将完整 ZIP 解压到新目录，关闭旧版后，使用包内的「ABYA 开发工具.cmd」启动。"}</p>
    <div className="modal-actions"><button className="secondary-button" onClick={later}>{en ? "Remind me later" : "稍后提醒"}</button>
    <button className="primary-button" onClick={() => void openUrl(candidate.packageUrl).then(later).catch(() =>
      notify(en ? "Could not open the download link." : "无法打开下载链接。", true))}>{en ? "Get update" : "获取新版"}</button></div>
  </Modal>;
  return { check, checking, reminder, dialog };
}
