<script lang="ts">
  import type { TwinCATOptions } from "./options.ts";

  import { Child } from "@tauri-apps/api/shell";

  import { invoke } from "@tauri-apps/api";

  import Button from "./utils/Button.svelte";
  import CheckBox from "./utils/CheckBox.svelte";
  import NumberInput from "./utils/NumberInput.svelte";
  import IpInput from "./utils/IpInput.svelte";

  export let twincatOptions: TwinCATOptions;

  let child: null | Child = null;
  let running = false;

  let handleRunClick = async () => {
    running = true;

    if (twincatOptions) {
      let args = {
        twincatOptions: JSON.stringify(twincatOptions),
      };
      try {
        await invoke("run_twincat_server", args);
      } catch (err) {
        alert(err);
      }
    }

    running = false;
  };

  let handleOpenXaeShellClick = async () => {
    try {
      await invoke("open_xae_shell", {});
    } catch (err) {
      alert(err);
    }
  };

  let handleCopyAUTDXmlClick = async () => {
    try {
      await invoke("copy_autd_xml", {});
    } catch (err) {
      alert(err);
    }
  };
</script>

<div class="ui">
  <label for="client">Client IP address:</label>
  <IpInput id="client" bind:value={twincatOptions.client} />

  <label for="sync0">Sync0 cycle time:</label>
  <NumberInput id="sync0" bind:value={twincatOptions.sync0} min="1" step="1" />

  <label for="task">Send task cycle time:</label>
  <NumberInput id="task" bind:value={twincatOptions.task} min="1" step="1" />

  <label for="base">CPU base time:</label>
  <NumberInput id="base" bind:value={twincatOptions.base} min="1" step="1" />

  <label for="keep">Keep XAE Shell open:</label>
  <CheckBox id="keep" bind:checked={twincatOptions.keep} />

  <Button label="Run" click={handleRunClick} disabled={running} />
  <Button label="Open XAE Shell" click={handleOpenXaeShellClick} />
  <Button label="Copy AUTD.xml" click={handleCopyAUTDXmlClick} />
</div>

<style>
  .ui {
    display: grid;
    grid-template-columns: auto 120px;
    grid-gap: 10px 0px;
    align-items: right;
  }
  label {
    text-align: right;
    padding-right: 10px;
  }
</style>
