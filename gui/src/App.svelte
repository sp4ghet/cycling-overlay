<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  let message = "loading…";
  onMount(async () => {
    try {
      message = await invoke<string>("hello_from_rust");
    } catch (e) {
      message = `error: ${e}`;
    }
  });
</script>

<main><h1>{message}</h1></main>

<style>
  main { padding: 2rem; font-family: system-ui, -apple-system, sans-serif; }
</style>
