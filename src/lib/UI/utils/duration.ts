export interface Duration {
    secs: number;
    nanos: number;
}

export let msToDuration = (ms: number) => {
    let secs = Math.floor(ms / 1000);
    let nanos = (ms % 1000) * 1000000;
    return { secs: secs, nanos };
}

export let msFromDuration = (duration: Duration) => {
    return duration.secs * 1000 + duration.nanos / 1000000;
}
