<script lang="ts">
  import { session } from "../lib/stores";

  const CODECS = [
    { id: "prores_4444", label: "prores",      desc: "transparent alpha (largest files)" },
    { id: "h264_nvenc",  label: "h264_nvenc",  desc: "fast, NVIDIA GPU acceleration" },
    { id: "hevc_nvenc",  label: "hevc_nvenc",  desc: "smallest files, NVIDIA GPU acceleration" },
    { id: "h264",        label: "h264",        desc: "no NVIDIA GPU, small filesize (CPU encode)" },
  ];

  $: isAlpha = $session.codec === "prores_4444";
  $: qualityLabel = isAlpha ? "qscale (lower = larger)" : "CRF / CQ (lower = better)";
</script>

<div class="field">
  <label for="codec-select">Codec</label>
  <select id="codec-select" bind:value={$session.codec}>
    {#each CODECS as c}
      <option value={c.id}>{c.label} — {c.desc}</option>
    {/each}
  </select>
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
    color: #ddd;
    font-size: 0.9rem;
  }
  select, input {
    padding: 0.3rem;
    background: #333;
    color: #eee;
    border: 1px solid #444;
  }
  input[type="color"] {
    width: 4rem;
    height: 2rem;
    padding: 0;
  }
</style>
