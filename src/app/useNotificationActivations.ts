import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { attentionApi, type NotificationActivation } from "../shared/notifications";

export function useNotificationActivations(onActivation: (activation: NotificationActivation) => void) {
  const callback = useRef(onActivation);
  const handled = useRef(new Set<string>());
  callback.current = onActivation;
  useEffect(() => {
    let alive = true, draining = false, again = false;
    async function drain() {
      if (draining) { again = true; return; }
      draining = true;
      try {
        do {
          again = false;
          const activations = await attentionApi.takeActivations();
          if (alive) {
            for (const activation of activations ?? []) {
              if (!handled.current.has(activation.id)) {
                callback.current(activation);
                handled.current.add(activation.id);
              }
            }
            await attentionApi.acknowledgeActivations((activations ?? []).map(a => a.id));
          }
        } while (alive && again);
      } catch { /* No task action is submitted by this listener. */ }
      finally { draining = false; }
    }
    const subscription = listen("task-notification-activated", () => void drain())
      .catch(() => undefined);
    void subscription.then(() => { if (alive) void drain(); });
    return () => { alive = false; void subscription.then(unlisten => unlisten?.()); };
  }, []);
}
