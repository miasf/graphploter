<script lang="ts">
  // 数式の入力欄（MathLive の <math-field>）。値は LaTeX でやりとりする。
  import type { MathfieldElement } from 'mathlive'
  import { onMount } from 'svelte'
  import { fenceBareDelimiters } from './latex'
  import { colBlank, colsOf, formatMatrix, insertCol, insertRow, isBlank, MAX_SIZE, parseMatrix, removeCol, removeRow, resize, rowBlank, rowsOf, type Matrix } from './matrixEdit'
  import { keyboardOnBlur, keyboardOnFocus, keyboardPolicy, SCRIPT_SIZE_SHEET, SHORTCUTS } from './mathSetup'

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
    // 指数・下付き（積分の範囲など）を大きく描く
    const root = el.shadowRoot
    if (root && !root.adoptedStyleSheets.includes(SCRIPT_SIZE_SHEET)) root.adoptedStyleSheets = [...root.adoptedStyleSheets, SCRIPT_SIZE_SHEET]
    if (autofocus) startEditing()
    el.addEventListener('keydown', matrixKey, { capture: true })
    el.addEventListener('keydown', fixLayoutKey, { capture: true })
    el.addEventListener('selection-change', updateMatrix)
    window.mathVirtualKeyboard?.addEventListener('geometrychange', keyboardMoved)
    el.addEventListener('pointerdown', fieldDown, { capture: true })
    el.addEventListener('pointermove', fieldMove, { capture: true })
    el.addEventListener('pointerup', fieldUp, { capture: true })
    el.addEventListener('pointercancel', fieldCancel, { capture: true })
    el.addEventListener('wheel', fieldWheel, { passive: false })
  })

  // ---- 行列の大きさを、その場で変える（計画 M6）----
  // カーソルが行列の中にある間だけ、入力欄の下に小さな帯（行 ＋・行 −・列 ＋・列 −・大きさ）を出す。
  // キー（Enter・カンマ・Tab・Backspace）も、行列の中にある間だけ意味を変える。どれも、行列の LaTeX を成分の表に分けて
  // 書き換え（matrixEdit.ts）、行列だけを入れ替える（取り消しの 1 手になる）。成分の中身は変えない。
  //
  // MathLive は式を「位置 k の直前にあるもの」の列で持ち、行列は [成分の先頭, 成分の中身…, 成分の先頭, …, 行列そのもの] と並ぶ。
  // 深さ（getElementInfo の depth）は、成分の中が行列より 1 つ深い

  /** カーソルのある行列 */
  interface MatrixAt {
    /** 行列の直前の位置と、行列そのものの位置（この範囲を選ぶと行列 1 つ） */
    start: number
    end: number
    m: Matrix
    /** カーソルのある成分 */
    row: number
    col: number
    /** カーソルが成分のすぐ中にあるか（成分の中の括弧・分数の中ではない） */
    top: boolean
  }
  /** 帯に出すもの（行列の外なら null） */
  let matrix = $state<{ rows: number; cols: number; row: number; col: number } | null>(null)
  /** 大きさを選ぶマス目を開いているか、指しているマス */
  let picker = $state(false)
  let hover = $state<[number, number] | null>(null)
  /** 画面の数式キーボードの高さ（指で操作する画面で、帯をそのすぐ上に出す） */
  let keyboardHeight = $state(0)

  const depthAt = (k: number) => el.getElementInfo(k)?.depth ?? 0

  /** 範囲 (start, end) の中の、成分の先頭の位置（行の順）。`level` は成分の深さ */
  function cellStarts(start: number, end: number, level: number): number[] {
    const out: number[] = []
    for (let j = start + 1; j < end; j++) {
      // 成分の先頭は中身が空で、直前が同じ深さか浅い（分数など中身のあるものは、直前が深い）
      if (depthAt(j) === level && atomAt(j) === '' && depthAt(j - 1) <= level) out.push(j)
    }
    return out
  }

  /** カーソルを含む、いちばん内側の行列。無ければ null */
  function findMatrix(): MatrixAt | null {
    if (!el || el.lastOffset > 800) return null
    const pos = el.position
    let inner = pos
    let d = depthAt(pos)
    while (d > 0) {
      // inner を含むものの終わり（浅くなる最初の位置）と、その直前
      let end = inner + 1
      while (end <= el.lastOffset && depthAt(end) >= d) end++
      if (end > el.lastOffset) return null
      const outer = depthAt(end)
      let start = inner - 1
      while (start >= 0 && depthAt(start) > outer) start--
      if (start < 0) return null
      const m = parseMatrix(el.getValue(start, end, 'latex'))
      if (m) {
        const level = depthAt(start + 1)
        const starts = cellStarts(start, end, level)
        if (starts.length !== rowsOf(m) * colsOf(m)) return null
        const k = starts.filter((j) => j <= pos).length - 1
        if (k < 0) return null
        return { start, end, m, row: Math.floor(k / colsOf(m)), col: k % colsOf(m), top: depthAt(pos) === level }
      }
      inner = end
      d = outer
    }
    return null
  }

  function updateMatrix() {
    const at = focused ? findMatrix() : null
    matrix = at ? { rows: rowsOf(at.m), cols: colsOf(at.m), row: at.row, col: at.col } : null
    if (!matrix) picker = false
  }

  function keyboardMoved() {
    keyboardHeight = window.mathVirtualKeyboard?.boundingRect.height ?? 0
  }

  /** 行列を `next` に入れ替えて、カーソルを (row, col) の成分に置く（空欄ならその空欄を選ぶ。打つと置き換わる） */
  function replaceMatrix(at: MatrixAt, next: Matrix, row: number, col: number) {
    const level = depthAt(at.start + 1)
    subscripting = true
    try {
      el.selection = { ranges: [[at.start, at.end]], direction: 'forward' }
      el.insert(formatMatrix(next), { format: 'latex', selectionMode: 'after' })
      placeInCell(at.start, el.position, level, next, row, col)
    } finally {
      subscripting = false
    }
    emit()
    updateMatrix()
  }

  /** 範囲 (start, end) の行列の (row, col) の成分にカーソルを置く */
  function placeInCell(start: number, end: number, level: number, m: Matrix, row: number, col: number) {
    const r = Math.max(0, Math.min(rowsOf(m) - 1, row))
    const c = Math.max(0, Math.min(colsOf(m) - 1, col))
    const starts = cellStarts(start, end, level)
    const k = r * colsOf(m) + c
    if (k >= starts.length) return
    const last = k + 1 < starts.length ? starts[k + 1] - 1 : end - 1
    if (isBlank(m.cells[r][c]) && last > starts[k]) el.selection = { ranges: [[starts[k], last]], direction: 'forward' }
    else el.position = last
  }

  /** 行・列を足す・消す・大きさを決める（帯のボタン・キー・マス目が呼ぶ） */
  function editMatrix(op: 'row+' | 'row-' | 'col+' | 'col-' | 'row^' | 'size', rows = 0, cols = 0) {
    const at = findMatrix()
    if (!at) return false
    const { m, row, col } = at
    let next = m
    let [r, c] = [row, col]
    if (op === 'row+') [next, r] = [insertRow(m, row + 1), row + 1]
    else if (op === 'row^') next = insertRow(m, row)
    else if (op === 'col+') [next, c] = [insertCol(m, col + 1), col + 1]
    else if (op === 'row-') [next, r] = [removeRow(m, row), Math.max(0, row - 1)]
    else if (op === 'col-') [next, c] = [removeCol(m, col), Math.max(0, col - 1)]
    else next = resize(m, rows, cols)
    if (next === m) return false
    replaceMatrix(at, next, r, c)
    return true
  }

  /** 行列の中でのキー: Enter = 下に行、Shift + Enter = 上に行、カンマ = 右に列、Tab = 次の成分、空の行・列で Backspace = その行・列を消す */
  function matrixKey(e: KeyboardEvent) {
    if (!e.isTrusted || e.ctrlKey || e.metaKey || e.altKey || e.isComposing) return
    if (!['Enter', ',', 'Tab', 'Backspace'].includes(e.key)) return
    const at = findMatrix()
    if (!at) return
    const done = () => {
      e.stopImmediatePropagation()
      e.preventDefault()
    }
    const { m, row, col } = at
    if (e.key === 'Enter') {
      done()
      editMatrix(e.shiftKey ? 'row^' : 'row+')
    } else if (e.key === ',') {
      if (!at.top) return // 成分の中の括弧の中（max(a, b) など）では、ふつうのカンマ
      done()
      editMatrix('col+')
    } else if (e.key === 'Tab') {
      const k = row * colsOf(m) + col + (e.shiftKey ? -1 : 1)
      if (k < 0) return
      done()
      if (k >= rowsOf(m) * colsOf(m)) {
        // 最後の成分で Tab: 次の行を足して、その最初の成分へ
        const next = insertRow(m, rowsOf(m))
        if (next !== m) replaceMatrix(at, next, rowsOf(m), 0)
        return
      }
      placeInCell(at.start, at.end, depthAt(at.start + 1), m, Math.floor(k / colsOf(m)), k % colsOf(m))
      updateMatrix()
    } else if (blankLineOp(at)) {
      done()
      removeBlankLine(at)
    }
  }

  /** カーソルが空の行・空の列の中にあれば、消す操作（行を優先）。そうでなければ null */
  function blankLineOp(at: MatrixAt): 'row-' | 'col-' | null {
    const [from, to] = el.selection.ranges[0] ?? [0, 0]
    // 範囲を選んでいる（空欄を選んだ状態は除く）なら、ふつうに消す
    if (from !== to && !isBlank(at.m.cells[at.row][at.col])) return null
    if (rowBlank(at.m, at.row) && rowsOf(at.m) > 1) return 'row-'
    if (colBlank(at.m, at.col) && colsOf(at.m) > 1) return 'col-'
    return null
  }

  function removeBlankLine(at: MatrixAt) {
    const op = blankLineOp(at)
    if (op) editMatrix(op)
  }

  // ---- 長い式を横に送る（スクロール）----
  // 欄に収まらない式は、MathLive が中身（.ML__content）を横に送って、カーソルのある所を見せる。
  // 自分で送るには矢印キーを 1 回ずつ押すしかなかったので、指で横になぞる（PC では横スクロール・Shift + ホイール）と送れるようにする。
  // 送った位置は、ほかの式へ移っても（入力をやめても）そのまま残す。

  /** 横に送られる中身 */
  const content = () => el?.shadowRoot?.querySelector<HTMLElement>('.ML__content') ?? null
  /** 欄に収まっていないか */
  const overflowing = (c: HTMLElement) => c.scrollWidth > c.clientWidth + 1

  /** 入力中の欄を指でなぞっている間の状態 */
  let swipe: {
    id: number
    x: number
    y: number
    left: number
    /** 横に送り始めたか */
    panning: boolean
    /** 長押しして範囲を選んでいるなら、その始まりの位置 */
    anchor: number | null
    timer: ReturnType<typeof setTimeout> | undefined
  } | null = null

  /**
   * 入力中の欄に指を置いたとき（収まらない式だけ）。MathLive は指をなぞると範囲を選ぶので、こちらで受け取って、
   * なぞったら横に送り、動かさずに離したらその位置にカーソルを置く。範囲を選ぶのは、長押ししてからなぞる
   */
  function fieldDown(e: PointerEvent) {
    if (e.pointerType === 'mouse' || !focused || !e.isPrimary) return
    const c = content()
    if (!c || !overflowing(c) || !e.composedPath().includes(c)) return
    e.stopPropagation()
    e.preventDefault()
    const { clientX: x, clientY: y } = e
    swipe = { id: e.pointerId, x, y, left: c.scrollLeft, panning: false, anchor: null, timer: undefined }
    swipe.timer = setTimeout(() => {
      if (!swipe || swipe.panning) return
      const at = el.getOffsetFromPoint(x, y)
      if (at >= 0) {
        swipe.anchor = at
        el.position = at
      }
    }, 450)
    el.setPointerCapture(e.pointerId)
  }

  function fieldMove(e: PointerEvent) {
    if (!swipe || e.pointerId !== swipe.id) return
    e.stopPropagation()
    e.preventDefault()
    const c = content()
    if (!c) return
    if (swipe.anchor !== null) {
      // 長押しの後: 範囲を選ぶ
      const to = el.getOffsetFromPoint(e.clientX, e.clientY)
      if (to >= 0) el.selection = { ranges: [[Math.min(swipe.anchor, to), Math.max(swipe.anchor, to)]], direction: to < swipe.anchor ? 'backward' : 'forward' }
      return
    }
    const dx = e.clientX - swipe.x
    if (!swipe.panning && Math.hypot(dx, e.clientY - swipe.y) < 10) return
    swipe.panning = true
    clearTimeout(swipe.timer)
    c.scrollLeft = swipe.left - dx
  }

  function fieldUp(e: PointerEvent) {
    if (!swipe || e.pointerId !== swipe.id) return
    e.stopPropagation()
    e.preventDefault()
    clearTimeout(swipe.timer)
    const { panning, anchor } = swipe
    swipe = null
    // 動かさずに離した（タップ）: その位置にカーソルを置く
    if (!panning && anchor === null) {
      const at = el.getOffsetFromPoint(e.clientX, e.clientY)
      if (at >= 0) el.position = at
    }
  }

  function fieldCancel(e: PointerEvent) {
    if (!swipe || e.pointerId !== swipe.id) return
    clearTimeout(swipe.timer)
    swipe = null
  }

  /** PC: 横スクロール（トラックパッド）か、Shift + ホイールで横に送る */
  function fieldWheel(e: WheelEvent) {
    const c = content()
    if (!c || !overflowing(c)) return
    const dx = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.shiftKey ? e.deltaY : 0
    if (dx === 0) return
    e.preventDefault()
    c.scrollLeft += dx
  }

  /** 入力をやめたとき: 横に送った位置を残す（MathLive が戻してしまっても、元の位置に戻す） */
  function keepScroll() {
    const c = content()
    if (!c) return
    const left = c.scrollLeft
    const put = () => {
      if (!focused && c.scrollLeft !== left) c.scrollLeft = left
    }
    requestAnimationFrame(put)
    setTimeout(put, 100)
  }

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
    // 入力を始めると、MathLive は前のカーソルの位置まで式を横に送る。押した所にカーソルを置くので、
    // 横に送ってあった位置はそのままにする（送り直されたら戻す）
    const c = content()
    const left = c?.scrollLeft ?? 0
    el.focus()
    if (x !== undefined && y !== undefined) {
      if (c) c.scrollLeft = left
      const offset = el.getOffsetFromPoint(x, y)
      if (offset >= 0) el.position = offset
      const put = () => {
        if (c && c.scrollLeft !== left) c.scrollLeft = left
      }
      put()
      requestAnimationFrame(put)
    }
  }

  // ---- スマホ: 入力欄の上に重ねた透明な「タップ受け」----
  // MathLive の入力欄は、指が上に乗るとスクロールを止めてしまう（触れただけで入力が始まりキーボードが出る）。
  // そこで入力中でないときは透明な板を重ねておき、指を動かしたらふつうにスクロール、
  // 動かさずに離したら（タップ）入力を始める。
  let tapStart: { x: number; y: number; t: number; left: number; panning: boolean } | null = null
  let tapped = false

  function coverDown(e: PointerEvent) {
    tapStart = { x: e.clientX, y: e.clientY, t: e.timeStamp, left: content()?.scrollLeft ?? 0, panning: false }
    tapped = false
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }

  /** 入力中でない欄を横になぞったら、式を横に送る（縦はふつうのスクロール） */
  function coverMove(e: PointerEvent) {
    const c = content()
    if (!tapStart || !c || !overflowing(c)) return
    const dx = e.clientX - tapStart.x
    if (!tapStart.panning && Math.abs(dx) < 10) return
    tapStart.panning = true
    c.scrollLeft = tapStart.left - dx
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
    // 行列の空欄を選んだ状態（大きさを変えた直後）で ⌫: 空の行・列ならそれを消す
    if (e.inputType === 'deleteContentBackward' && from !== to) {
      const at = findMatrix()
      if (at && blankLineOp(at)) {
        e.preventDefault()
        removeBlankLine(at)
        return
      }
    }
    if (el.selection.ranges.length !== 1 || from !== to) return // 選んだ範囲を置き換える・消す入力はそのまま
    const pos = el.position
    // 一文字消す（物理キーボードの Backspace も、数式キーボードの ⌫ もここを通る）
    if (e.inputType === 'deleteContentBackward') {
      e.preventDefault()
      // 行列の空の行・列の中なら、その行・列を消す
      const at = findMatrix()
      if (at && blankLineOp(at)) {
        removeBlankLine(at)
        return
      }
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
      // 行列の中では、下に行を足す（数式キーボードの改行。物理キーボードは matrixKey）
      if (editMatrix('row+')) return
      if (findMatrix()) return
      onEnter?.()
      return
    }
    if (subscripting) return // 書き換えの途中の値は知らせない（終わってから emit で知らせる）
    emit()
    updateMatrix()
  }

  /** 帯のボタン: 押しても入力欄からフォーカスを外さない */
  const keepFocus = (e: Event) => e.preventDefault()
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
    keyboardMoved()
    updateMatrix()
    // 画面上のキーボードが出て一覧の見える部分が狭くなるので、出終わってから入力欄が見える所まで送る
    setTimeout(() => {
      if (focused) el.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
    }, 350)
    onFocusChange?.(true)
  }}
  onblur={() => {
    keepScroll()
    focused = false
    matrix = null
    picker = false
    keyboardOnBlur()
    onFocusChange?.(false)
  }}
