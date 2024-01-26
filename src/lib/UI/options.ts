import type { Duration } from "./utils/duration.js";

export const SyncModeValues = ["DC", "FreeRun"] as const
export type SyncMode = typeof SyncModeValues[number]

export const TimerStrategyValues = ["NativeTimer", "Sleep", "BusyWait"] as const
export type TimerStrategy = typeof TimerStrategyValues[number]

export interface TwinCATOptions {
    client: string;
    sync0: number;
    task: number;
    base: number;
    mode: SyncMode;
    keep: boolean;
}

export interface SOEMOptions {
    ifname: string;
    port: number;
    sync0: number;
    send: number;
    buf_size: number;
    mode: SyncMode;
    timer_strategy: TimerStrategy;
    state_check_interval: Duration;
    timeout: Duration;
    debug: boolean;
}

export interface SimulatorOptions {
    vsync: boolean;
    port: number;
    gpu_idx: number;
    window_width: number;
    window_height: number;
    unity: boolean;
}

export interface Options {
    twincat: TwinCATOptions;
    soem: SOEMOptions;
    simulator: SimulatorOptions;
}
