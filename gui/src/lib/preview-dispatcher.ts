import { previewFrame } from "./tauri";
import { previewImage, previewBusy } from "./stores";

let nextId = 0;
let lastReceived = -1;

/**
 * Request a preview frame at `t` (seconds). Requests issued after a previous
 * call win; stale responses (ones whose id is below the highest id we've
 * already applied) are dropped. This lets callers fire off many scrub-tick
 * requests during drag without worrying about out-of-order arrivals.
 */
export async function requestPreview(t: number, downscaleWidth?: number): Promise<void> {
  const id = nextId++;
  previewBusy.set(true);
  try {
    const url = await previewFrame(t, downscaleWidth);
    if (id > lastReceived) {
      lastReceived = id;
      previewImage.set(url);
    }
  } catch (e) {
    console.error("preview_frame failed:", e);
  } finally {
    previewBusy.set(false);
  }
}

// Test hook — reset module state between tests.
export function __reset(): void {
  nextId = 0;
  lastReceived = -1;
}
