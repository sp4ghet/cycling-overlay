<script lang="ts">
  import { Select } from "bits-ui";
  import type { Selected } from "bits-ui";
  import { session } from "../lib/stores";

  const CODECS = [
    { id: "prores4444", label: "prores", desc: "transparent alpha (largest files)" },
    { id: "h264_nvenc", label: "h264_nvenc", desc: "fast, NVIDIA GPU acceleration" },
    { id: "hevc_nvenc", label: "hevc_nvenc", desc: "smallest files, NVIDIA GPU acceleration" },
    { id: "h264", label: "h264", desc: "no NVIDIA GPU, small filesize (CPU encode)" },
  ];

  $: isAlpha = $session.codec === "prores4444";
  $: qualityLabel = isAlpha ? "qscale (lower = larger)" : "CRF / CQ (lower = better)";

  $: selectedCodec = CODECS.find((c) => c.id === $session.codec) ?? CODECS[0];

  function onSelectedChange(val: Selected<string> | undefined) {
    if (val) session.update((s) => ({ ...s, codec: val.value }));
  }
</script>

<div class="field">
  <label for="codec-select">Codec</label>
  <Select.Root
    selected={{ value: selectedCodec.id, label: selectedCodec.label }}
    {onSelectedChange}
    items={CODECS.map((c) => ({ value: c.id, label: c.label }))}
  >
    <Select.Trigger id="codec-select" class="select-trigger">
      <Select.Value placeholder="Select codec" />
      <span class="chevron">▾</span>
    </Select.Trigger>
    <Select.Content class="select-content">
      {#each CODECS as c}
        <Select.Item value={c.id} label={c.label} class="select-item">
          <span class="item-label">{c.label}</span>
          <span class="item-desc">{c.desc}</span>
        </Select.Item>
      {/each}
    </Select.Content>
  </Select.Root>
</div>

<div class="field">
  <label for="quality-input">Quality ({qualityLabel})</label>
  <input id="quality-input" type="number" min="0" max="51" bind:value={$session.quality} />
</div>

{#if !isAlpha}
  <div class="field">
    <label for="chromakey-input">Chromakey color</label>
    <input id="chromakey-input" type="color" bind:value={$session.chromakey} />
  </div>
{/if}

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-bottom: 0.9rem;
  }
  .field > label {
    font-weight: 600;
    color: var(--text-muted);
    font-size: 0.9rem;
  }

  :global(.select-trigger) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 0.3rem 0.5rem;
    background: var(--bg-control);
    color: var(--text);
    border: 1px solid var(--border);
    cursor: pointer;
    font-size: 0.9rem;
    font-family: inherit;
  }
  :global(.select-trigger:hover) {
    background: var(--bg-control-hover);
  }
  :global(.select-trigger:focus) {
    outline: 1px solid var(--accent-dim);
  }

  :global(.select-content) {
    background: var(--bg-overlay);
    border: 1px solid var(--border);
    padding: 0.2rem;
    z-index: 50;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5);
    min-width: var(--bits-select-anchor-width);
  }
  :global(.select-item) {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    padding: 0.35rem 0.5rem;
    cursor: pointer;
  }
  :global(.select-item[data-highlighted]) {
    background: var(--bg-control);
  }
  :global(.select-item[data-selected]) {
    background: var(--accent-selection);
  }

  .chevron {
    color: var(--text-faint);
    font-size: 0.75rem;
  }
  .item-label {
    color: var(--text);
    font-size: 0.9rem;
  }
  .item-desc {
    color: var(--text-faint);
    font-size: 0.77rem;
  }

  input {
    padding: 0.3rem;
    background: var(--bg-control);
    color: var(--text);
    border: 1px solid var(--border);
    font-family: inherit;
  }
  input[type="color"] {
    width: 4rem;
    height: 2rem;
    padding: 0;
  }
</style>
