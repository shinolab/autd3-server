import type { Duration } from "./utils/duration.js";

export const SleeperValues = ["Spin", "Std", "SpinWait"] as const
export type Sleeper = typeof SleeperValues[number]

export const CpuBaseTimeValues = ["1ms", "500us", "333us", "250us", "200us", "125us", "100us", "83.3us", "76.9us", "71.4us", "66.6us", "62.5us", "50us"] as const
export type CpuBaseTime = typeof CpuBaseTimeValues[number]

export const TwinCATVersionValues = ["4024", "4026"] as const
export type TwinCATVersion = typeof TwinCATVersionValues[number]

export interface TwinCATOptions {
    client: string;
    device_name: string;
    version: TwinCATVersion;
    sync0: number;
    task: number;
    base: CpuBaseTime;
    keep: boolean;
    debug: boolean;
}

export interface SOEMOptions {
    ifname: string;
    port: number;
    sync0: Duration;
    send: Duration;
    buf_size: number;
    sleeper: Sleeper;
    state_check_interval: Duration;
    sync_tolerance: Duration;
    sync_timeout: Duration;
}

export interface SimulatorOptions {
    vsync: boolean;
    port: number;
    window_width: number;
    window_height: number;
    unity: boolean;
}

export interface Options {
    twincat: TwinCATOptions;
    soem: SOEMOptions;
    simulator: SimulatorOptions;
}
