<script lang="ts">
  import type { SimulatorOptions } from "./options.ts";

  import { resolveResource } from "@tauri-apps/api/path";
  import { onMount } from "svelte";
  import { writable } from "svelte/store";
  import { platform } from "@tauri-apps/api/os";
  import { Command, Child } from "@tauri-apps/api/shell";
  import { invoke } from "@tauri-apps/api";
  import { consoleOutputQueue } from "./console_output.ts";
  import { appConfigDir } from "@tauri-apps/api/path";

  import Button from "./utils/Button.svelte";
  import Select from "./utils/Select.svelte";
  import CheckBox from "./utils/CheckBox.svelte";
  import NumberInput from "./utils/NumberInput.svelte";

  export let simulatorOptions: SimulatorOptions;

  let appConfigDirPath: string;

  let command;
  let child: null | Child = null;

  let gpuName: string;
  $: {
    if (gpuName) {
      const idx = availableGpusNames.indexOf(gpuName);
      if (idx == 0 || idx == -1) {
        simulatorOptions.gpu_idx = -1;
      } else {
        let gpu_idx = availableGpus[idx - 1].split(":")[0].trim();
        simulatorOptions.gpu_idx = parseInt(gpu_idx);
      }
    }
  }

  const cachedGPUs = writable<string | null>(null);
  let availableGpus: string[] = [];
  $: availableGpusNames = ["Auto"].concat(
    availableGpus.map((gpu) => gpu.split(":")[1].trim()),
  );

  let handleRunClick = async () => {
    const resourcePath = await resolveResource("");
    const args: string[] = [
      "run",
      "-w",
      `${simulatorOptions.window_width},${simulatorOptions.window_height}`,
      "-p",
      simulatorOptions.port.toString(),
      "-v",
      simulatorOptions.vsync ? "true" : "false",
      "-s",
      "simulator_settings.json",
      "--config_path",
      appConfigDirPath,
      "--resource_path",
      resourcePath,
    ];
    if (simulatorOptions.gpu_idx !== -1) {
      args.push("-g");
      args.push(simulatorOptions.gpu_idx.toString());
    }
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

    let gpus: null | string = null;
    cachedGPUs.subscribe((v) => {
      gpus = v;
    })();
    if (gpus == null) {
      let { stdout, stderr } = await Command.sidecar(
        "simulator",
        "list",
      ).execute();
      if (stderr) {
        consoleOutputQueue.update((v) => {
          return [...v, stderr.trimEnd()];
        });
        if ((await platform()) == "darwin") {
          consoleOutputQueue.update((v) => {
            return [
              ...v,
              "If you are using macOS, please install VulkanSDK and try again.",
            ];
          });
        }
        gpus = "";
      } else {
        gpus = stdout;
      }
      cachedGPUs.set(gpus);
    }
    if (gpus) {
      availableGpus = gpus
        .trimEnd()
        .split("\n")
        .map((s) => s.trim().replace(/ \(type .*\)$/g, ""));
    } else {
      availableGpus = [];
    }
    gpuName = (
      availableGpus.find(
        (gpu) =>
          parseInt(gpu.split(":")[0].trim()) === simulatorOptions.gpu_idx,
      ) ?? "0:Auto"
    )
      .split(":")[1]
      .trim();
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

  <label for="gpuName">GPU: </label>
  <Select id="gpuName" bind:value={gpuName} values={availableGpusNames} />

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
