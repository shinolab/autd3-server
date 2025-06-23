<script lang="ts">
  import {
    CpuBaseTimeValues,
    TwinCATVersionValues,
    type TwinCATOptions,
  } from "./options.ts";

  import { Command, Child } from "@tauri-apps/plugin-shell";

  import { invoke } from "@tauri-apps/api/core";
  import { consoleOutputQueue } from "./console_output.ts";

  import Button from "./utils/Button.svelte";
  import CheckBox from "./utils/CheckBox.svelte";
  import Select from "./utils/Select.svelte";
  import NumberInput from "./utils/NumberInput.svelte";
  import IpInput from "./utils/IpInput.svelte";

  import { Tooltip } from "@svelte-plugins/tooltips";

  interface Props {
    twincatOptions: TwinCATOptions;
  }

  let { twincatOptions = $bindable() }: Props = $props();

  let command;
  let child: null | Child = null;
  let running = $state(false);

  let handleRunClick = async () => {
    await handleCloseClick();
    running = true;

    const args = {
      twincatOptions: JSON.stringify(twincatOptions),
    };
    try {
      console.log("Running TwinCAT server with args:", args);
      await invoke("run_twincat_server", args);
    } catch (err) {
      alert(err);
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
  <label for="twincatVersion">TwinCAT Version:</label>
  <Select
    id="twincatVersion"
    bind:value={twincatOptions.version}
    values={TwinCATVersionValues}
  />

  <Tooltip content="If empty, use local TwinCAT.">
    <label for="client">Client IP address:</label>
  </Tooltip>
  <IpInput id="client" bind:value={twincatOptions.client} />

  <Tooltip content="If empty, use the first device found.">
    <label for="device_name">Ethernet device name:</label>
  </Tooltip>
  <input id="device_name" bind:value={twincatOptions.device_name} />

  <label for="sync0Us">Sync0 cycle in units of 500μs:</label>
  <NumberInput
    id="sync0Us"
    bind:value={twincatOptions.sync0}
    min="1"
    step="1"
  />

  <label for="taskUs">Send task cycle in units of CPU base time:</label>
  <NumberInput id="taskUs" bind:value={twincatOptions.task} min="1" step="1" />

  <label for="cpuBase">CPU base time:</label>
  <Select
    id="cpuBase"
    bind:value={twincatOptions.base}
    values={CpuBaseTimeValues}
  />

  <label for="keep">Keep XAE Shell open:</label>
  <CheckBox id="keep" bind:checked={twincatOptions.keep} />

  <label for="debug">Debug:</label>
  <CheckBox id="debug" bind:checked={twincatOptions.debug} />

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
  :global(.tooltip-container) {
    text-align: right;
  }

  input {
    border-radius: 3px;
    border: 1px solid var(--color-border-interactive-muted, #2b659b);
    background: var(--color-background-base-default, #101923);
    color: var(--color-text-base-default, #ffffff);
  }
</style>
