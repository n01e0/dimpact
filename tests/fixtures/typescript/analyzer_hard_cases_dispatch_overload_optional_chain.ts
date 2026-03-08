export type Sink<T> = {
  emit(v: T): void;
};

export function parse(v: string): number;
export function parse(v: number): number;
export function parse(v: string | number): number {
  return typeof v === "number" ? v : Number.parseInt(v, 10);
}

export class Pipe<T> {
  constructor(private sink?: Sink<T>) {}

  run(input: T, map: (v: T) => T): number {
    const mapped = map(input);
    const fn = this.sink?.emit;
    fn?.call(this.sink, mapped);
    this.sink?.emit(mapped);
    return parse(String(mapped));
  }
}
