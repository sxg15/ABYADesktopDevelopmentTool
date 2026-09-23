import fs from "node:fs";
import { execFileSync } from "node:child_process";

if (!fs.existsSync("CHANGELOG.md")) {
  console.error("CHANGELOG.md is required.");
  process.exit(1);
}

try {
  const changed = execFileSync("git", ["diff", "--name-only", "HEAD"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  })
    .split(/\r?\n/)
    .filter(Boolean)
    .map((value) => value.replaceAll("\\", "/"));
  const production = changed.some(
    (file) => file.startsWith("src/") || file.startsWith("src-tauri/src/"),
  );
  if (production && !changed.includes("CHANGELOG.md")) {
    console.error("Production code changed without CHANGELOG.md.");
    process.exit(1);
  }
} catch {
  // The initial uncommitted scaffold is validated by file existence.
}

console.log("Changelog policy validated.");
