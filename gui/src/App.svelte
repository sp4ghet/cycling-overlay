<script lang="ts">
  import { onMount } from "svelte";
  import PreviewPane from "./components/PreviewPane.svelte";
  import Seekbar from "./components/Seekbar.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import ExportFooter from "./components/ExportFooter.svelte";
  import { sessionLoad } from "./lib/tauri";
  import { session } from "./lib/stores";

  onMount(async () => {
    try {
      const s = await sessionLoad();
      session.set(s);
    } catch (e) {
      console.error("session_load failed:", e);
    }
  });
</script>

<div class="app">
  <main class="main">
    <PreviewPane />
    <Seekbar />
  </main>
  <Sidebar />
  <ExportFooter />
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: 1fr 320px;
    grid-template-rows: 1fr auto;
    grid-template-areas:
      "main sidebar"
      "footer footer";
    height: 100vh;
    width: 100vw;
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
</style>
