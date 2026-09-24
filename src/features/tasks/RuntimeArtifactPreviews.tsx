import { useEffect, useMemo, useState } from "react";
import { api, errorMessage } from "../../shared/api";
import type { MessageKey } from "../../i18n";

export function RuntimeArtifactPreviews({ taskId, detail, t }: {
  taskId: string; detail: string; t: (key: MessageKey) => string;
}) {
  const [selected, setSelected] = useState("");
  const [image, setImage] = useState("");
  const [error, setError] = useState("");
  const paths = useMemo(() => {
    try { const value = JSON.parse(detail).artifacts;
      return Array.isArray(value) ? value.filter((v): v is string => typeof v === "string") : [];
    } catch { return []; }
  }, [detail]);
  useEffect(() => {
    let disposed = false; setImage(""); setError("");
    if (selected) void api.readRuntimeArtifact(taskId, selected).then(value => {
      if (!disposed) setImage(value);
    }).catch(reason => { if (!disposed) setError(errorMessage(reason)); });
    return () => { disposed = true; };
  }, [taskId, selected]);
  if (!paths.length) return null;
  return <div>
    {paths.map(path => <button className="secondary-button" key={path} onClick={() => setSelected(path)}>
      {t("previewRuntimeImage")} · {path.split(/[\\/]/).pop()}
    </button>)}
    {error && <p role="alert">{error}</p>}
    {image && <img src={image} alt={t("previewRuntimeImage")} style={{ width: "100%", maxHeight: "60vh", objectFit: "contain" }} />}
  </div>;
}
