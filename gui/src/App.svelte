<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { get } from "svelte/store";
  import { open } from "@tauri-apps/plugin-dialog";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import PreviewPane from "./components/PreviewPane.svelte";
  import Seekbar from "./components/Seekbar.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import ExportFooter from "./components/ExportFooter.svelte";
  import StartupBanner from "./components/StartupBanner.svelte";
  import {
    sessionLoad,
    probeFfmpeg,
    probeCli,
    loadActivity,
    loadLayout,
    watchLayout,
    previewFrame,
    onLayoutReloaded,
    onLayoutError,
  } from "./lib/tauri";
  import { requestPreview } from "./lib/preview-dispatcher";
  import {
    session,
    previewT,
    previewImage,
    activityInfo,
    layoutInfo,
  } from "./lib/stores";
  import { ffmpegMissing, cliMissing, loadError } from "./lib/runtime-stores";

  let layoutError: string | null = null;
  const unlistens: UnlistenFn[] = [];

  onMount(async () => {
    let s;
    try {
      s = await sessionLoad();
      // Migrate legacy codec IDs: an earlier build stored `prores_4444`,
      // but clap actually accepts `prores4444`. Rewrite the field so the
      // dropdown + argv stay consistent.
      if (s.codec === "prores_4444") {
        s = { ...s, codec: "prores4444" };
      }
      session.set(s);
    } catch (e) {
      console.error("session_load failed:", e);
      return;
    }

    probeFfmpeg(s.ffmpeg_path_override ?? undefined)
      .then(() => ffmpegMissing.set(false))
      .catch(() => ffmpegMissing.set(true));
    probeCli(s.cli_path_override ?? undefined)
      .then(() => cliMissing.set(false))
      .catch(() => cliMissing.set(true));

    // Session-restore: re-hydrate the backend AppState from the persisted
    // paths. Without this, the sidebar shows the old paths but the backend
    // has no activity/layout loaded — so preview, watcher, and export are
    // all broken until the user re-picks the files manually.
    const restoreErrors: string[] = [];
    if (s.input_path) {
      try {
        const info = await loadActivity(s.input_path);
        activityInfo.set(info);
      } catch (e) {
        restoreErrors.push(`activity ${s.input_path} (${e})`);
        session.update((cur) => ({ ...cur, input_path: null }));
      }
    }
    if (s.layout_path) {
      try {
        const info = await loadLayout(s.layout_path);
        layoutInfo.set(info);
        await watchLayout(s.layout_path);
      } catch (e) {
        restoreErrors.push(`layout ${s.layout_path} (${e})`);
        session.update((cur) => ({ ...cur, layout_path: null }));
      }
    }
    if (restoreErrors.length > 0) {
      loadError.set(`Could not restore previous session: ${restoreErrors.join("; ")}`);
    }

    // Render an initial preview if both are loaded.
    if (get(activityInfo) && get(layoutInfo)) {
      const t0 = s.from_seconds ?? 0;
      previewT.set(t0);
      try {
        const url = await previewFrame(t0);
        previewImage.set(url);
      } catch (e) {
        console.debug("initial preview after restore failed:", e);
      }
    }

    const u1 = await onLayoutReloaded(async () => {
      layoutError = null;
      const t = get(previewT);
      try {
        await requestPreview(t);
      } catch (e) {
        console.error("preview after layout-reloaded failed:", e);
      }
    });
    const u2 = await onLayoutError((msg) => {
      layoutError = msg;
    });
    unlistens.push(u1, u2);
  });

  onDestroy(() => {
    unlistens.forEach((u) => u());
  });

  async function setCliPath() {
    const path = await open({ multiple: false });
    if (typeof path !== "string") return;
    session.update((s) => ({ ...s, cli_path_override: path }));
    probeCli(path)
      .then(() => cliMissing.set(false))
      .catch(() => cliMissing.set(true));
  }

  async function setFfmpegPath() {
    const path = await open({ multiple: false });
    if (typeof path !== "string") return;
    session.update((s) => ({ ...s, ffmpeg_path_override: path }));
    probeFfmpeg(path)
      .then(() => ffmpegMissing.set(false))
      .catch(() => ffmpegMissing.set(true));
  }
</script>

<div class="root">
  {#if $ffmpegMissing}
    <StartupBanner kind="ffmpeg" onSetPath={setFfmpegPath} />
  {/if}
  {#if $cliMissing}
    <StartupBanner kind="cli" onSetPath={setCliPath} />
  {/if}
  {#if $loadError}
    <div class="load-error" role="alert">
      <span>{$loadError}</span>
      <button class="dismiss" on:click={() => loadError.set(null)} aria-label="Dismiss">×</button>
    </div>
  {/if}
  {#if layoutError}
    <div class="layout-error" role="alert">
      <span>Layout parse error: {layoutError}</span>
    </div>
  {/if}

  <div class="app">
    <main class="main">
      <PreviewPane />
      <Seekbar />
    </main>
    <Sidebar />
    <ExportFooter />
  </div>
</div>

<style>
  .root {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
  }
  .app {
    flex: 1 1 auto;
    display: grid;
    grid-template-columns: 1fr 320px;
    grid-template-rows: 1fr auto;
    grid-template-areas:
      "main sidebar"
      "footer footer";
    min-height: 0;
  }
  .main {
    grid-area: main;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  :global(.sidebar) { grid-area: sidebar; overflow-y: auto; }
  :global(.footer)  { grid-area: footer; }
  .layout-error {
    background: #844;
    color: #ffeeee;
    padding: 0.4rem 1rem;
    font-size: 0.85rem;
  }
  .load-error {
    background: #a33;
    color: white;
    padding: 0.5rem 1rem;
    font-size: 0.9rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
  }
  .load-error .dismiss {
    background: transparent;
    border: 0;
    color: white;
    font-size: 1.3rem;
    line-height: 1;
    cursor: pointer;
    padding: 0 0.3rem;
  }
  .load-error .dismiss:hover { color: #eee; }
</style>
