<script lang="ts">
  // 数式の入力欄（MathLive の <math-field>）。値は LaTeX でやりとりする。
  import type { MathfieldElement } from 'mathlive'
  import { onMount } from 'svelte'
  import { keyboardOnBlur, keyboardOnFocus, keyboardPolicy, SHORTCUTS } from './mathSetup'

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
    /** エンター（改行）が押されたとき（その下に新しい式を足すのに使う） */
    onEnter?: () => void
    /** 表示されたらすぐ入力を始める（新しく足した式など） */
    autofocus?: boolean
  }
  let {
    value,
    onInput,
    label,
    placeholder = '',
    small = false,
    keyboardToggle = true,
    onFocusChange,
    onEnter,
    autofocus = false,
  }: Props = $props()

  // スマホなど指で操作する画面か
  const touch = window.matchMedia('(pointer: coarse)').matches
  let focused = $state(false)

  let el: MathfieldElement
  /** 最後にこの欄から外へ知らせた値。これと違う値が来たときだけ「外から変わった」とみなす */
  let lastEmitted: string | null = null

  onMount(() => {
    el.inlineShortcuts = SHORTCUTS
    el.smartFence = true // ( を打つと ) も自動で入る
    el.mathVirtualKeyboardPolicy = keyboardPolicy() // 画面上の数式キーボードを出す（PC では入力欄に入ったときに出す）
    if (autofocus) startEditing()
  })

  /** 入力を始める（スマホでは数式キーボードが出る）。x, y があれば、その位置にカーソルを置く */
  function startEditing(x?: number, y?: number) {
    el.focus()
    if (x !== undefined && y !== undefined) {
      const offset = el.getOffsetFromPoint(x, y)
      if (offset >= 0) el.position = offset
    }
  }

  // ---- スマホ: 入力欄の上に重ねた透明な「タップ受け」----
  // MathLive の入力欄は、指が上に乗るとスクロールを止めてしまう（触れただけで入力が始まりキーボードが出る）。
  // そこで入力中でないときは透明な板を重ねておき、指を動かしたらふつうにスクロール、
  // 動かさずに離したら（タップ）入力を始める。
  let tapStart: { x: number; y: number; t: number } | null = null
  let tapped = false

  function coverDown(e: PointerEvent) {
    tapStart = { x: e.clientX, y: e.clientY, t: e.timeStamp }
    tapped = false
  }

  function coverUp(e: PointerEvent) {
    if (!tapStart) return
    const moved = Math.hypot(e.clientX - tapStart.x, e.clientY - tapStart.y)
    const long = e.timeStamp - tapStart.t > 600 // 長押しは入力を始めない
    tapStart = null
    tapped = moved < 10 && !long
  }

  // 入力は click のときに始める（pointerup の時点で始めると、その後のブラウザの処理でフォーカスが外れる）
  function coverClick(e: MouseEvent) {
    if (tapped) startEditing(e.clientX, e.clientY)
    tapped = false
  }

  // 外から値が変わったときだけ書き換える。
  // 速く入力していると、知らせた値が戻ってくる前に次の文字が入っていることがあるので、
  // 「今の欄の中身」ではなく「最後に知らせた値」と比べる（比べ方を間違えると入力した文字が消える）
  $effect(() => {
    if (el && value !== lastEmitted) {
      lastEmitted = value
      el.value = value
    }
  })

  function handleInput(e: Event) {
    // エンター（改行）は式には入れず、新しい式を足す合図にする
    if ((e as InputEvent).inputType === 'insertLineBreak') {
      onEnter?.()
      return
    }
    lastEmitted = el.value
    onInput(lastEmitted)
  }
</script>

<div class="wrap">
<math-field
  bind:this={el}
  class:small
  class:no-toggle={!keyboardToggle || small}
  aria-label={label}
  {placeholder}
  oninput={handleInput}
  onfocus={() => {
    focused = true
    keyboardOnFocus()
    // 画面上のキーボードが出て一覧の見える部分が狭くなるので、出終わってから入力欄が見える所まで送る
    setTimeout(() => {
      if (focused) el.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
    }, 350)
    onFocusChange?.(true)
  }}
  onblur={() => {
    focused = false
    keyboardOnBlur()
    onFocusChange?.(false)
  }}
></math-field>
  {#if touch && !focused}
    <!-- 指で操作するとき専用の透明な板（キーボード操作では入力欄に直接フォーカスが来る） -->
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      class="tap-cover"
      aria-hidden="true"
      onpointerdown={coverDown}
      onpointerup={coverUp}
      onpointercancel={() => (tapStart = null)}
      onclick={coverClick}
    ></div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
    min-width: 0;
  }
  .tap-cover {
    position: absolute;
    inset: 0;
    touch-action: pan-y; /* 縦のスクロールはそのまま通す */
    -webkit-touch-callout: none; /* 長押しのメニューを出さない */
    user-select: none;
  }
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
