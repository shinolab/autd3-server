import { writable } from 'svelte/store';

export const consoleOutputQueue = writable<string[]>([]);
