// A delayed fallback read must never overwrite a newer pushed snapshot.
export class LiveStatus<T> {
  private revision = 0;
  private apply: (value: T) => void;
  constructor(apply: (value: T) => void) {
    this.apply = apply;
  }
  push(value: T): void {
    this.revision++;
    this.apply(value);
  }
  async read(fetch: () => Promise<T>): Promise<void> {
    const revision = ++this.revision;
    const value = await fetch();
    if (revision === this.revision) this.apply(value);
  }
}
