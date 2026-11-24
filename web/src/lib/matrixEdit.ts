// 行列の大きさを、入れ直さずに変える（計画 docs/plan-math-notation.md の M6）。
// LaTeX の `\begin{pmatrix} … \end{pmatrix}` を成分の表に分け、行・列を足す・消す。成分の中身は書き換えない。
// 増えた成分は空欄（\placeholder{}）で、空欄は 0 として読む（latex.ts）

/** 行列の大きさの上限（計算エンジンと同じ 6 × 6） */
export const MAX_SIZE = 6
/** 空欄 */
export const BLANK = '\\placeholder{}'

/** 行列 1 つ。`cells` は行ごとの成分の LaTeX、`env` は括弧の形（pmatrix・bmatrix・vmatrix・pmatrix*・smallmatrix など） */
export interface Matrix {
  env: string
  cells: string[][]
  /** pmatrix* などの列のそろえ方（`[r]`。書いてあったものをそのまま残す） */
  align?: string
}

/** 行列の環境の名前か（計算のほう latex.ts が読めるものと同じ） */
export const isMatrixEnv = (env: string) => /^(p|b|B|v|V|small)?matrix\*?$/.test(env)

/** 行の間の空き \\[2pt] の [2pt] */
const ROW_GAP = /^\[\s*-?[\d.]+\s*(pt|em|ex|mm|cm|in|mu|bp|pc|dd|cc|sp)\s*\]/

/** 成分が空欄か（何も無い・空欄の印だけ） */
export const isBlank = (cell: string) => cell.replace(/\\placeholder\s*\{\s*\}/g, '').trim() === ''

/** LaTeX の位置 i から始まる命令の終わり（`\name` は英字の並び、それ以外は 2 文字） */
function commandEnd(s: string, i: number): number {
  let j = i + 1
  if (j < s.length && /[a-zA-Z]/.test(s[j])) {
    while (j < s.length && /[a-zA-Z]/.test(s[j])) j++
    return j
  }
  return Math.min(s.length, j + 1)
}

/** `\begin{env}` か `\end{env}` の env と、その終わりの位置。そうでなければ null */
function envAt(s: string, i: number, word: 'begin' | 'end'): { env: string; end: number } | null {
  const m = new RegExp(`^\\\\${word}\\s*\\{([a-zA-Z*]+)\\}`).exec(s.slice(i))
  return m ? { env: m[1], end: i + m[0].length } : null
}

/**
 * `latex` 全体が行列 1 つ（`\begin{…matrix}` で始まり、対応する `\end` で終わる）なら、成分の表にする。
 * 成分の中の `{}`・`\left … \right`・入れ子の `\begin … \end` の中の `&` と `\\` は区切りにしない
 */
export function parseMatrix(latex: string): Matrix | null {
  const s = latex.trim()
  const open = envAt(s, 0, 'begin')
  if (!open || !isMatrixEnv(open.env)) return null
  const cells: string[][] = [[]]
  let cur = ''
  let depth = 0
  let i = open.end
  const align = open.env.endsWith('*') ? /^\[[lcr]\]/.exec(s.slice(i))?.[0] : undefined
  if (align) i += align.length
  const push = () => {
    cells[cells.length - 1].push(cur.trim())
    cur = ''
  }
  while (i < s.length) {
    const c = s[i]
    if (c === '\\') {
      const end = commandEnd(s, i)
      const cmd = s.slice(i, end)
      if (cmd === '\\end' && depth === 0) {
        const close = envAt(s, i, 'end')
        if (!close || close.env !== open.env || close.end !== s.length) return null
        push()
        // 最後の行の後の \\（\\ \end{pmatrix}）で、空の行を足さない
        const last = cells[cells.length - 1]
        if (cells.length > 1 && last.length === 1 && last[0] === '') cells.pop()
        // 行ごとの成分の数をそろえる（足りない所は空欄）
        const cols = Math.max(...cells.map((r) => r.length))
        for (const r of cells) while (r.length < cols) r.push('')
        return align ? { env: open.env, cells, align } : { env: open.env, cells }
      }
      if (cmd === '\\\\' && depth === 0) {
        push()
        cells.push([])
        i = end
        // 行の間の空き \\[2pt] は成分に入れない
        const gap = ROW_GAP.exec(s.slice(i))
        if (gap) i += gap[0].length
        continue
      }
      if (cmd === '\\begin' || cmd === '\\left') depth++
      if (cmd === '\\end' || cmd === '\\right') depth--
      cur += cmd
      i = end
      continue
    }
    if (c === '{') depth++
    if (c === '}') depth--
    if (c === '&' && depth === 0) {
      push()
      i++
      continue
    }
    cur += c
    i++
  }
  return null
}

