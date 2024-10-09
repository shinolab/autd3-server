<script lang="ts">
  import type { Options } from "./lib/UI/options";

  import { onMount } from "svelte";
  import { writable } from "svelte/store";

  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { TauriEvent } from "@tauri-apps/api/event";

  import LeftPanel from "./lib/LeftPanel.svelte";
  import RightPanel from "./lib/RightPanel.svelte";

  import License from "./lib/License.svelte";
  // @ts-ignore
  import Modal, { bind } from "svelte-simple-modal";
  const appWindow = getCurrentWebviewWindow();

  const licenseModal = writable<any>(null);
  const showLicenseModal = () => licenseModal.set(bind(License, {}));

  let options: null | Options = null;

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
    <Modal
      show={$licenseModal}
      closeButton={false}
      styleWindow={{ backgroundColor: "#101923" }}
    >
      <button on:click={showLicenseModal}>License</button>
    </Modal>
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
