<script lang="ts">
  import type { SimulatorOptions } from "./options.ts";

  import { onMount } from "svelte";
  import { Command, Child } from "@tauri-apps/plugin-shell";
  import { invoke } from "@tauri-apps/api/core";
  import { consoleOutputQueue } from "./console_output.ts";
  import { appConfigDir } from "@tauri-apps/api/path";

  import Button from "./utils/Button.svelte";
  import CheckBox from "./utils/CheckBox.svelte";
  import NumberInput from "./utils/NumberInput.svelte";

  interface Props {
    simulatorOptions: SimulatorOptions;
  }

  let { simulatorOptions = $bindable() }: Props = $props();

  let appConfigDirPath: string;

  let command;
  let child: null | Child = $state(null);

  let handleRunClick = async () => {
    const setting_file = simulatorOptions.unity
      ? "unity_simulator_settings.json"
      : "simulator_settings.json";

    const args: string[] = [
      "-w",
      `${simulatorOptions.window_width},${simulatorOptions.window_height}`,
      "-p",
      simulatorOptions.port.toString(),
      "-v",
      simulatorOptions.vsync.toString(),
      "-s",
      setting_file,
      "--setting_dir",
      appConfigDirPath,
      "--lightweight",
      simulatorOptions.lightweight.toString(),
    ];
    command = simulatorOptions.unity
      ? Command.sidecar("simulator-unity", args)
      : Command.sidecar("simulator", args);
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
  };

  let handleCloseClick = async () => {
    if (child !== null) {
      await child.kill();
      child = null;
    }
  };

  onMount(async () => {
    await invoke("set_libpath", {});

    appConfigDirPath = await appConfigDir();
  });
</script>

<div class="ui">
  <label for="vsync">Vsync:</label>
  <CheckBox id="vsync" bind:checked={simulatorOptions.vsync} />

  <label for="port">Port:</label>
  <NumberInput
    id="port"
    bind:value={simulatorOptions.port}
    min="0"
    max="65535"
    step="1"
  />

  <label for="window_width">Window width:</label>
  <NumberInput
    id="window_width"
    bind:value={simulatorOptions.window_width}
    min="1"
    step="1"
  />

  <label for="window_height">Window height:</label>
  <NumberInput
    id="window_height"
    bind:value={simulatorOptions.window_height}
    min="1"
    step="1"
  />

  <label for="unity">Unity:</label>
  <CheckBox id="unity" bind:checked={simulatorOptions.unity} />

  <label for="lightweight">Lightweight mode:</label>
  <CheckBox id="lightweight" bind:checked={simulatorOptions.lightweight} />

  <Button label="Run" click={handleRunClick} disabled={!!child} />
  <Button label="Close" click={handleCloseClick} disabled={!child} />
</div>

<style>
  .ui {
    display: grid;
    grid-template-columns: auto 120px;
    grid-gap: 10px 0px;
    align-items: center;
  }
  label {
    text-align: right;
    padding-right: 10px;
  }
</style>