/** 成分の表を LaTeX にする（空の成分は空欄の印にする） */
export function formatMatrix(m: Matrix): string {
  const body = m.cells.map((r) => r.map((c) => (isBlank(c) ? BLANK : c)).join(' & ')).join('\\\\ ')
  return `\\begin{${m.env}}${m.align ?? ''}${body}\\end{${m.env}}`
}

export const rowsOf = (m: Matrix) => m.cells.length
export const colsOf = (m: Matrix) => m.cells[0]?.length ?? 0

/** 大きさを rows × cols にする（1〜6 に丸める）。残る成分はそのまま、増えた成分は空欄、余った行・列は捨てる */
export function resize(m: Matrix, rows: number, cols: number): Matrix {
  const clamp = (n: number) => Math.max(1, Math.min(MAX_SIZE, Math.round(n)))
  const [r, c] = [clamp(rows), clamp(cols)]
  return { ...m, cells: Array.from({ length: r }, (_, i) => Array.from({ length: c }, (_, j) => m.cells[i]?.[j] ?? '')) }
}

/** `at` 行目の位置に、空の行を足す（上限を超えるなら、そのまま） */
export function insertRow(m: Matrix, at: number): Matrix {
  if (rowsOf(m) >= MAX_SIZE) return m
  const cells = m.cells.map((r) => [...r])
  cells.splice(Math.max(0, Math.min(at, cells.length)), 0, Array.from({ length: colsOf(m) }, () => ''))
  return { ...m, cells }
}

/** `at` 列目の位置に、空の列を足す（上限を超えるなら、そのまま） */
export function insertCol(m: Matrix, at: number): Matrix {
  if (colsOf(m) >= MAX_SIZE) return m
  const k = Math.max(0, Math.min(at, colsOf(m)))
  return { ...m, cells: m.cells.map((r) => [...r.slice(0, k), '', ...r.slice(k)]) }
}

/** 行を消す（最後の 1 行は消せない） */
export function removeRow(m: Matrix, row: number): Matrix {
  if (rowsOf(m) <= 1 || row < 0 || row >= rowsOf(m)) return m
  return { ...m, cells: m.cells.filter((_, i) => i !== row).map((r) => [...r]) }
}

/** 列を消す（最後の 1 列は消せない） */
export function removeCol(m: Matrix, col: number): Matrix {
  if (colsOf(m) <= 1 || col < 0 || col >= colsOf(m)) return m
  return { ...m, cells: m.cells.map((r) => r.filter((_, j) => j !== col)) }
}

/** 行・列が全部空欄か */
export const rowBlank = (m: Matrix, row: number) => m.cells[row]?.every(isBlank) ?? false
export const colBlank = (m: Matrix, col: number) => m.cells.every((r) => isBlank(r[col] ?? ''))

/**
 * LaTeX の位置 `at` を含む、いちばん内側の行列の範囲 [start, end)（`\begin` の頭から `\end{…}` の後ろまで）。無ければ null
 */
export function matrixRangeAt(latex: string, at: number): [number, number] | null {
  const stack: { env: string; start: number }[] = []
  let best: [number, number] | null = null
  let i = 0
  while (i < latex.length) {
    if (latex[i] !== '\\') {
      i++
      continue
    }
    const b = envAt(latex, i, 'begin')
    if (b) {
      stack.push({ env: b.env, start: i })
      i = b.end
      continue
    }
    const e = envAt(latex, i, 'end')
    if (e) {
      const open = stack.pop()
      if (open && isMatrixEnv(open.env) && open.start <= at && at <= e.end) {
        // 内側の行列が先に閉じるので、最初に見つかったものがいちばん内側
        best ??= [open.start, e.end]
      }
      i = e.end
      continue
    }
    i = commandEnd(latex, i)
  }
  return best
}

/**
 * 式 `latex` の、位置 `at` にある（いちばん内側の）行列の大きさを rows × cols にした式を返す。
 * 行列が無ければ、そのまま返す
 */
export function resizeMatrix(latex: string, at: number, rows: number, cols: number): string {
  const range = matrixRangeAt(latex, at)
  if (!range) return latex
  const m = parseMatrix(latex.slice(range[0], range[1]))
  if (!m) return latex
  return latex.slice(0, range[0]) + formatMatrix(resize(m, rows, cols)) + latex.slice(range[1])
}

/** 新しい行列（全部空欄）の LaTeX */
export function blankMatrix(rows: number, cols: number, env = 'pmatrix'): string {
  return formatMatrix(resize({ env, cells: [[]] }, rows, cols))
}

