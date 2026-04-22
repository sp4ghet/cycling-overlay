<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import {
    session,
    activityInfo,
    layoutInfo,
    previewT,
    previewImage,
  } from "../lib/stores";
  import { loadActivity, loadLayout, watchLayout, previewFrame } from "../lib/tauri";
  import CodecSelect from "./CodecSelect.svelte";

  async function pickInput() {
    const path = await open({
      multiple: false,
      filters: [{ name: "Activity", extensions: ["fit", "gpx"] }],
    });
    if (typeof path !== "string") return;
    try {
      const info = await loadActivity(path);
      activityInfo.set(info);
      session.update((s) => ({
        ...s,
        input_path: path,
        from_seconds: 0,
        to_seconds: info.duration_seconds,
      }));
      previewT.set(0);
      try {
        const url = await previewFrame(0);
        previewImage.set(url);
      } catch (e) {
        // Preview fails if no layout loaded — fine.
        console.debug("initial preview skipped:", e);
      }
    } catch (e) {
      console.error("load_activity failed:", e);
    }
  }

  async function pickLayout() {
    const path = await open({
      multiple: false,
      filters: [{ name: "Layout JSON", extensions: ["json"] }],
    });
    if (typeof path !== "string") return;
    try {
      const info = await loadLayout(path);
      layoutInfo.set(info);
      session.update((s) => ({ ...s, layout_path: path }));
      await watchLayout(path);
      if ($activityInfo) {
        try {
          const url = await previewFrame($previewT);
          previewImage.set(url);
        } catch (e) {
          console.debug("preview after layout load failed:", e);
        }
      }
    } catch (e) {
      console.error("load_layout failed:", e);
    }
  }

  async function pickOutput() {
    const path = await save({
      filters: [{ name: "Video", extensions: ["mov", "mp4", "mkv"] }],
    });
    if (typeof path === "string") {
      session.update((s) => ({ ...s, output_path: path }));
    }
  }
</script>

<aside class="sidebar">
  <div class="row">
    <label>Input</label>
    <button on:click={pickInput}>Browse…</button>
    <div class="path">{$session.input_path ?? "—"}</div>
    {#if $activityInfo}
      <div class="meta">
        {Math.round($activityInfo.duration_seconds)}s · {$activityInfo.sample_count} samples
      </div>
    {/if}
  </div>

  <div class="row">
    <label>Layout</label>
    <button on:click={pickLayout}>Browse…</button>
    <div class="path">{$session.layout_path ?? "—"}</div>
    {#if $layoutInfo}
      <div class="meta">
        {$layoutInfo.width}×{$layoutInfo.height} @ {$layoutInfo.fps}fps · {$layoutInfo.widget_count} widgets
      </div>
    {/if}
  </div>

  <div class="row">
    <label>Output</label>
    <button on:click={pickOutput}>Browse…</button>
    <div class="path">{$session.output_path ?? "—"}</div>
  </div>

  <CodecSelect />

  <div class="row">
    <label>Time range (seconds)</label>
    <div class="time-row">
      <input
        type="number"
        min="0"
        step="1"
        bind:value={$session.from_seconds}
        placeholder="from"
      />
      <span>→</span>
      <input
        type="number"
        min="0"
        step="1"
        bind:value={$session.to_seconds}
        placeholder="to"
      />
    </div>
  </div>
</aside>

<style>
  .sidebar {
    padding: 1rem;
    background: #222;
    height: 100%;
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-bottom: 1.25rem;
  }
  .row > label {
    font-weight: 600;
    color: #ddd;
  }
  .path {
    font-size: 0.85rem;
    color: #888;
    word-break: break-all;
  }
  .meta {
    font-size: 0.8rem;
    color: #6ac;
  }
  button {
    padding: 0.3rem 0.8rem;
    background: #333;
    color: #eee;
    border: 1px solid #444;
    cursor: pointer;
    align-self: flex-start;
  }
  button:hover { background: #3a3a3a; }
  .time-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .time-row input {
    flex: 1;
    min-width: 0;
    padding: 0.3rem;
    background: #333;
    color: #eee;
    border: 1px solid #444;
  }
  .time-row span { color: #888; }
</style>
