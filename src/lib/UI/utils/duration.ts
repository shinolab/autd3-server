export interface Duration {
    secs: number;
    nanos: number;
}

export let msToDuration = (ms: number) => {
    let secs = Math.floor(ms / 1000);
    let nanos = (ms % 1000) * 1000000;
    return { secs: secs, nanos };
}

export let usToDuration = (us: number) => {
    let secs = Math.floor(us / 1000000);
    let nanos = (us % 1000000) * 1000;
    return { secs: secs, nanos };
}

export let sToDuration = (s: number) => {
    return { secs: s, nanos: 0 };
}

export let msFromDuration = (duration: Duration) => {
    return duration.secs * 1000 + duration.nanos / 1000000;
}

export let usFromDuration = (duration: Duration) => {
    return duration.secs * 1000000 + duration.nanos / 1000;
}

export let sFromDuration = (duration: Duration) => {
    return duration.secs;
}
