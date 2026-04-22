<script lang="ts">
  import { previewImage, previewBusy } from "../lib/stores";
</script>

<section class="preview" class:busy={$previewBusy}>
  {#if $previewImage}
    <img src={$previewImage} alt="Preview frame" />
  {:else}
    <div class="empty">Load an activity and layout to see a preview.</div>
  {/if}
</section>

<style>
  .preview {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    justify-content: center;
    align-items: center;
    background-color: #333;
    background-image:
      linear-gradient(45deg, #444 25%, transparent 25%),
      linear-gradient(-45deg, #444 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #444 75%),
      linear-gradient(-45deg, transparent 75%, #444 75%);
    background-size: 20px 20px;
    background-position: 0 0, 0 10px, 10px -10px, -10px 0;
  }
  .preview img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    display: block;
  }
  .empty { color: #888; font-size: 0.95rem; }
  .busy { opacity: 0.85; }
</style>
