<script lang="ts">
  import type { SOEMOptions, SyncMode, TimerStrategy } from "./options.ts";
  import { SyncModeValues, TimerStrategyValues } from "./options.ts";

  import { onMount } from "svelte";
  import { Command, Child } from "@tauri-apps/api/shell";
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

  export let soemOptions: SOEMOptions;
  export let adapters: string[] = [];

  let command;
  let child: null | Child = null;

  let parseStrategy = (strategy: TimerStrategy) => {
    switch (strategy) {
      case "Sleep":
        return "sleep";
      case "BusyWait":
        return "busy-wait";
      default:
        return "sleep";
    }
  };

  let stateCheckIntervalMs = msFromDuration(soemOptions.state_check_interval);
  $: {
    soemOptions.state_check_interval = msToDuration(stateCheckIntervalMs);
  }
  let timeoutMs = msFromDuration(soemOptions.timeout);
  $: {
    soemOptions.timeout = msToDuration(timeoutMs);
  }
  let syncToleranceUs = usFromDuration(soemOptions.sync_tolerance);
  $: {
    soemOptions.sync_tolerance = usToDuration(syncToleranceUs);
  }
  let syncTimeoutS = sFromDuration(soemOptions.sync_timeout);
  $: {
    soemOptions.sync_timeout = sToDuration(syncTimeoutS);
  }

  let adapterNames: string[] = [];
  let adapterName: string = "Auto";
  $: {
    if (adapterName == "Auto") {
      soemOptions.ifname = "Auto";
    } else {
      const idx = adapters.findIndex(
        (adapter) => adapter.split(",")[1].trim() == adapterName,
      );
      soemOptions.ifname = adapters[idx].split(",")[0].trim();
    }
  }

  let handleRunClick = async () => {
    const args: string[] = [
      "run",
      "-i",
      soemOptions.ifname == "Auto" ? "" : soemOptions.ifname,
      "-p",
      soemOptions.port.toString(),
      "-s",
      soemOptions.sync0.toString(),
      "-c",
      soemOptions.send.toString(),
      "-b",
      soemOptions.buf_size.toString(),
      "-w",
      parseStrategy(soemOptions.timer_strategy),
      "-e",
      stateCheckIntervalMs.toString(),
      "-t",
      timeoutMs.toString(),
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
      if (data.code < -1) {
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

  <label for="sync0">Sync0 cycle:</label>
  <NumberInput id="sync0" bind:value={soemOptions.sync0} min="1" step="1" />

  <label for="send">Send cycle:</label>
  <NumberInput id="send" bind:value={soemOptions.send} min="1" step="1" />

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

  <label for="timeoutMs">Timeout [ms]:</label>
  <NumberInput id="timeoutMs" bind:value={timeoutMs} min="1" step="1" />

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
