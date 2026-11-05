// Svelte のテンプレートで <math-field> を使えるようにする型の宣言
import type { MathfieldElement } from 'mathlive'
import type { HTMLAttributes } from 'svelte/elements'

declare module 'svelte/elements' {
  interface SvelteHTMLElements {
    'math-field': HTMLAttributes<MathfieldElement> & { placeholder?: string }
  }
}
