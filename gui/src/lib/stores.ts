import { writable, type Writable } from "svelte/store";
import type {
  SessionState,
  ActivityInfo,
  LayoutInfo,
  ProgressPayload,
} from "./types";
import { sessionSave } from "./tauri";

const defaultSession: SessionState = {
  input_path: null,
  layout_path: null,
  output_path: null,
  codec: "prores4444",
  quality: 20,
  chromakey: "#00ff00",
  from_seconds: 0,
  to_seconds: null,
  cli_path_override: null,
};

export const session: Writable<SessionState> = writable(defaultSession);
export const activityInfo: Writable<ActivityInfo | null> = writable(null);
export const layoutInfo: Writable<LayoutInfo | null> = writable(null);

export const previewT: Writable<number> = writable(0);
export const previewImage: Writable<string | null> = writable(null);
export const previewBusy: Writable<boolean> = writable(false);

export type ExportStatus = "idle" | "running" | "success" | "canceled" | "error";
export const exportStatus: Writable<ExportStatus> = writable("idle");
export const exportProgress: Writable<ProgressPayload | null> = writable(null);
export const exportLog: Writable<string[]> = writable([]);

// Debounced auto-save: write to disk ~500ms after last change.
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let skipFirst = true;
session.subscribe((s) => {
  if (skipFirst) {
    skipFirst = false;
    return;
  }
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    sessionSave(s).catch((e) => console.error("session_save failed:", e));
  }, 500);
});
