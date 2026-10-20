"""Recheck recorded text crops against immutable specimen images; requires Pillow."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
if args.output.exists():
    raise SystemExit("Refusing to overwrite an existing measurement report")
index = json.loads((root / "evidence/index.json").read_text(encoding="utf-8"))
results = []
for sample in index["samples"]:
    images = []
    for kind in ("reference", "native"):
        file = root / "evidence" / sample["images"][kind]
        if hashlib.sha256(file.read_bytes()).hexdigest() != sample["hashes"][kind]:
            raise SystemExit(f"Capture identity changed: {file}")
        images.append(Image.open(file).convert("RGB"))
    measurements = json.loads((root / "evidence" / sample["measurementPath"]).read_text(encoding="utf-8"))
    for glyph in measurements["glyphs"]:
        heights = []
        for image in images:
            crop = image.crop(glyph["crop"])
            mask = Image.new("1", crop.size)
            mask.putdata([max(rgb) < 150 and max(rgb) - min(rgb) < 90 for rgb in crop.getdata()])
            bounds = mask.getbbox()
            heights.append(None if bounds is None else bounds[3] - bounds[1])
        error = abs(heights[1] - heights[0]) / heights[0] * 100 if all(heights) else None
        results.append({"width": sample["width"], "mode": sample["mode"], "state": sample["state"],
                        "id": glyph["id"], "heights": heights, "errorPercent": error,
                        "withinInitial5Percent": error is not None and error <= 5})
report = {"schemaVersion": 1, "visualAcceptance": "not-evaluated",
          "method": "Recorded text-only crops; dark ink max channel <150, channel spread <90. Inspect glyph shapes separately.",
          "checks": results}
args.output.parent.mkdir(parents=True, exist_ok=True)
with args.output.open("x", encoding="utf-8") as stream:
    json.dump(report, stream, ensure_ascii=False, indent=2)
print(json.dumps({"checks": len(results),
                  "differences": sum(not row["withinInitial5Percent"] for row in results)}))
