<script lang="ts">
  // 数式の入力欄（MathLive の <math-field>）。値は LaTeX でやりとりする。
  import type { MathfieldElement } from 'mathlive'
  import { onMount } from 'svelte'
  import { SHORTCUTS } from './mathSetup'

  interface Props {
    /** 入力欄の中身（LaTeX） */
    value: string
    onInput: (latex: string) => void
    /** 読み上げ用の説明 */
    label: string
    placeholder?: string
    small?: boolean
    /** 画面上のキーボードを出すボタンを表示するか */
    keyboardToggle?: boolean
    onFocusChange?: (focused: boolean) => void
  }
  let {
    value,
    onInput,
    label,
    placeholder = '',
    small = false,
    keyboardToggle = true,
    onFocusChange,
  }: Props = $props()

  let el: MathfieldElement
  /** 最後にこの欄から外へ知らせた値。これと違う値が来たときだけ「外から変わった」とみなす */
  let lastEmitted: string | null = null

  onMount(() => {
    el.inlineShortcuts = SHORTCUTS
    el.smartFence = true // ( を打つと ) も自動で入る
    el.mathVirtualKeyboardPolicy = 'auto' // スマホでは画面上の数式キーボードを出す
  })

  // 外から値が変わったときだけ書き換える。
  // 速く入力していると、知らせた値が戻ってくる前に次の文字が入っていることがあるので、
  // 「今の欄の中身」ではなく「最後に知らせた値」と比べる（比べ方を間違えると入力した文字が消える）
  $effect(() => {
    if (el && value !== lastEmitted) {
      lastEmitted = value
      el.value = value
    }
  })

  function handleInput() {
    lastEmitted = el.value
    onInput(lastEmitted)
  }
</script>

<math-field
  bind:this={el}
  class:small
  class:no-toggle={!keyboardToggle || small}
  aria-label={label}
  {placeholder}
  oninput={handleInput}
  onfocus={() => onFocusChange?.(true)}
  onblur={() => onFocusChange?.(false)}
></math-field>

<style>
  math-field {
    display: block;
    width: 100%;
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    color: var(--text);
    font-size: 19px;
    --caret-color: var(--text);
    --selection-background-color: var(--hover);
    --contains-highlight-background-color: transparent;
    --placeholder-color: var(--muted);
  }
  math-field.small {
    font-size: 15px;
    padding: 2px 6px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
  }
  /* 右クリックメニューのボタンは使わないので隠す */
  math-field::part(menu-toggle) {
    display: none;
  }
  math-field.no-toggle::part(virtual-keyboard-toggle) {
    display: none;
  }
</style>
