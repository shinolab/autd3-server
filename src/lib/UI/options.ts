import type { Duration } from "./utils/duration.js";

export const TimerStrategyValues = ["SpinSleep", "StdSleep", "SpinWait"] as const
export type TimerStrategy = typeof TimerStrategyValues[number]

export const CpuBaseTimeValues = ["1ms", "500us", "333us", "250us", "200us", "125us", "100us", "83.3us", "76.9us", "71.4us", "66.6us", "62.5us", "50us"] as const
export type CpuBaseTime = typeof CpuBaseTimeValues[number]

export interface TwinCATOptions {
    client: string;
    sync0: number;
    task: number;
    base: CpuBaseTime;
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
