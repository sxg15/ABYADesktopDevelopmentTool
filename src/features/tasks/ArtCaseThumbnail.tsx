import { useEffect, useRef, useState } from "react";
import { ImageOff, Image } from "lucide-react";
import { artLibraryApi, type ArtCase } from "../../shared/artDesign";

export function ArtCaseThumbnail({ entry, en }: { entry: ArtCase; en: boolean }) {
  const host = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(false), [src, setSrc] = useState(""), [failed, setFailed] = useState(false);
  const preferred = entry.media?.findIndex(media => media.sourceType !== "reference") ?? -1;
  const index = preferred >= 0 ? preferred : 0, media = entry.media?.[index];
  useEffect(() => {
    if (!host.current || typeof IntersectionObserver === "undefined") { setVisible(true); return; }
    const observer = new IntersectionObserver(entries => {
      if (entries.some(item => item.isIntersecting)) { setVisible(true); observer.disconnect(); }
    }, { rootMargin: "120px" });
    observer.observe(host.current); return () => observer.disconnect();
  }, []);
  useEffect(() => {
    let alive = true; setSrc(""); setFailed(false);
    if (visible && entry.version && media) void artLibraryApi.media(entry.id, entry.version, index)
      .then(value => { if (alive) setSrc(value); }).catch(() => { if (alive) setFailed(true); });
    return () => { alive = false; };
  }, [entry.id, entry.version, index, visible, Boolean(media)]);
  return <span ref={host} className="art-case-thumbnail">
    {src && media ? media.mediaType.startsWith("video/")
      ? <video src={src} preload="metadata" muted aria-label={media.title} />
      : <img src={src} alt={media.title} loading="lazy" />
      : <span className="art-case-thumbnail-placeholder">{failed ? <ImageOff size={24}/> : <Image size={24}/>}
        <small>{failed ? (en ? "Preview unavailable" : "预览暂不可用") : media ? (en ? "Loading preview" : "正在读取预览") : (en ? "Open to inspect draft" : "查看草稿内容")}</small></span>}
    {media?.sourceType === "reference" && <span className="art-case-source-label">{en ? "Original reference" : "原始参考"}</span>}
  </span>;
}
