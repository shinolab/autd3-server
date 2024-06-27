<script lang="ts">
  import type { TwinCATOptions } from "./options.ts";

  import { Command, Child } from "@tauri-apps/api/shell";

  import { invoke } from "@tauri-apps/api";
  import { consoleOutputQueue } from "./console_output.ts";

  import Button from "./utils/Button.svelte";
  import CheckBox from "./utils/CheckBox.svelte";
  import NumberInput from "./utils/NumberInput.svelte";
  import IpInput from "./utils/IpInput.svelte";

  export let twincatOptions: TwinCATOptions;

  let command;
  let child: null | Child = null;
  let running = false;

  let handleRunClick = async () => {
    await handleCloseClick();
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

    if (twincatOptions.lightweight) {
      const args: string[] = ["-p", twincatOptions.lightweight_port.toString()];
      command = Command.sidecar("TwinCATAUTDServerLightweight", args);
      child = await command.spawn();
      command.stdout.on("data", (line) =>
        consoleOutputQueue.update((v) => {
          return [...v, line.trimEnd()];
        }),
      );
      command.stderr.on("data", (line) =>
        consoleOutputQueue.update((v) => {
          return [...v, line.trimEnd()];
        }),
      );
      command.on("error", () => handleCloseClick());
      command.on("close", () => handleCloseClick());
    }

    running = false;
  };

  let handleCloseClick = async () => {
    if (child !== null) {
      await child.kill();
      child = null;
    }
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

  <label for="lightweight">Lightweight mode:</label>
  <CheckBox id="lightweight" bind:checked={twincatOptions.lightweight} />

  {#if twincatOptions.lightweight}
    <label for="lightweight_port">Lightweight port:</label>
    <NumberInput
      id="lightweight_port"
      bind:value={twincatOptions.lightweight_port}
      min="0"
      max="65535"
      step="1"
    />
  {/if}

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
