<script lang="ts">
  import type { Options } from "./UI/options";

  // @ts-ignore
  import { Tabs, Tab, TabList, TabPanel } from "svelte-tabs";

  import { platform } from "@tauri-apps/plugin-os";
  import { invoke } from "@tauri-apps/api/core";
  import { Command } from "@tauri-apps/plugin-shell";

  import TwinCAT from "./UI/TwinCAT.svelte";
  import Simulator from "./UI/Simulator.svelte";

  interface Props {
    options: Options;
  }

  let { options }: Props = $props();

  const platformName = platform();

  async function checkAvailableTabs() {
    let twincatAvailable =
      platformName == "windows" && (await invoke("twincat_installed", {}));

    return {
      twincatAvailable,
    };
  }

  let promise = checkAvailableTabs();
</script>

<div>
  {#await promise then { twincatAvailable }}
    <Tabs>
      <TabList>
        {#if twincatAvailable}
          <Tab>TwinCAT</Tab>
        {/if}
        <Tab>Simulator</Tab>
      </TabList>

      {#if twincatAvailable}
        <TabPanel>
          <TwinCAT twincatOptions={options.twincat} />
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
