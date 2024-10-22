<script lang="ts">
  import type { Options } from "./lib/UI/options";

  import { onMount } from "svelte";

  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { TauriEvent } from "@tauri-apps/api/event";
  import { resolveResource } from "@tauri-apps/api/path";

  import LeftPanel from "./lib/LeftPanel.svelte";
  import RightPanel from "./lib/RightPanel.svelte";

  const appWindow = getCurrentWebviewWindow();

  const showLicense = async () => {
    const resourcePath = await resolveResource("LICENSE");
    await invoke("showfile", {
      path: resourcePath,
    });
  };

  let options: null | Options = $state(null);

  onMount(async () => {
    options = await invoke("load_settings", {});
  });

  const handleUnload = async () => {
    if (options) {
      let args = {
        options: JSON.stringify(options),
      };
      console.log("Saving settings");
      await invoke("save_settings", args);
      console.log("Settings saved");
    }
  };

  appWindow.listen(TauriEvent.WINDOW_CLOSE_REQUESTED, async () => {
    await handleUnload();
    await appWindow.destroy();
  });
</script>

<main class="container">
  <div>
    {#if options}
      <LeftPanel {options} />
    {/if}
    <RightPanel />
  </div>

  <footer class="right-align">
    <button onclick={showLicense}>License</button>
  </footer>
</main>

<style>
  .container {
    display: flex;
    flex-flow: column;
    flex-shrink: 0;

    width: 100%;
    padding: 10px 10px 0px 10px;

    justify-content: stretch;

    height: 100vh;
  }

  div {
    display: flex;
    gap: 10px;

    height: calc(100% - 26px);

    width: 100%;
  }

  button {
    font-size: 12px;
    color: #ffffff;
    text-decoration: underline;
  }

  footer {
    width: 100%;
    height: 26px;

    text-align: right;

    padding: 0;
    margin: 0;
    box-sizing: border-box;
    border: 0;
  }
</style>
