import { CircleCheck, CircleDashed, CirclePause, CirclePlay, Clock3, Eye, TriangleAlert } from "lucide-react";
import { statusName } from "./productionText";

function stageAppearance(status: string) {
  switch (status) {
    case "passed":
    case "accepted":
    case "completed":
      return { tone: "passed", Icon: CircleCheck };
    case "blocked":
    case "rejected":
    case "failed":
      return { tone: "blocked", Icon: TriangleAlert };
    case "in-progress":
    case "playable":
      return { tone: "active", Icon: CirclePlay };
    case "awaiting-confirmation":
    case "awaiting-feedback":
    case "awaiting-recheck":
    case "waiting-for-answers":
    case "execution-converged":
    case "submitted":
      return { tone: "active", Icon: Clock3 };
    case "stopped":
      return { tone: "neutral", Icon: CirclePause };
    default:
      return { tone: "neutral", Icon: CircleDashed };
  }
}

export function StageCard({ number, name, status, current, viewed, onView }: {
  number: number;
  name: string;
  status: string;
  current: boolean;
  viewed: boolean;
  onView: () => void;
}) {
  const { tone, Icon } = stageAppearance(status);
  return <button type="button" className="stage-card" data-tone={tone}
    aria-current={current ? "step" : undefined} aria-pressed={viewed}
    title={viewed ? "正在查看：" + name : "查看" + name} onClick={onView}>
    <span className="stage-card-heading"><span className="stage-card-number">{number}</span><strong>{name}</strong></span>
    <span className="stage-card-footer">
      <span className="stage-card-status"><Icon size={14} aria-hidden="true"/>{statusName(status)}</span>
      {viewed && <Eye size={14} aria-hidden="true"/>}
    </span>
  </button>;
}
