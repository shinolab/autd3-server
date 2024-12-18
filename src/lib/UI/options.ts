import type { Duration } from "./utils/duration.js";

export const SyncModeValues = ["DC", "FreeRun"] as const
export type SyncMode = typeof SyncModeValues[number]

export const TimerStrategyValues = ["SpinSleep", "StdSleep", "SpinWait"] as const
export type TimerStrategy = typeof TimerStrategyValues[number]

export interface TwinCATOptions {
    client: string;
    sync0: number;
    task: number;
    base: number;
    keep: boolean;
    lightweight: boolean;
    lightweight_port: number;
}

export interface SOEMOptions {
    ifname: string;
    port: number;
    sync0: Duration;
    send: Duration;
    buf_size: number;
    timer_strategy: TimerStrategy;
    state_check_interval: Duration;
    sync_tolerance: Duration;
    sync_timeout: Duration;
    lightweight: boolean;
}

export interface SimulatorOptions {
    vsync: boolean;
    port: number;
    window_width: number;
    window_height: number;
    unity: boolean;
    lightweight: boolean;
}

export interface Options {
    twincat: TwinCATOptions;
    soem: SOEMOptions;
    simulator: SimulatorOptions;
}
