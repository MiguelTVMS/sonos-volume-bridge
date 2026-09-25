// Only explicit user actions enter this queue. Device notifications use reads.
// Serialize commits so a slow earlier request cannot overwrite a later gesture.
export class UserWrites {
  private tail: Promise<unknown> = Promise.resolve();
  run<T>(write: () => Promise<T>): Promise<T> {
    const result = this.tail.then(write);
    this.tail = result.catch(() => undefined);
    return result;
  }
}
