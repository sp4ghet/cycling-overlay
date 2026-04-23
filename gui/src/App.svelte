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
    onLayoutReloaded,
    onLayoutError,
  } from "./lib/tauri";
  import { requestPreview } from "./lib/preview-dispatcher";
  import { session, previewT } from "./lib/stores";
  import { ffmpegMissing, cliMissing } from "./lib/runtime-stores";

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

    probeFfmpeg(s.cli_path_override ?? undefined)
      .then(() => ffmpegMissing.set(false))
      .catch(() => ffmpegMissing.set(true));
    probeCli(s.cli_path_override ?? undefined)
      .then(() => cliMissing.set(false))
      .catch(() => cliMissing.set(true));

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
    // v1: no dedicated override for ffmpeg (the plan defers this). Button
    // currently just re-probes with the CLI override path, in case the user
    // fixed PATH externally and wants to dismiss the banner.
    probeFfmpeg()
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
</style>
