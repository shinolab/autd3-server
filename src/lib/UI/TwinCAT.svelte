<script lang="ts">
  import type { TwinCATOptions } from "./options.ts";

  import { Command, Child } from "@tauri-apps/plugin-shell";

  import { invoke } from "@tauri-apps/api/core";
  import { consoleOutputQueue } from "./console_output.ts";

  import Button from "./utils/Button.svelte";
  import CheckBox from "./utils/CheckBox.svelte";
  import NumberInput from "./utils/NumberInput.svelte";
  import IpInput from "./utils/IpInput.svelte";

  interface Props {
    twincatOptions: TwinCATOptions;
  }

  let { twincatOptions = $bindable() }: Props = $props();

  let baseUs = $state(twincatOptions.base * 500);
  $effect(() => {
    twincatOptions.base = baseUs / 500;
  });
  let sync0Us = $state(twincatOptions.sync0 * 500);
  $effect(() => {
    twincatOptions.sync0 = sync0Us / 500;
  });
  let taskUs = $state(twincatOptions.task * 500);
  $effect(() => {
    twincatOptions.task = taskUs / 500;
  });

  let command;
  let child: null | Child = null;
  let running = $state(false);

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

  <label for="sync0Us">Sync0 cycle time [us]:</label>
  <NumberInput id="sync0Us" bind:value={sync0Us} min="500" step="500" />

  <label for="taskUs">Send task cycle time [us]:</label>
  <NumberInput id="taskUs" bind:value={taskUs} min="500" step="500" />

  <label for="baseUs">CPU base time [us]:</label>
  <NumberInput id="baseUs" bind:value={baseUs} min="500" step="500" />

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
