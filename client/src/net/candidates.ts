const DEFAULT_PORT = 9247;

export interface CandidateEnv {
  native: boolean;
  devHost?: string;
  locationHost?: string;
}

/** Ordered, de-duplicated `host:port` strings to try. */
export function buildCandidates(env: CandidateEnv, saved: { hosts: string[]; port: number } | null): string[] {
  let list: string[];
  if (env.native) {
    const port = saved?.port ?? DEFAULT_PORT;
    list = [`127.0.0.1:${port}`, ...(saved?.hosts ?? []).map((h) => `${h}:${port}`)];
  } else if (env.devHost) {
    list = [env.devHost];
  } else {
    list = env.locationHost ? [env.locationHost] : [];
  }
  return [...new Set(list)];
}
