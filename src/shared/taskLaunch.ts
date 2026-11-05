import { invoke } from "@tauri-apps/api/core";
import type { ArchiveOption } from "./types";
export interface TaskLaunchSources {
  entries: { id: string; origin: "task" | "global"; archive: ArchiveOption }[];
  warnings: string[];
  candidate: { version: string; path: string } | null;
  previewVersion?: string | null;
  globalRoot: string;
  player: { selectedPath: string; selectedHash: string; previous: { path?: string; hash?: string } | null; matches: boolean };
}
export const taskLaunchApi = {
  sources: (taskId: string) => invoke<TaskLaunchSources>("task_launch_sources", { taskId }),
  adopt: (taskId: string, expectedHash: string) => invoke("adopt_task_player", { taskId, expectedHash }),
  export: (taskId: string, version: string) => invoke<{ destination: string; version: string; outcome: string }>("export_task_archive", { taskId, version }),
};
