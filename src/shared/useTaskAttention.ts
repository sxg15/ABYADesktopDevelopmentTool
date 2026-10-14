import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { attentionApi, type TaskAttentionSnapshot } from "./notifications";

export function useTaskAttention(taskId: string) {
  const [snapshot, setSnapshot] = useState<TaskAttentionSnapshot>();
  useEffect(() => {
    let alive = true, sequence = 0;
    setSnapshot(undefined);
    async function refresh() {
      const request = ++sequence;
      try {
        const next = await attentionApi.get(taskId);
        if (alive && request === sequence && next?.taskId === taskId) setSnapshot(next);
      } catch { if (alive && request === sequence) setSnapshot(undefined); }
    }
    const subscription = listen<TaskAttentionSnapshot>("task-attention-changed", ({ payload }) => {
      if (alive && payload.taskId === taskId) { sequence++; setSnapshot(payload); }
    }).catch(() => undefined);
    void refresh();
    const timer = window.setInterval(() => void refresh(), 3000);
    window.addEventListener("abya:control-changed", refresh);
    return () => {
      alive = false; sequence++; window.clearInterval(timer);
      window.removeEventListener("abya:control-changed", refresh);
      void subscription.then(unlisten => unlisten?.());
    };
  }, [taskId]);
  return snapshot?.taskId === taskId ? snapshot : undefined;
}
