// Typing in the Source tab, as undo steps: what is typed joins one step
// until a pause or the end of a word, as a code editor groups it. Each
// change is sent as it is made; the grouping is the core's gestures.

export interface Gestures {
  begin(): void;
  end(): void;
}

/** How long a pause ends a step. */
export const PAUSE = 800;

export class Typist {
  private open = false;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(private readonly gestures: Gestures) {}

  /** Before a change of `inserted` is sent. */
  before() {
    if (!this.open) {
      this.gestures.begin();
      this.open = true;
    }
  }

  /** After it was sent: a space, a line or punctuation ends the word, and
      with it the step; otherwise a pause will. */
  after(inserted: string) {
    if (this.timer) clearTimeout(this.timer);
    if (/[\s;{}()=,]$/u.test(inserted) || inserted === '') {
      this.close();
      return;
    }
    this.timer = setTimeout(() => this.close(), PAUSE);
  }

  /** Ends the step now, as leaving the editor does. */
  close() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    if (this.open) {
      this.open = false;
      this.gestures.end();
    }
  }
}
