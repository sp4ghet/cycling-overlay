<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import {
    session,
    activityInfo,
    layoutInfo,
    previewT,
    previewImage,
    exportStatus,
    exportProgress,
    exportLog,
  } from "../lib/stores";
  import {
    loadActivity,
    loadLayout,
    watchLayout,
    previewFrame,
    startExport,
    probeCli,
  } from "../lib/tauri";
  import CodecSelect from "./CodecSelect.svelte";
  import { ffmpegMissing, cliMissing, loadError } from "../lib/runtime-stores";
  import { parseTimeSpec, formatTimeSpec } from "../lib/time";

  // Uncontrolled time-range inputs: we grab DOM refs via bind:this and
  // write `.value` manually when the session changes — but skip the write
  // while the user is typing (document.activeElement guard) so the cursor
  // doesn't warp on every auto-save tick of the session store.
  let fromInput: HTMLInputElement | undefined;
  let toInput: HTMLInputElement | undefined;
  let fromInvalid = false;
  let toInvalid = false;

  $: if (fromInput && document.activeElement !== fromInput) {
    const next = formatTimeSpec($session.from_seconds);
    if (fromInput.value !== next) fromInput.value = next;
  }
  $: if (toInput && document.activeElement !== toInput) {
    const next = $session.to_seconds != null ? formatTimeSpec($session.to_seconds) : "";
    if (toInput.value !== next) toInput.value = next;
  }

  function commitFrom() {
    if (!fromInput) return;
    const val = parseTimeSpec(fromInput.value);
    if (val == null) {
      fromInvalid = true;
      return;
    }
    fromInvalid = false;
    if (val !== $session.from_seconds) {
      session.update((s) => ({ ...s, from_seconds: val }));
    }
  }

  function commitTo() {
    if (!toInput) return;
    const val = parseTimeSpec(toInput.value);
    if (val == null) {
      toInvalid = true;
      return;
    }
    toInvalid = false;
    if (val !== $session.to_seconds) {
      session.update((s) => ({ ...s, to_seconds: val }));
    }
  }

  function onKeydown(e: KeyboardEvent, commit: () => void) {
    if (e.key === "Enter") {
      (e.target as HTMLInputElement).blur(); // triggers on:change → commit
      commit();
    }
  }

  async function pickInput() {
    const path = await open({
      multiple: false,
      filters: [{ name: "Activity", extensions: ["fit", "gpx"] }],
    });
    if (typeof path !== "string") return;
    // Clear the stale preview immediately; if load + render succeed we'll
    // replace it below, but on failure the pane should not keep showing
    // the previous activity's frame.
    previewImage.set(null);
    try {
      const info = await loadActivity(path);
      loadError.set(null);
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
      loadError.set(`Failed to load activity ${path}: ${e}`);
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
      loadError.set(null);
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
      loadError.set(`Failed to load layout ${path}: ${e}`);
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

  async function doExport() {
    if (
      !$session.input_path ||
      !$session.layout_path ||
      !$session.output_path ||
      $session.to_seconds == null
    ) {
      console.warn("missing fields; cannot export");
      return;
    }
    try {
      const cliPath = await probeCli($session.cli_path_override ?? undefined);
      exportProgress.set(null);
      exportLog.set([]);
      exportStatus.set("running");
      await startExport({
        cli_path: cliPath,
        input: $session.input_path,
        layout: $session.layout_path,
        output: $session.output_path,
        codec: $session.codec,
        quality: $session.quality,
        chromakey: $session.chromakey,
        from_seconds: $session.from_seconds,
        to_seconds: $session.to_seconds,
        ffmpeg_path_override: $session.ffmpeg_path_override,
      });
    } catch (e) {
      exportStatus.set("error");
      console.error("start_export failed:", e);
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
    <label>Time range (HH:MM:SS)</label>
    <div class="time-row">
      <input
        bind:this={fromInput}
        type="text"
        inputmode="numeric"
        on:change={commitFrom}
        on:keydown={(e) => onKeydown(e, commitFrom)}
        class:invalid={fromInvalid}
        placeholder="00:00:00"
      />
      <span>→</span>
      <input
        bind:this={toInput}
        type="text"
        inputmode="numeric"
        on:change={commitTo}
        on:keydown={(e) => onKeydown(e, commitTo)}
        class:invalid={toInvalid}
        placeholder="00:00:00"
      />
    </div>
  </div>

  <button
    class="primary"
    on:click={doExport}
    disabled={$exportStatus === "running"
      || !$session.input_path
      || !$session.layout_path
      || !$session.output_path
      || $session.to_seconds == null
      || $ffmpegMissing
      || $cliMissing}
  >
    {$exportStatus === "running" ? "Exporting…" : "Export"}
  </button>
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
    font-family: ui-monospace, "Cascadia Code", Menlo, monospace;
  }
  .time-row input.invalid { border-color: #a33; }
  .time-row span { color: #888; }
  .primary {
    margin-top: 1rem;
    padding: 0.6rem 1rem;
    background: #4a6;
    color: white;
    border: 0;
    cursor: pointer;
    font-weight: 600;
  }
  .primary:hover:not(:disabled) { background: #5b7; }
  .primary:disabled { background: #333; color: #888; cursor: not-allowed; }
</style>
