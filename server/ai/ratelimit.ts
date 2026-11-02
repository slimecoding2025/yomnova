/** Fixed-memory sliding-window limiter, keyed by client address. There is no user authentication in YOMNOVA. */
export class RateLimiter {
  private hits = new Map<string, number[]>();
  constructor(
    private limit: number,
    private windowMs: number,
  ) {}

  take(key: string, now = Date.now()): { ok: boolean; retryAfterMs: number } {
    const cutoff = now - this.windowMs;
    const list = (this.hits.get(key) ?? []).filter((t) => t > cutoff);
    if (list.length >= this.limit) {
      this.hits.set(key, list);
      return { ok: false, retryAfterMs: list[0] + this.windowMs - now };
    }
    list.push(now);
    this.hits.set(key, list);
    if (this.hits.size > 5000) {
      for (const [k, v] of this.hits) if (v.every((t) => t <= cutoff)) this.hits.delete(k);
    }
    return { ok: true, retryAfterMs: 0 };
  }
}