/**
 * 式が、行列の定義の行 `A = (2×2 か 3×3 の行列)` なら、名前（LaTeX。`A`・`\vec{b}` など）と大きさ。そうでなければ null
 * （「座標系として使う」を出すかどうかを決める）
 */
export function matrixDefinition(latex: string): { name: string; size: 2 | 3 } | null {
  const k = latex.indexOf('=')
  if (k < 0) return null
  const name = latex.slice(0, k).trim()
  if (!/^([A-Za-z](_\{?\d+\}?)?|\\vec\{[A-Za-z]\w*\})$/.test(name)) return null
  const m = parseMatrix(latex.slice(k + 1))
  if (!m || rowsOf(m) !== colsOf(m) || (rowsOf(m) !== 2 && rowsOf(m) !== 3)) return null
  return { name, size: rowsOf(m) as 2 | 3 }
}

// ---- 小さくした分を覚えておく（大きさを戻すと、元の成分が戻る）----
// 「覚え」は、今の行列と、大きさを小さくしたときに外れた成分を合わせた表（今の大きさ以上）。今の行列の成分がいつも優先

/** 覚え ghost に、今の行列 m を重ねる（m の成分で上書きし、外の成分は残す） */
export function remember(ghost: Matrix | null, m: Matrix): Matrix {
  const rows = Math.max(rowsOf(m), ghost ? rowsOf(ghost) : 0)
  const cols = Math.max(colsOf(m), ghost ? colsOf(ghost) : 0)
  const cells = Array.from({ length: rows }, (_, i) =>
    Array.from({ length: cols }, (_, j) => (i < rowsOf(m) && j < colsOf(m) ? m.cells[i][j] : (ghost?.cells[i]?.[j] ?? ''))),
  )
  return { ...m, cells }
}

/** 行列の編集 */
export type MatrixOp = { op: 'row+' | 'col+'; at: number } | { op: 'row-' | 'col-'; at: number } | { op: 'size'; rows: number; cols: number }

/**
 * 覚えを使って行列を編集する。結果の行列と、新しい覚え。
 * - 大きさを小さくする・最後の行（列）を消す: 外れた成分は覚えに残す
 * - 大きさを大きくする・最後に行（列）を足す: 覚えにある成分を戻す（無ければ空欄）
 * - 途中の行（列）を足す・消す: 覚えの行（列）も一緒にずらす（消した行は、覚えからも消す）
 */
export function editWithMemory(m: Matrix, ghost: Matrix | null, e: MatrixOp): { next: Matrix; ghost: Matrix } {
  const g = remember(ghost, m)
  const take = (src: Matrix, rows: number, cols: number): Matrix => ({
    ...m,
    cells: Array.from({ length: rows }, (_, i) => Array.from({ length: cols }, (_, j) => src.cells[i]?.[j] ?? '')),
  })
  const [R, C] = [rowsOf(m), colsOf(m)]
  const clamp = (n: number) => Math.max(1, Math.min(MAX_SIZE, Math.round(n)))
  switch (e.op) {
    case 'size':
      return { next: take(g, clamp(e.rows), clamp(e.cols)), ghost: g }
    case 'row+': {
      if (R >= MAX_SIZE) return { next: m, ghost: g }
      if (e.at >= R) return { next: take(g, R + 1, C), ghost: g }
      const g2 = insertRow(g.cells.length >= MAX_SIZE ? removeRow(g, rowsOf(g) - 1) : g, e.at)
      return { next: take(g2, R + 1, C), ghost: g2 }
    }
    case 'col+': {
      if (C >= MAX_SIZE) return { next: m, ghost: g }
      if (e.at >= C) return { next: take(g, R, C + 1), ghost: g }
      const g2 = insertCol(colsOf(g) >= MAX_SIZE ? removeCol(g, colsOf(g) - 1) : g, e.at)
      return { next: take(g2, R, C + 1), ghost: g2 }
    }
    case 'row-': {
      if (R <= 1 || e.at < 0 || e.at >= R) return { next: m, ghost: g }
      if (e.at === R - 1) return { next: take(g, R - 1, C), ghost: g }
      const g2 = removeRow(g, e.at)
      return { next: take(g2, R - 1, C), ghost: g2 }
    }
    case 'col-': {
      if (C <= 1 || e.at < 0 || e.at >= C) return { next: m, ghost: g }
      if (e.at === C - 1) return { next: take(g, R, C - 1), ghost: g }
      const g2 = removeCol(g, e.at)
      return { next: take(g2, R, C - 1), ghost: g2 }
    }
  }
}
