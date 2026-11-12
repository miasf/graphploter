<script lang="ts">
  // 数式の入力欄（MathLive の <math-field>）。値は LaTeX でやりとりする。
  import type { MathfieldElement } from 'mathlive'
  import { onMount } from 'svelte'
  import { fenceBareDelimiters } from './latex'
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
    el.addEventListener('keydown', fixLayoutKey, { capture: true })
  })

  // ---- 日本語（JIS）配列などのキーボード ----
  // MathLive は、キーの操作（\ で LaTeX の命令の入力、" で文字の入力など）をキーの位置（code）から US 配列として決める。
  // JIS 配列では位置と文字の対応が違うので、] を打つと「\ のキー」として LaTeX の命令の入力に変わり（続けて打った文字が消える）、
  // * を打つと「" のキー」として文字の入力に変わってしまう。
  // そこで、打った文字が US 配列のその位置の文字と違うキーは、同じ文字を出す US 配列の位置のキーとして MathLive に渡し直す。

  /** US 配列の記号のキーの位置 → [そのまま, Shift] の文字 */
  const US_KEYS: Record<string, [string, string]> = {
    Backquote: ['`', '~'],
    Minus: ['-', '_'],
    Equal: ['=', '+'],
    BracketLeft: ['[', '{'],
    BracketRight: [']', '}'],
    Backslash: ['\\', '|'],
    Semicolon: [';', ':'],
    Quote: ["'", '"'],
    Comma: [',', '<'],
    Period: ['.', '>'],
    Slash: ['/', '?'],
    Digit1: ['1', '!'],
    Digit2: ['2', '@'],
    Digit3: ['3', '#'],
    Digit4: ['4', '$'],
    Digit5: ['5', '%'],
    Digit6: ['6', '^'],
    Digit7: ['7', '&'],
    Digit8: ['8', '*'],
    Digit9: ['9', '('],
    Digit0: ['0', ')'],
  }
  /** 文字 → US 配列でその文字を出すキーの位置と Shift */
  const US_CHARS = new Map<string, { code: string; shiftKey: boolean }>()
  for (const [code, [a, b]] of Object.entries(US_KEYS)) {
    US_CHARS.set(a, { code, shiftKey: false })
    US_CHARS.set(b, { code, shiftKey: true })
  }

  function fixLayoutKey(e: KeyboardEvent) {
    if (!e.isTrusted || e.ctrlKey || e.metaKey || e.altKey || e.isComposing || e.key.length !== 1) return
    const us = US_KEYS[e.code]
    if (!us || us[e.shiftKey ? 1 : 0] === e.key) return // US 配列と同じ（または記号のキーでない）
    // MathLive に届く前に止めて、同じ文字の US 配列のキーとして送り直す。
    // US 配列に無い文字（¥ など）は、位置を空にして送る（MathLive は文字だけで処理する）
    e.stopPropagation()
    e.preventDefault()
    const target = e.composedPath()[0] ?? el
    const as = US_CHARS.get(e.key) ?? { code: '', shiftKey: e.shiftKey }
    const ev = new KeyboardEvent('keydown', { key: e.key, ...as, bubbles: true, cancelable: true, composed: true })
    target.dispatchEvent(ev)
    // キーの操作として処理されなかった文字（ふつうの文字）は、打った文字として入れる
    if (!ev.defaultPrevented) el.executeCommand(['typedText', e.key])
  }

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
      // 裸の ⌊ ⌋（\lfloor … \rfloor）は \left … \right の形にして入れる。
      // そのままだと MathLive が1組の括弧として扱わず、⌊x⌋ の後ろで / を打つと z= まで分子に入る
      el.value = fenceBareDelimiters(value)
    }
  })

  // ---- 文字の後ろの数字を添字にする（w2 → w₂、t10 → t₁₀）----
  // 数字を入れる直前（beforeinput）に、カーソルの前を調べる。物理キーボードでも画面のキーボードでも同じように通る。
  // MathLive は入力欄の中身を「位置 k の直前にあるもの」の列として持っていて、w₁ は [w, 添字の先頭, 1, 添字の終わり] と並ぶ。

  /** 添字にしない文字（定数 e と虚数単位 i） */
  const NO_SUBSCRIPT = new Set(['e', 'i'])
  /** 名前の途中に数字がある関数（hyp2F1, hankelH1 など）。ここまで打ったら、次の数字は名前の続き */
  const NAMES_WITH_DIGIT = ['hyp', 'hyp0F', 'hyp1F', 'hyp2F', 'hankelH']
  /** こちらで数字を入れている最中か（その間の beforeinput は調べない） */
  let subscripting = false

  /** 位置 k の直前にあるものの LaTeX */
  const atomAt = (k: number) => (k >= 1 ? el.getValue(k - 1, k, 'latex') : '')

  /**
   * カーソルが「数字だけの添字が付いた文字」（w₁₂ など）の直後にあれば、添字の数字の数。そうでなければ 0。
   * このとき、添字の最後の数字の後ろは 位置 − 1
   */
  function digitSubscriptBefore(pos: number): number {
    if (pos < 4 || atomAt(pos) !== '') return 0
    let k = pos - 1
    while (k >= 1 && /^\d$/.test(atomAt(k))) k--
    const digits = pos - 1 - k
    if (digits === 0 || k < 2 || atomAt(k) !== '') return 0
    return /^[a-zA-Z]_\{?\d+\}?$/.test(el.getValue(k - 2, pos, 'latex')) ? digits : 0
  }

  function handleBeforeInput(e: InputEvent) {
    if (subscripting) return
    const [from, to] = el.selection.ranges[0] ?? [0, 0]
    if (el.selection.ranges.length !== 1 || from !== to) return // 選んだ範囲を置き換える・消す入力はそのまま
    const pos = el.position
    // 一文字消す（物理キーボードの Backspace も、数式キーボードの ⌫ もここを通る）
    if (e.inputType === 'deleteContentBackward') {
      e.preventDefault()
      subscripting = true
      try {
        deleteBackward(pos)
      } finally {
        subscripting = false
      }
      emit()
      return
    }
    const digit = e.data
    if (e.inputType !== 'insertText' || !digit || !/^\d$/.test(digit)) return
    const prev = atomAt(pos)
    /** 添字の中の、最後の数字の後ろ（ここに数字を足す）。新しく添字を作るなら null */
    let inside: number | null = null
    if (/^[a-zA-Z]$/.test(prev)) {
      // 文字の直後: その文字に添字を付ける
      if (NO_SUBSCRIPT.has(prev)) return
      let name = ''
      for (let k = pos; k >= 1 && name.length < 8; k--) {
        const c = atomAt(k)
        if (!/^[a-zA-Z0-9]$/.test(c)) break
        name = c + name
      }
      if (NAMES_WITH_DIGIT.some((n) => name.endsWith(n))) return
    } else if (digitSubscriptBefore(pos) > 0) {
      // 数字だけの添字が付いた文字の直後（w₁ の後ろ）: 添字に数字を足す（w₁₀）
      inside = pos - 1
    } else {
      return
    }
    e.preventDefault()
    subscripting = true
    try {
      if (inside === null) el.executeCommand('moveToSubscript')
      else el.position = inside
      el.insert(digit)
      el.executeCommand('moveAfterParent') // 添字から出る（続けて + などを打てるように）
    } finally {
      subscripting = false
    }
    emit()
  }

  /**
   * カーソル（位置 pos）の前を一文字消す。
   * - 添字付きの文字の直後なら、添字の最後の数字を消す（数字が無くなったら添字ごと消す）。
   *   そのままだと、1回目はカーソルが添字の中に入るだけで何も消えない
   * - 関数名（floor などの \operatorname）や括弧の中身の先頭では、MathLive の「一文字消す」は何もせず、
   *   カーソルも動かない（何回押しても消せない）。例: mod(□, b) の最初の引数を消した後。
   *   そのときは、その箱の手前に出てから、その前を消す
   */
  function deleteBackward(pos: number, depth = 0) {
    const digits = digitSubscriptBefore(pos)
    if (digits > 0) {
      el.position = pos - 1
      el.executeCommand('deleteBackward')
      if (digits > 1) el.executeCommand('moveAfterParent')
      else el.executeCommand('deleteBackward') // 空になった添字を消す
      return
    }
    const before = el.value
    el.executeCommand('deleteBackward')
    if (el.value !== before && el.position === pos && ghostOpens(el.value) > ghostOpens(before)) {
      unwrapFence(pos)
      return
    }
    if (el.value !== before || el.position !== pos || depth >= 8) return
    // 行き詰まった: 箱の手前に出る（式の先頭で、手前に何も無ければそのまま）
    el.executeCommand('moveToPreviousChar')
    const out = el.position
    if (out < pos) deleteBackward(out, depth + 1)
  }

  /** 見えない開き括弧（\left.）の数 */
  const ghostOpens = (latex: string) => latex.match(/\\left\./g)?.length ?? 0

  /** 関数名（\sin, \Gamma, \operatorname{floor} など）で終わっているか */
  const FUNCTION_TAIL =
    /(\\operatorname\{(\\mathrm\{[^{}]*\}|[^{}])*\}|\\(arc)?(sin|cos|tan|sec|csc|cot)h?|\\(exp|ln|log|Gamma|zeta|psi|max|min|arg|Re|Im))$/

  /**
   * 開き括弧を消した直後（MathLive は「(」を見えない括弧 \left. に変えて「)」を残す）。位置 pos はその括弧の中身の先頭。
   * 括弧の組ごと外して中身だけを残す。括弧の前が関数名なら、関数名も消す（Desmos と同じ: sin(x) → x）。
   * そのままだと、見えない括弧の前で何回押しても消せず、「)」だけが残る
   */
  function unwrapFence(pos: number) {
    el.executeCommand('moveToPreviousChar') // 括弧の手前へ
    const start = el.position
    const head = el.getValue(0, start, 'latex')
    // 括弧の終わり: 先頭からそこまでの中身に、見えない括弧が増える所
    let end = -1
    for (let k = pos; k <= el.lastOffset; k++) {
      if (ghostOpens(el.getValue(0, k, 'latex')) > ghostOpens(head)) {
        end = k
        break
      }
    }
    if (start >= pos || end < 0) {
      el.position = pos
      return
    }
    let inner = el.getValue(pos, end - 1, 'latex')
    // 中身が空欄（□）とカンマだけ（関数キーで入れて何も書いていない mod(□, □) など）なら、中身ごと消す
    if (inner.replace(/\\placeholder\{\}|,/g, '').trim() === '') inner = ''
    el.selection = { ranges: [[start, end]], direction: 'forward' }
    if (inner === '') el.executeCommand('deleteBackward')
    else el.insert(inner, { format: 'latex', selectionMode: 'after' })
    el.position = start
    if (FUNCTION_TAIL.test(head)) el.executeCommand('deleteBackward')
  }

  /** 今の中身を外へ知らせる（こちらで書き換えたときは、MathLive の input を待たずに知らせる） */
  function emit() {
    if (el.value === lastEmitted) return
    lastEmitted = el.value
    onInput(lastEmitted)
  }

  function handleInput(e: Event) {
    // エンター（改行）は式には入れず、新しい式を足す合図にする
    if ((e as InputEvent).inputType === 'insertLineBreak') {
      onEnter?.()
      return
    }
    if (subscripting) return // 書き換えの途中の値は知らせない（終わってから emit で知らせる）
    emit()
  }
</script>

<div class="wrap">
<math-field
  bind:this={el}
  class:small
  class:no-toggle={!keyboardToggle || small}
  aria-label={label}
  {placeholder}
  onbeforeinput={handleBeforeInput}
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
