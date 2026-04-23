<script lang="ts">
  import { onMount } from "svelte";
  import { Progress, Collapsible } from "bits-ui";
  import { exportStatus, exportProgress, exportLog } from "../lib/stores";
  import { onExportProgress, onExportLog, onExportDone, cancelExport } from "../lib/tauri";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  let collapsed = true;

  onMount(() => {
    const unlistens: UnlistenFn[] = [];

    onExportProgress((p) => {
      exportProgress.set(p);
    }).then((u) => unlistens.push(u));

    onExportLog((p) => {
      exportLog.update((l) => {
        const next = [...l, p.line];
        return next.length > 500 ? next.slice(next.length - 500) : next;
      });
    }).then((u) => unlistens.push(u));

    onExportDone((p) => {
      exportStatus.set(p.status);
      if (p.message) {
        exportLog.update((l) => [...l, `[export-done ${p.status}] ${p.message}`]);
      }
      // Auto-expand the log on non-success so the user sees the CLI's
      // stderr (which contains the actual ffmpeg/render failure). Without
      // this the error is invisible behind a collapsed pane.
      if (p.status !== "success") {
        collapsed = false;
      }
    }).then((u) => unlistens.push(u));

    return () => unlistens.forEach((u) => u());
  });

  $: pct = $exportProgress
    ? Math.round(($exportProgress.frame / Math.max(1, $exportProgress.total)) * 100)
    : 0;

  $: etaLabel =
    $exportProgress?.eta_seconds != null ? `${Math.round($exportProgress.eta_seconds)}s` : "—";

  // Stay visible through every non-idle state so an immediate failure
  // (e.g. the CLI rejecting argv before the first progress line) still
  // shows the FAILED badge + auto-expanded log. Prior condition hid the
  // footer whenever $exportProgress was null, which silently swallowed
  // those early failures.
  $: showFooter = $exportStatus !== "idle";
</script>

<footer class="footer" class:running={$exportStatus === "running"}>
  {#if showFooter}
    <Collapsible.Root open={!collapsed} onOpenChange={(v) => (collapsed = !v)} class="collapsible">
      <div class="top">
        <Progress.Root max={100} value={pct} class="bar">
          <div class="fill" style="width: {pct}%"></div>
        </Progress.Root>

        <div class="stats">
          {#if $exportStatus === "error"}
            <span class="status err">failed</span>
          {:else if $exportStatus === "canceled"}
            <span class="status cancel">canceled</span>
          {:else if $exportStatus === "success"}
            <span class="status ok">done</span>
          {/if}
          {#if $exportProgress}
            {$exportProgress.frame} / {$exportProgress.total}
            · {$exportProgress.fps.toFixed(1)} fps · ETA {etaLabel}
            · {pct}%
          {:else}
            starting…
          {/if}
        </div>

        {#if $exportStatus === "running"}
          <button class="cancel" on:click={cancelExport}>Cancel</button>
        {/if}
        <Collapsible.Trigger class="toggle">
          {collapsed ? "Show log" : "Hide log"}
        </Collapsible.Trigger>
      </div>

      <Collapsible.Content>
        <div class="log">
          {#each $exportLog as line}
            <div class="logline">{line}</div>
          {/each}
          {#if $exportLog.length === 0}
            <div class="logline muted">(no log yet)</div>
          {/if}
        </div>
      </Collapsible.Content>
    </Collapsible.Root>
  {/if}
</footer>

<style>
  .footer {
    background: var(--bg-raised);
  }
  :global(.collapsible) {
    display: flex;
    flex-direction: column;
  }
  .top {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    padding: 0.5rem 1rem;
  }
  :global(.bar) {
    flex: 1;
    background: var(--bg-muted);
    height: 10px;
    border-radius: 5px;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent-teal);
    transition: width 0.1s linear;
  }
  .stats {
    font-size: 0.85rem;
    color: var(--text-muted);
    min-width: 20ch;
    text-align: right;
    display: flex;
    gap: 0.5rem;
    align-items: center;
    justify-content: flex-end;
  }
  .status {
    font-weight: 700;
    text-transform: uppercase;
    font-size: 0.75rem;
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
  }
  .status.err {
    background: var(--danger);
    color: white;
  }
  .status.cancel {
    background: var(--status-cancel);
    color: white;
  }
  .status.ok {
    background: var(--status-ok);
    color: white;
  }
  .cancel {
    background: var(--danger);
    color: white;
    border: 0;
    padding: 0.3rem 0.8rem;
    cursor: pointer;
  }
  .cancel:hover {
    background: var(--danger-hover);
  }
  :global(.toggle) {
    background: var(--bg-control);
    color: var(--text-muted);
    border: 0;
    padding: 0.3rem 0.8rem;
    cursor: pointer;
    font-family: inherit;
  }
  .log {
    max-height: 200px;
    overflow-y: auto;
    font-family: ui-monospace, "Cascadia Code", Menlo, monospace;
    font-size: 12px;
    padding: 0.5rem 1rem;
    background: var(--bg-base);
    border-top: 1px solid var(--bg-overlay);
  }
  .logline {
    color: var(--text-dim);
    white-space: pre-wrap;
  }
  .muted {
    color: var(--text-disabled);
  }
</style>
