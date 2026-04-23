<script lang="ts">
  import { Slider } from "bits-ui";
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

  function onValueChange(vals: number[]) {
    const v = vals[0];
    previewT.set(v);
    if (!dragging) return;
    const now = performance.now();
    if (now - lastTick < THROTTLE_MS) return;
    lastTick = now;
    requestPreview(v, DOWNSCALE_WIDTH);
  }

  function onDown() {
    dragging = true;
  }

  function onUp() {
    if (!dragging) return;
    dragging = false;
    // Full-res frame on release
    requestPreview($previewT);
  }
</script>

<!-- svelte-ignore a11y-no-static-element-interactions -->
<section class="seekbar" on:pointerdown={onDown} on:pointerup={onUp}>
  <Slider.Root
    min={0}
    max={duration}
    step={0.1}
    value={[$previewT]}
    disabled={!$activityInfo}
    {onValueChange}
    class="slider"
    let:thumbs
  >
    <Slider.Range class="range" />
    {#each thumbs as thumb}
      <Slider.Thumb {thumb} class="thumb" />
    {/each}
  </Slider.Root>
  <div class="labels">
    <span>{fmt($previewT)}</span>
    <span>{fmt(duration)}</span>
  </div>
</section>

<style>
  .seekbar {
    padding: 0.5rem 1rem;
    background: var(--bg-raised);
  }
  :global(.slider) {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
    height: 20px;
    cursor: pointer;
  }
  :global(.slider[data-disabled]) {
    opacity: 0.4;
    cursor: not-allowed;
  }
  :global(.slider)::before {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    height: 4px;
    background: var(--bg-control);
    border-radius: 2px;
  }
  :global(.range) {
    position: absolute;
    height: 4px;
    background: var(--accent-teal);
    border-radius: 2px;
    left: 0;
  }
  :global(.thumb) {
    display: block;
    width: 14px;
    height: 14px;
    background: var(--text);
    border: 2px solid var(--accent-teal);
    border-radius: 50%;
    position: absolute;
    transform: translateX(-50%);
    cursor: grab;
  }
  :global(.thumb:focus) {
    outline: none;
    box-shadow: 0 0 0 3px rgba(68, 170, 204, 0.4);
  }
  :global(.thumb:active) {
    cursor: grabbing;
  }
  .labels {
    display: flex;
    justify-content: space-between;
    font-size: 0.85rem;
    color: var(--text-dim);
    margin-top: 0.25rem;
  }
</style>
