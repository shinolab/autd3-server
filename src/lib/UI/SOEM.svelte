<script lang="ts">
  import type { SOEMOptions, TimerStrategy } from "./options.ts";
  import { TimerStrategyValues } from "./options.ts";

  import { onMount } from "svelte";
  import { Command, Child } from "@tauri-apps/plugin-shell";
  import { consoleOutputQueue } from "./console_output.ts";

  import Button from "./utils/Button.svelte";
  import Select from "./utils/Select.svelte";
  import CheckBox from "./utils/CheckBox.svelte";
  import NumberInput from "./utils/NumberInput.svelte";

  import {
    msToDuration,
    msFromDuration,
    usFromDuration,
    usToDuration,
    sToDuration,
    sFromDuration,
  } from "./utils/duration.ts";

  interface Props {
    soemOptions: SOEMOptions;
    adapters?: string[];
  }

  let { soemOptions = $bindable(), adapters = [] }: Props = $props();

  let command;
  let child: null | Child = $state(null);

  let parseStrategy = (strategy: TimerStrategy) => {
    switch (strategy) {
      case "SpinSleep":
        return "spin-sleep";
      case "StdSleep":
        return "std-sleep";
      case "SpinWait":
        return "spin-wait";
      default:
        return "spin-sleep";
    }
  };

  let stateCheckIntervalMs = $state(
    msFromDuration(soemOptions.state_check_interval),
  );
  $effect(() => {
    soemOptions.state_check_interval = msToDuration(stateCheckIntervalMs);
  });

  let sendUs = $state(usFromDuration(soemOptions.send));
  $effect(() => {
    soemOptions.send = usToDuration(sendUs);
  });
  let sync0Us = $state(usFromDuration(soemOptions.sync0));
  $effect(() => {
    soemOptions.sync0 = usToDuration(sync0Us);
  });

  let syncToleranceUs = $state(usFromDuration(soemOptions.sync_tolerance));
  $effect(() => {
    soemOptions.sync_tolerance = usToDuration(syncToleranceUs);
  });
  let syncTimeoutS = $state(sFromDuration(soemOptions.sync_timeout));
  $effect(() => {
    soemOptions.sync_timeout = sToDuration(syncTimeoutS);
  });

  let adapterNames: string[] = $state([]);
  let adapterName: string = $state("Auto");
  $effect(() => {
    if (adapterName == "Auto") {
      soemOptions.ifname = "Auto";
    } else {
      const idx = adapters.findIndex(
        (adapter) => adapter.split(",")[1].trim() == adapterName,
      );
      soemOptions.ifname = adapters[idx].split(",")[0].trim();
    }
  });

  let handleRunClick = async () => {
    const args: string[] = [
      "run",
      "-i",
      soemOptions.ifname == "Auto" ? "" : soemOptions.ifname,
      "-p",
      soemOptions.port.toString(),
      "-s",
      sync0Us.toString(),
      "-c",
      sendUs.toString(),
      "-b",
      soemOptions.buf_size.toString(),
      "-t",
      parseStrategy(soemOptions.timer_strategy),
      "-e",
      stateCheckIntervalMs.toString(),
      "--sync_tolerance",
      syncToleranceUs.toString(),
      "--sync_timeout",
      syncTimeoutS.toString(),
    ];
    if (soemOptions.lightweight) {
      args.push("-l");
    }

    command = Command.sidecar("SOEMAUTDServer", args);
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
    command.on("error", (err) => {
      alert(err);
      handleCloseClick();
    });
    command.on("close", async (data) => {
      if (data.code != null && data.code < -1) {
        alert(`SOEMAUTDServer exited with code ${data.code}`);
      }
      handleCloseClick();
    });
  };

  let handleCloseClick = async () => {
    if (child !== null) {
      await child.kill();
      child = null;
    }
  };

  onMount(async () => {
    adapterNames = ["Auto"].concat(
      adapters.map((adapter) => adapter.split(",")[1].trim()),
    );
  });
</script>

<div class="ui">
  <label for="ifname">Interface name:</label>
  <Select id="ifname" bind:value={adapterName} values={adapterNames} />

  <label for="port">Port:</label>
  <NumberInput
    id="port"
    bind:value={soemOptions.port}
    min="0"
    max="65535"
    step="1"
  />

  <label for="buf_size">Buffer size:</label>
  <NumberInput
    id="buf_size"
    bind:value={soemOptions.buf_size}
    min="1"
    step="1"
  />

  <label for="sync0Us">Sync0 cycle [us]:</label>
  <NumberInput id="sync0Us" bind:value={sync0Us} min="500" step="500" />

  <label for="sendUs">Send cycle [us]:</label>
  <NumberInput id="sendUs" bind:value={sendUs} min="500" step="500" />

  <label for="timer_strategy">Timer strategy:</label>
  <Select
    id="timer_strategy"
    bind:value={soemOptions.timer_strategy}
    values={TimerStrategyValues}
  />

  <label for="stateCheckIntervalMs">State check interval [ms]:</label>
  <NumberInput
    id="stateCheckIntervalMs"
    bind:value={stateCheckIntervalMs}
    min="1"
    step="1"
  />

  <label for="syncToleranceUs">Sync tolerance [us]:</label>
  <NumberInput
    id="syncToleranceUs"
    bind:value={syncToleranceUs}
    min="1"
    step="1"
  />

  <label for="syncTimeoutS">Sync timeout [s]:</label>
  <NumberInput id="syncTimeoutS" bind:value={syncTimeoutS} min="1" step="1" />

  <label for="lightweight">Lightweight mode:</label>
  <CheckBox id="lightweight" bind:checked={soemOptions.lightweight} />

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
