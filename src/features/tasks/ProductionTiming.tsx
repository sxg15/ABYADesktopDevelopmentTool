import { useEffect, useState } from "react";
import { productionApi } from "../../shared/production";
import { errorMessage } from "../../shared/api";
export function ProductionTiming({
  taskId,
  en,
}: {
  taskId: string;
  en: boolean;
}) {
  const [data, setData] =
    useState<Awaited<ReturnType<typeof productionApi.timing>>>();
  const [error, setError] = useState("");
  useEffect(() => {
    let alive = true;
    setData(undefined);
    async function read() {
      try {
        const d = await productionApi.timing(taskId);
        if (alive) setData(d);
      } catch (e) {
        if (alive) setError(errorMessage(e));
      }
    }
    void read();
    return () => {
      alive = false;
    };
  }, [taskId]);
  const names = [
    ["elapsedMs", en ? "Total elapsed" : "总历时"],
    ["firstPlayableMs", en ? "First playable elapsed" : "首次可玩历时"],
    ["activeWorkMs", en ? "Recorded active progress" : "有记录的推进时间"],
    ["humanWaitMs", en ? "Human review wait" : "人工等待"],
    ["blockedMs", en ? "Blocked stage time" : "阶段阻塞"],
    ["feedbackReworkMs", en ? "Feedback rework" : "反馈返工"],
    ["executionMs", en ? "Completed turn intervals" : "已结束轮次区间"],
    ["toolMs", en ? "Tool activity" : "工具活动"],
    ["answerWaitMs", en ? "Waiting for first answers" : "首次答题等待"],
    ["approvalWaitMs", en ? "Current document review" : "当前文档确认等待"],
    ["pauseMs", en ? "Completed pauses" : "已结束暂停区间"],
    ["connectionMs", en ? "Connection setup" : "连接建立"],
  ] as const;
  return (
    <section className="production-card">
      <h4>{en ? "Recorded time" : "制作时间汇总"}</h4>
      {error && <p>{error}</p>}
      {data ? (
        <>
          <div className="production-timing-grid">
            {names.map(([key, name]) => (
              <div key={key}>
                <small>{name}</small>
                <strong>{duration(data[key], en)}</strong>
              </div>
            ))}
          </div>
          <p className="production-meta">
            {en
              ? "Intervals include tool and waiting time. Overlapping intervals in each category are merged; do not add categories together. Missing or unfinished intervals are unknown."
              : data.note}
          </p>
        </>
      ) : (
        <p>{en ? "Reading recorded events…" : "正在读取时间记录…"}</p>
      )}
    </section>
  );
}
function duration(value: number | null, en: boolean) {
  if (value == null) return en ? "Unknown" : "未知";
  const s = Math.floor(value / 1000);
  return `${Math.floor(s / 3600)}${en ? "h" : "小时"} ${Math.floor((s % 3600) / 60)}${en ? "m" : "分"} ${s % 60}${en ? "s" : "秒"}`;
}
