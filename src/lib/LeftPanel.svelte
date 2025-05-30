<script lang="ts">
  import type { Options } from "./UI/options";

  // @ts-ignore
  import { Tabs, Tab, TabList, TabPanel } from "svelte-tabs";

  import { platform } from "@tauri-apps/plugin-os";
  import { invoke } from "@tauri-apps/api/core";
  import { Command } from "@tauri-apps/plugin-shell";

  import TwinCAT from "./UI/TwinCAT.svelte";
  import SOEM from "./UI/SOEM.svelte";
  import Simulator from "./UI/Simulator.svelte";

  interface Props {
    options: Options;
  }

  let { options }: Props = $props();

  const platformName = platform();

  let adapters: string[] = $state([]);

  async function checkAvailableTabs() {
    let twincatAvailable =
      platformName == "windows" && (await invoke("twincat_installed", {}));

    try {
      let ifnames: string = "";
      if (await invoke("wpcap_installed", {})) {
        ifnames = (await Command.sidecar("SOEMAUTDServer", ["list"]).execute())
          .stdout;
      } else {
        console.log("wpcap not installed, no adapters available.");
      }
      adapters = ifnames
        .split("\n")
        .slice(1)
        .filter((s) => s)
        .map((line) => line.trim().split("\t").join(","));
    } catch (err) {
      console.log(err);
      adapters = [];
    }
    let soemAvailable = adapters.length > 0;

    return {
      twincatAvailable,
      soemAvailable,
    };
  }

  let promise = checkAvailableTabs();
</script>

<div>
  {#await promise then { twincatAvailable, soemAvailable }}
    <Tabs>
      <TabList>
        {#if twincatAvailable}
          <Tab>TwinCAT</Tab>
        {/if}
        {#if soemAvailable}
          <Tab>SOEM (Remote)</Tab>
        {/if}
        <Tab>Simulator</Tab>
      </TabList>

      {#if twincatAvailable}
        <TabPanel>
          <TwinCAT twincatOptions={options.twincat} />
        </TabPanel>
      {/if}
      {#if soemAvailable}
        <TabPanel>
          <SOEM {adapters} soemOptions={options.soem} />
        </TabPanel>
      {/if}
      <TabPanel>
        <Simulator simulatorOptions={options.simulator} />
      </TabPanel>
    </Tabs>
  {/await}
</div>

<style>
  div {
    display: flex;
    width: 440px;
    flex-direction: column;
    align-items: flex-start;
    flex-shrink: 0;
    align-self: stretch;
  }

  :global(.svelte-tabs) {
    width: 100%;
  }

  :global(.svelte-tabs ul.svelte-tabs__tab-list) {
    border-bottom: none;
  }

  :global(.svelte-tabs li.svelte-tabs__tab) {
    box-sizing: border-box;
    color: var(--color-text-base-default, #ffffff);
  }

  :global(.svelte-tabs li.svelte-tabs__selected) {
    color: #4dacff;
  }

  :global(.svelte-tabs div.svelte-tabs__tab-panel) {
    color: var(--color-text-base-default, #ffffff);
  }
</style>