></math-field>
  {#if matrix}
    <!-- 行列の中にカーソルがある間だけ出る帯（行・列を足す・消す、大きさを一度に決める） -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="matrix-bar"
      class:floating={touch}
      style:bottom={touch ? `${keyboardHeight}px` : undefined}
      onpointerdown={keepFocus}
      onmousedown={keepFocus}
    >
      <button type="button" title="カーソルの行の下に行を足す（Enter）" disabled={matrix.rows >= MAX_SIZE} onclick={() => editMatrix('row+')}>行 ＋</button>
      <button type="button" title="カーソルの行を消す" disabled={matrix.rows <= 1} onclick={() => editMatrix('row-')}>行 −</button>
      <button type="button" title="カーソルの列の右に列を足す（,）" disabled={matrix.cols >= MAX_SIZE} onclick={() => editMatrix('col+')}>列 ＋</button>
      <button type="button" title="カーソルの列を消す" disabled={matrix.cols <= 1} onclick={() => editMatrix('col-')}>列 −</button>
      <button type="button" class="size" title="大きさを選ぶ" aria-expanded={picker} onclick={() => (picker = !picker)}>
        {matrix.rows} × {matrix.cols}
      </button>
      {#if picker}
        <div class="size-grid" role="group" aria-label="行列の大きさ" onpointerleave={() => (hover = null)}>
          {#each Array(MAX_SIZE) as _, r}
            {#each Array(MAX_SIZE) as _, c}
              <button
                type="button"
                class="cell"
                class:on={hover ? r <= hover[0] && c <= hover[1] : r < matrix.rows && c < matrix.cols}
                aria-label="{r + 1} × {c + 1}"
                onpointerenter={() => (hover = [r, c])}
                onclick={() => {
                  picker = false
                  hover = null
                  editMatrix('size', r + 1, c + 1)
                }}
              ></button>
            {/each}
          {/each}
          <span class="size-label">{hover ? `${hover[0] + 1} × ${hover[1] + 1}` : `${matrix.rows} × ${matrix.cols}`}</span>
        </div>
      {/if}
    </div>
  {/if}
  {#if touch && !focused}
    <!-- 指で操作するとき専用の透明な板（キーボード操作では入力欄に直接フォーカスが来る） -->
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      class="tap-cover"
      aria-hidden="true"
      onpointerdown={coverDown}
      onpointermove={coverMove}
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
  .matrix-bar {
    position: relative;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    margin-top: 4px;
    font-size: 12px;
  }
  /* 指で操作する画面: 数式キーボードのすぐ上に出す（キーボードに隠れない） */
  .matrix-bar.floating {
    position: fixed;
    left: 0;
    right: 0;
    z-index: 200;
    margin: 0;
    padding: 6px 8px;
    background: var(--panel, var(--bg));
    border-top: 1px solid var(--border-strong);
  }
  .matrix-bar button {
    padding: 3px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .matrix-bar.floating button {
    padding: 7px 10px;
    font-size: 14px;
  }
  .matrix-bar button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .matrix-bar .size {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
  }
  .size-grid {
    position: absolute;
    right: 0;
    top: 100%;
    z-index: 30;
    display: grid;
    grid-template-columns: repeat(6, 22px);
    gap: 3px;
    margin-top: 4px;
    padding: 8px;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--bg);
    box-shadow: 0 4px 16px rgb(0 0 0 / 25%);
  }
  .matrix-bar.floating .size-grid {
    top: auto;
    bottom: 100%;
    right: 8px;
    grid-template-columns: repeat(6, 30px);
    margin: 0 0 4px;
  }
  .matrix-bar .size-grid .cell {
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 3px;
  }
  .matrix-bar.floating .size-grid .cell {
    width: 30px;
    height: 30px;
  }
  .matrix-bar .size-grid .cell.on {
    background: var(--accent, #4a90d9);
    border-color: var(--accent, #4a90d9);
  }
  .size-label {
    grid-column: 1 / -1;
    text-align: center;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
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
