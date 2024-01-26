<script lang="ts">
  import { consoleOutputQueue } from "./UI/console_output.ts";
  import { listen } from "@tauri-apps/api/event";
  import { afterUpdate, onMount } from "svelte";

  let element: HTMLTextAreaElement;

  afterUpdate(() => {
    element.scroll({ top: element.scrollHeight, behavior: "smooth" });
  });

  let console_output = "";
  $: {
    while ($consoleOutputQueue.length > 100) {
      $consoleOutputQueue.shift();
    }
    console_output = $consoleOutputQueue.join("\n");
  }

  onMount(async () => {
    await listen("console-emu", (event) => {
      consoleOutputQueue.update((v) => {
        return [...v, `${event.payload}`];
      });
    });
  });
</script>

<div>
  <textarea bind:this={element} readonly>{console_output}</textarea>
</div>

<style>
  div {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    flex: 1 0 0;
    align-self: stretch;

    width: 100%;
    box-sizing: border-box;
  }

  textarea {
    resize: none;

    font-size: small;

    display: flex;
    padding: 8px;
    align-items: flex-start;
    flex: 1 0 0;
    align-self: stretch;

    border-radius: 3px;
    border: 1px solid var(--color-border-interactive-muted, #2b659b);
    background: var(--color-background-base-default, #101923);
    color: var(--color-text-base-default, #ffffff);
  }
</style>
