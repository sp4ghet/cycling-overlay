<script lang="ts">
  import { activityInfo, previewT } from "../lib/stores";
  import { requestPreview } from "../lib/preview-dispatcher";

  $: duration = $activityInfo?.duration_seconds ?? 0;

  let dragging = false;
  const DOWNSCALE_WIDTH = 800;
  const THROTTLE_MS = 67; // ~15fps
  let lastTick = 0;

  function fmt(s: number): string {
    const t = Math.max(0, Math.floor(s));
    const h = Math.floor(t / 3600);
    const m = Math.floor((t % 3600) / 60);
    const sec = t % 60;
    return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
  }

  function onInput(e: Event) {
    const v = Number((e.target as HTMLInputElement).value);
    previewT.set(v);
    if (!dragging) return;
    const now = performance.now();
    if (now - lastTick < THROTTLE_MS) return;
    lastTick = now;
    requestPreview(v, DOWNSCALE_WIDTH);
  }

  function onDown() { dragging = true; }

  function onUp() {
    if (!dragging) return;
    dragging = false;
    // Full-res frame on release
    requestPreview($previewT);
  }
</script>

<section class="seekbar">
  <input
    type="range"
    min="0"
    max={duration}
    step="0.1"
    value={$previewT}
    on:input={onInput}
    on:mousedown={onDown}
    on:mouseup={onUp}
    on:touchstart={onDown}
    on:touchend={onUp}
    disabled={!$activityInfo}
  />
  <div class="labels">
    <span>{fmt($previewT)}</span>
    <span>{fmt(duration)}</span>
  </div>
</section>

<style>
  .seekbar {
    padding: 0.5rem 1rem;
    background: #1a1a1a;
  }
  .seekbar input {
    width: 100%;
  }
  .labels {
    display: flex;
    justify-content: space-between;
    font-size: 0.85rem;
    color: #aaa;
    margin-top: 0.25rem;
  }
</style>
