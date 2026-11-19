// 数式入力欄（MathLive）の LaTeX を、計算エンジン（Rust）が読める式の文字列に変換する。
//
// 例) \frac{x^{2}+1}{y}            → ((x^(2)+1)/(y))
//     \sin\left(x\right)\cdot y     → sin (x)*y
//     \operatorname{nCr}\left(5,2\right) → nCr(5,2)
//     \left|x\right|                → abs(x)
//
// 文字が続けて書かれた部分（例: "floor"）はそのまま残し、エンジン側の「関数名の最長一致」で読ませる。

import { engineNameOf, isAccentCommand, isNameCommand } from './names.ts'

/** 変換できない書き方があったときのエラー */
export class LatexError extends Error {}

/** そのまま関数名として出力するコマンド */
const FUNCTIONS = new Set([
  'sin', 'cos', 'tan', 'sinh', 'cosh', 'tanh', 'arcsin', 'arccos', 'arctan', 'exp', 'ln', 'log',
  // ふつうの LaTeX で関数として書かれるもの（\max{…} ではなく \max\left(…\right) の形で使う）
  'max', 'min', 'arg', 'Re', 'Im', 'sec', 'csc', 'cot',
])

/** 単純に置き換えるコマンド */
const SYMBOLS: Record<string, string> = {
  pi: ' pi ',
  Gamma: ' gamma ',
  zeta: ' zeta ',
  // 座標の名前（座標系。計画 S38）。計算エンジンは theta・phi・rho を 1 つの変数として読む
  theta: ' theta ',
  vartheta: ' theta ',
  phi: ' phi ',
  varphi: ' phi ',
  rho: ' rho ',
  varrho: ' rho ',
  sigma: ' sigma ',
  varsigma: ' sigma ',
  tau: ' tau ',
  mu: ' mu ',
  nu: ' nu ',
  psi: ' psi ',
  exponentialE: 'e',
  imaginaryI: 'i',
  cdot: '*',
  // 外積（スカラーどうしなら、計算エンジンが掛け算にする）
  times: '×',
  ast: '*',
  div: '/',
  ',': ' ',
  ';': ' ',
  ':': ' ',
  '!': '',
  ' ': ' ',
  quad: ' ',
  qquad: ' ',
  // 不等号（連立方程式・不等式。計算エンジンは <=・>= を ≤・≥ として読む）
  le: '<=',
  leq: '<=',
  leqslant: '<=',
  leqq: '<=',
  ge: '>=',
  geq: '>=',
  geqslant: '>=',
  geqq: '>=',
  lt: '<',
  gt: '>',
  // 否定つきの不等号は、逆の不等号（計画 M13）
  nless: '>=',
  ngtr: '<=',
  nleq: '>',
  nleqslant: '>',
  nleqq: '>',
  ngeq: '<',
  ngeqslant: '<',
  ngeqq: '<',
  lneq: '<',
  lneqq: '<',
  lvertneqq: '<',
  gneq: '>',
  gneqq: '>',
  gvertneqq: '>',
  // 小さな記号（計画 M13）
  equiv: '=',
  bmod: '%%',
  divides: '∣',
  perp: '⟂',
  parallel: '∥',
  degree: '*(pi/180)',
  '%': '/100',
  star: '*',
  bullet: '*',
  centerdot: '*',
  Re: ' Re ',
  Im: ' Im ',
  angle: ' @angle ',
  gcd: ' gcd ',
  // 数学の記法（計画 docs/plan-math-notation.md）。計算エンジンが読む文字に置き換える
  pm: '±',
  mp: '∓',
  ldots: '…',
  cdots: '…',
  dots: '…',
  dotsc: '…',
  dotsb: '…',
  circ: '∘',
  // 論理・集合（計画 M11）。XOR・NAND・NOR・対称差も
  land: '∧',
  wedge: '∧',
  And: '∧',
  lor: '∨',
  vee: '∨',
  neg: '¬',
  lnot: '¬',
  implies: '⇒',
  Rightarrow: '⇒',
  Longrightarrow: '⇒',
  impliedby: '⇐',
  Leftarrow: '⇐',
  Longleftarrow: '⇐',
  iff: '⇔',
  Leftrightarrow: '⇔',
  Longleftrightarrow: '⇔',
  biconditional: '⇔',
  oplus: '⊕',
  veebar: '⊕',
  nleftrightarrow: '⊕',
  nLeftrightarrow: '⊕',
  barwedge: '⊼',
  uparrow: '⊼',
  downarrow: '⊽',
  nor: '⊽',
  cap: '∩',
  cup: '∪',
  setminus: '∖',
  smallsetminus: '∖',
  backslash: '∖',
  triangle: '△',
  bigtriangleup: '△',
  vartriangle: '△',
  ominus: '△',
  in: '∈',
  notin: '∉',
  ni: '∋',
  owns: '∋',
  ne: '≠',
  neq: '≠',
  emptyset: '∅',
  varnothing: '∅',
  top: '⊤',
  intercal: '⊺',
  bot: '⊥',
  complement: '∁',
  mid: '∣',
  // 微分（\frac{d}{dx} などの部品。単独では計算エンジンが読めない）とプライム
  partial: '∂',
  infty: '∞',
  differentialD: 'd',
  // 共役転置 A^{\dagger}（転置 ^{\top}・^{\intercal} は ^ の所で T にする）
  dagger: '†',
  dag: '†',
  // 行列の演算子（括弧なしで直後の 1 つに掛かる。計算エンジンの解析器が決める）
  det: ' @det ',
  ker: ' @ker ',
  dim: ' @dim ',
  prime: "'",
  doubleprime: "''",
  limits: '',
  nolimits: '',
  lfloor: ' floor(',
  rfloor: ')',
  lceil: ' ceil(',
  rceil: ')',
}

/** \left / \right の後に来る括弧 → 開き側の出力 */
const OPENERS: Record<string, string> = {
  '(': '(',
  '[': '[',
  '\\lbrack': '[', // キーボードで [ を打つと、MathLive は \left\lbrack … \right\rbrack と書く（リスト）
  '\\{': '(',
  '|': ' abs(',
  '\\vert': ' abs(',
  '\\lvert': ' abs(',
  '\\lfloor': ' floor(',
  '\\lceil': ' ceil(',
  '.': '(',
}

type Token = string // "\\name"（コマンド）か 1 文字

function tokenize(src: string): Token[] {
  return repairSetBraces(normalizeSetTokens(rawTokenize(src)))
}

/**
 * 入力の途中で崩れた集合の括弧を直す（MathLive で } を消してから打ち直したときなど）。
 * - `\left\{ … \right.` の後に、同じ深さで単独の `\}` があれば、`\right.` を消して `\}` を `\right\}` にする
 *   （} を消すと MathLive は閉じ括弧を見えない `\right.` にし、打ち直した } は別の `\rbrace` になる）
 * - 対応する開き括弧の無い単独の `\}` は、その深さのかたまりの始まり（区切り `=`・`\in` などの後）に `\{` を補う
 *   （積分記号の下で打った { が添字の括弧になり、集合の { が消えた形 `_{(x,y)\mid … \}}`）
 */
export function repairSetBraces(toks: Token[]): Token[] {
  type Frame = { kind: 'group' | 'left' | 'brace'; start: number; open?: Token; pending?: number }
  const stack: Frame[] = [{ kind: 'group', start: 0 }]
  const drop = new Set<number>()
  const replace = new Map<number, Token[]>()
  const insertBefore = new Map<number, Token[]>()
  const SEPS = new Set(['=', '\\in', '\\notin', '\\cap', '\\cup', '\\setminus', ',', '\\times', '\\ni'])
  let changed = false
  for (let k = 0; k < toks.length; k++) {
    const t = toks[k]
    const top = stack[stack.length - 1]
    if (t === '{') stack.push({ kind: 'group', start: k + 1 })
    else if (t === '}') {
      if (stack.length > 1 && top.kind === 'group') stack.pop()
    } else if (t === '\\left') {
      stack.push({ kind: 'left', start: k + 2, open: toks[k + 1] })
      k++
    } else if (t === '\\right') {
      // 対応する \left まで戻る（途中の単独の \{ は閉じていないまま捨てる）
      while (stack.length > 1 && stack[stack.length - 1].kind === 'brace') stack.pop()
      const f = stack[stack.length - 1]
      if (stack.length > 1 && f.kind === 'left') {
        stack.pop()
        // 見えない閉じ括弧の集合: 後で単独の \} が来たら、そこまでを集合にする
        if (f.open === '\\{' && toks[k + 1] === '.') stack[stack.length - 1].pending = k
      }
      k++
    } else if (t === '\\{') {
      stack.push({ kind: 'brace', start: k + 1 })
    } else if (t === '\\}') {
      if (top.kind === 'brace') {
        stack.pop()
      } else if (top.pending !== undefined) {
        drop.add(top.pending)
        drop.add(top.pending + 1)
        replace.set(k, ['\\right', '\\}'])
        top.pending = undefined
        changed = true
      } else {
        // 開き括弧の無い \}: かたまりの始まり（\mid より前の、最後の区切りの後）に \{ を補う
        let depth = 0
        let at = top.start
        for (let j = top.start; j < k; j++) {
          const u = toks[j]
          // \\left を付けない ( ) も深さに数える（点 (x, y) の中のカンマを区切りにしない）
          if (u === '{' || u === '\\left' || u === '(') depth++
          else if (u === '}' || u === '\\right' || u === ')') depth = Math.max(0, depth - 1)
          else if (depth === 0 && u === '\\mid') break
          else if (depth === 0 && SEPS.has(u)) at = j + 1
        }
        insertBefore.set(at, [...(insertBefore.get(at) ?? []), '\\{'])
        changed = true
      }
    }
  }
  if (!changed) return toks
  const out: Token[] = []
  for (let k = 0; k <= toks.length; k++) {
    if (insertBefore.has(k)) out.push(...insertBefore.get(k)!)
    if (k === toks.length) break
    if (drop.has(k)) continue
    if (replace.has(k)) out.push(...replace.get(k)!)
    else out.push(toks[k])
  }
  return out
}

/** 空白の命令（\, など）か */
const isSpaceTok = (t: Token | undefined) => t === '\\,' || t === '\\;' || t === '\\:' || t === '\\ ' || t === '\\!' || t === '\\quad' || t === '\\qquad'

/**
 * 集合の書き方の揺れをそろえる。MathLive のキーボードで { を打つと \left\lbrace … \right\rbrace、集合の中で | を打つと
 * \,\middle\vert\,（\middle{\vert}・\middle| も）になるので、\left\{ … \right\} と \mid に直す。
 * \mid の両側の空白（\,）も消す（点と条件の区切りの判定・括弧の外し方が、空白で崩れないように）
 */
function normalizeSetTokens(toks: Token[]): Token[] {
  const out: Token[] = []
  for (let k = 0; k < toks.length; k++) {
    const t = toks[k]
    // 区間・実数全体の累乗 [0,1]^2・\mathbb{R}^3 は直積 [0,1]×[0,1]（∈ の右・× の隣・積分記号の下の先頭だけ。リストの累乗と取り違えない）
    const power = boxPower(toks, k, out)
    if (power) {
      out.push(...power.toks)
      k = power.next - 1
      continue
    }
    if (t === '\\lbrace') {
      out.push('\\{')
      continue
    }
    if (t === '\\rbrace') {
      out.push('\\}')
      continue
    }
    if (t === '\\middle') {
      const bar = (u: Token | undefined) => u === '|' || u === '\\vert' || u === '\\mid' || u === '\\lvert' || u === '\\rvert'
      if (bar(toks[k + 1])) {
        k += 1
      } else if (toks[k + 1] === '{' && bar(toks[k + 2]) && toks[k + 3] === '}') {
        k += 3
      } else {
        continue // そのほかの \middle（\middle/ など）は、区切りの形だけ残す
      }
      while (isSpaceTok(out[out.length - 1])) out.pop()
      out.push('\\mid')
      while (isSpaceTok(toks[k + 1])) k++
      continue
    }
    if (t === '\\mid') {
      while (isSpaceTok(out[out.length - 1])) out.pop()
      out.push(t)
      while (isSpaceTok(toks[k + 1])) k++
      continue
    }
    out.push(t)
  }
  return out
}

/** toks[k] から始まる「区間^n」「\\mathbb{R}^n」を直積に開いたトークンと、読み終わった位置。当てはまらなければ null */
function boxPower(toks: Token[], k: number, before: Token[]): { toks: Token[]; next: number } | null {
  const prev = before[before.length - 1]
  const atSetPlace = prev === '\\in' || prev === '\\times' || prev === '_' || (prev === '{' && before[before.length - 2] === '_')
  if (!atSetPlace) return null
  let box: Token[]
  let j: number
  if (toks[k] === '\\mathbb' && toks[k + 1] === '{' && toks[k + 2] === 'R' && toks[k + 3] === '}') {
    box = ['\\left', '[', '-', '\\infty', ',', '\\infty', '\\right', ']']
    j = k + 4
  } else if (toks[k] === '\\left' && (toks[k + 1] === '[' || toks[k + 1] === '\\lbrack' || toks[k + 1] === '(')) {
    let depth = 0
    j = k
    for (; j < toks.length; j++) {
      if (toks[j] === '\\left') depth++
      else if (toks[j] === '\\right' && --depth === 0) break
    }
    if (j >= toks.length) return null
    j += 2
    box = toks.slice(k, j)
    if (!box.includes(',')) return null
  } else return null
  if (toks[j] !== '^') return null
  const braced = toks[j + 1] === '{' && toks[j + 3] === '}'
  const n = Number(braced ? toks[j + 2] : toks[j + 1])
  if (!(n === 2 || n === 3)) return null
  const outToks: Token[] = []
  for (let i = 0; i < n; i++) outToks.push(...(i > 0 ? ['\\times'] : []), ...box)
  return { toks: outToks, next: j + (braced ? 4 : 2) }
}

function rawTokenize(src: string): Token[] {
  const out: Token[] = []
  let i = 0
  while (i < src.length) {
    const c = src[i]
    if (c === '\\') {
      let j = i + 1
      if (j < src.length && /[a-zA-Z]/.test(src[j])) {
        while (j < src.length && /[a-zA-Z]/.test(src[j])) j++
      } else {
        j++ // \, \; \{ など1文字のコマンド
      }
      out.push(src.slice(i, j))
      i = j
    } else if (/\s/.test(c)) {
      i++ // LaTeX の空白は意味を持たない
    } else {
      out.push(c)
      i++
    }
  }
  return out
}

class Converter {
  private i = 0
  private out = ''
  /** \left なしの | で開いている絶対値の数 */
  private bars = 0

  private toks: Token[]

  constructor(toks: Token[]) {
    // 条件の並びの省略形 a ≤ x, y ≤ b（関係の連立・集合・場合分け・Σ の下、どこに書いても同じに読む）
    this.toks = toks.length > 4 && toks.includes(',') ? expandListedRanges(toks) : toks
  }

  run(): string {
    this.seq(null)
    if (this.i < this.toks.length) throw new LatexError(`余分な "${this.toks[this.i]}" があります`)
    return this.out.replace(/\s+/g, ' ').trim()
  }

  private peek(): Token | undefined {
    return this.toks[this.i]
  }

  /** 直前に出力したものが「値の終わり」か（| が開きか閉じかの判定に使う） */
  private afterValue(): boolean {
    const t = this.out.trimEnd()
    return /[\w)!.]$/.test(t)
  }

  /** end のトークン（'}' や '\right'）が来るまで変換する */
  private seq(end: Token | null) {
    while (this.i < this.toks.length) {
      const t = this.toks[this.i]
      if (t === end) return
      if (t === '}' || t === '\\right') return
      this.i++
      this.token(t)
    }
    if (end !== null) throw new LatexError('括弧が閉じていません')
  }

  /** 引数1つ分（{…} か 1 トークン）を変換した文字列を返す */
  private arg(): string {
    const saved = this.out
    this.out = ''
    const t = this.toks[this.i++]
    if (t === undefined) throw new LatexError('式が途中で終わっています')
    if (t === '{') {
      this.seq('}')
      this.i++ // '}'
    } else {
      this.token(t)
    }
    const r = this.out
    this.out = saved
    return r.trim()
  }

  /**
   * {…} の中身を変換せずに文字列として返す（\operatorname{nCr} など）。
   * lettersOnly なら、中の命令（\mathrm など）と {} を除いた文字だけを返す
   */
  private rawGroup(lettersOnly = false): string {
    const keep = (t: string) => !lettersOnly || !(t.startsWith('\\') || t === '{' || t === '}')
    if (this.toks[this.i] !== '{') {
      const t = this.toks[this.i++] ?? ''
      return keep(t) ? t : ''
    }
    this.i++
    let depth = 1
    let s = ''
    while (this.i < this.toks.length) {
      const t = this.toks[this.i++]
      if (t === '{') depth++
      if (t === '}' && --depth === 0) break
      if (keep(t)) s += t
    }
    return s
  }

  private token(t: Token) {
    // ---- 1文字のもの ----
    if (t === '{') {
      this.out += '(' + this.argFromOpenBrace() + ')'
      return
    }
    if (t === '^') {
      // 度: x^{\circ} → x·(π/180)
      if (this.peek() === '\\circ' || (this.peek() === '{' && this.toks[this.i + 1] === '\\circ' && this.toks[this.i + 2] === '}')) {
        this.i += this.peek() === '{' ? 3 : 1
        this.out += '*(pi/180)'
        return
      }
      const a = this.arg()
      // 共役 z^{\ast}・z^{*}（行列なら成分ごと。計算エンジンの展開の段が決める）
      if (a.trim() === '*' || a.trim() === '∗') {
        this.out += '^(∗)'
        return
      }
      // 補集合 A^{\complement}
      if (a.trim() === '∁') {
        this.out += 'ᶜ'
        return
      }
      // 転置 A^{\top}・A^{\intercal} は A^T と同じ（底が行列かどうかは、計算エンジンの展開の段が決める）
      if (a.trim() === '⊤' || a.trim() === '⊺') {
        this.out += '^(T)'
        return
      }
      // f^{\prime} は f'（導関数）
      this.out += /^'+$/.test(a.replace(/\s+/g, '')) ? a.replace(/\s+/g, '') : '^(' + a + ')'
      return
    }
    if (t === '_') {
      // 添字: w_{2} → w_2、t_1 → t_1（文字の後ろに数字だけを付けられる）
      const sub = this.arg()
      // \vec{v}_{2}: 名前 {vec_v} の後の空白を詰める
      if (/\}\s+$/.test(this.out)) this.out = this.out.trimEnd()
      if (!/[a-zA-Z}]$/.test(this.out)) throw new LatexError('添字は文字の後ろに付けてください（例: w₂）')
      // 数字でない添字（行列の成分 A_{1,2}・数列の項 a_{n+1}）は _( ) の形
      this.out += /^\d+$/.test(sub) ? `_${sub}` : `_(${sub})`
      return
    }
    // [ ] はリスト（媒介変数表示の中で、要素ごとに図形をまとめて描く）
    if (t === '[' || t === '\\lbrack') {
      this.out += '['
      return
    }
    if (t === ']' || t === '\\rbrack') {
      this.out += ']'
      return
    }
    if (t === '|' || t === '\\vert' || t === '\\lvert' || t === '\\rvert') {
      if (this.bars > 0 && this.afterValue()) {
        this.out += ')'
        this.bars--
      } else {
        this.out += ' abs('
        this.bars++
      }
      return
    }
    if (!t.startsWith('\\')) {
      this.out += t
      return
    }

    // ---- コマンド ----
    const name = t.slice(1)
    // 添字つきの最大・最小 \max_{t\in[a,b]} 項 → @max(t,a,b) 項（等分点での近似。計画 M12）
    if (['max', 'min', 'sup', 'inf'].includes(name) && this.peek() === '_') {
      this.i++
      const sub = this.arg().replace(/\s+/g, '')
      const m = sub.match(/^([A-Za-z]\w*|\{\w+\})∈[[(](.+),(.+)[\])]$/)
      if (!m) throw new LatexError(`\\${name} の下には、t ∈ [0, 1] の形で変数と範囲を書いてください`)
      const op = name === 'sup' ? 'max' : name === 'inf' ? 'min' : name
      this.out += ` @${op}(${m[1].replace(/[{}]/g, '')},${m[2]},${m[3]}) `
      return
    }
    if (name === 'exists' || name === 'forall') {
      // ∃a∈[0,1], 中身 → @exists(a,0,1) 中身（行の先頭だけ）
      const head: Token[] = []
      let depth = 0
      while (this.i < this.toks.length) {
        const u = this.toks[this.i]
        if (depth === 0 && u === ',') {
          this.i++
          break
        }
        if (u === '{' || u === '\\left') depth++
        if (u === '}' || u === '\\right') depth--
        head.push(u)
        this.i++
      }
      const h = new Converter(head).run().replace(/\s+/g, '')
      const m = h.match(/^([A-Za-z]\w*|\{\w+\})∈[[(](.+),(.+)[\])]$/)
      if (!m) throw new LatexError(`${name === 'exists' ? '∃' : '∀'} の後には、a ∈ [0, 1], の形で変数と範囲を書いてください`)
      this.out += ` @${name}(${m[1].replace(/[{}]/g, '')},${m[2]},${m[3]}) `
      return
    }
    // 名前に使える文字（ギリシャ文字・文字の形の記号。計画 M13）。計算エンジンでは {alpha} の形
    if (isNameCommand(name)) {
      this.out += ` {${name}} `
      return
    }
    // アクセントを付けた 1 文字 \hat{x}（元の文字とは別の名前）
    if (isAccentCommand(name)) {
      const inner = this.rawGroup().trim()
      const n = engineNameOf(name, inner)
      if (!n) throw new LatexError(`\\${name}{ } の中には、文字を 1 つ書いてください`)
      this.out += ` {${n}} `
      return
    }
    if (FUNCTIONS.has(name)) {
      this.out += ` ${name} `
      return
    }
    if (name in SYMBOLS) {
      this.out += SYMBOLS[name]
      return
    }
    switch (name) {
      case 'frac':
      case 'dfrac':
      case 'tfrac':
      case 'cfrac': {
        const a = this.arg()
        const b = this.arg()
        const d = derivative(a, b)
        this.out += d ?? `((${a})/(${b}))`
        return
      }
      case 'sqrt': {
        let n: string | null = null
        if (this.peek() === '[') {
          // \sqrt[n]{x}: [ ] の中を変換
          this.i++
          const saved = this.out
          this.out = ''
          while (this.i < this.toks.length && this.toks[this.i] !== ']') this.token(this.toks[this.i++])
          this.i++ // ']'
          n = this.out.trim()
          this.out = saved
        }
        const x = this.arg()
        this.out += n === null ? ` sqrt(${x})` : `((${x})^(1/(${n})))`
        return
      }
      case 'left': {
        const d = this.toks[this.i++] ?? ''
        // 内積 ⟨a, b⟩ と、ノルム ‖a‖
        if (d === '\\langle' || d === '<' || d === '\\Vert' || d === '\\|' || d === '\\lVert') {
          const saved = this.out
          this.out = ''
          this.seq('\\right')
          if (this.toks[this.i] !== '\\right') throw new LatexError('括弧が閉じていません')
          this.i += 2
          const inner = this.out.trim()
          this.out = `${saved} @${d === '\\langle' || d === '<' ? 'dot' : 'norm'}(${inner}) `
          return
        }
        // 点での値 \left. 式 \right|_{x=a, y=b} → @at(式, x, a, y, b)
        if (d === '.') {
          const saved = this.out
          this.out = ''
          this.seq('\\right')
          if (this.toks[this.i] !== '\\right') throw new LatexError('括弧が閉じていません')
          const close = this.toks[this.i + 1] ?? ''
          if ((close === '|' || close === '\\vert' || close === '\\rvert') && this.toks[this.i + 2] === '_') {
            const inner = this.out.trim()
            this.i += 3
            const sub = this.rawTokens()
            const pairs = splitTop(sub, (t) => t === ',').map((p) => {
              const eq = p.indexOf('=')
              if (eq <= 0 || eq === p.length - 1) throw new LatexError('点での値は、式|_{x = a, y = b} の形で書いてください')
              return `${new Converter(p.slice(0, eq)).run().trim()},${new Converter(p.slice(eq + 1)).run()}`
            })
            this.out = `${saved} @at(${inner},${pairs.join(',')}) `
            return
          }
          this.out = `${saved}(${this.out})`
          this.i += 2
          return
        }
        if (!(d in OPENERS)) throw new LatexError(`括弧 ${d} は使えません`)
        // 集合の括弧 \left\{ 点 \mid 条件 \right\}
        if (d === '\\{' && this.setBuilder('\\right')) return
        // ∈ のすぐ後の \left\{1,4,9\right\} は有限集合
        if (d === '\\{' && /[∈∉]$/.test(this.out.trimEnd())) {
          const saved = this.out
          this.out = ''
          this.seq('\\right')
          if (this.toks[this.i] !== '\\right') throw new LatexError('括弧が閉じていません')
          this.i += 2
          const inner = this.out.trim()
          this.out = `${saved}⦃${inner}⦄`
          return
        }
        this.out += OPENERS[d]
        this.seq('\\right')
        if (this.toks[this.i] !== '\\right') throw new LatexError('括弧が閉じていません')
        const close = this.toks[this.i + 1] ?? ''
        this.i += 2 // \right と閉じ括弧
        // 半開区間 (0, 1]・[0, 1) は、閉じ括弧の形を残す（∈ の右で区間になる）
        this.out += close === ']' || close === '\\rbrack' ? ']' : OPENERS[d] === '[' && close !== ')' ? ']' : ')'
        return
      }
      case '{':
      case 'lbrace':
        // \{ 点 \mid 条件 \}（\left の無い集合の括弧）
        if (this.setBuilder(null)) return
        throw new LatexError('集合は \\{(x, y) \\mid 条件\\} の形で書いてください')
      case 'mathbb': {
        const c = this.rawGroup(true)
        const sets: Record<string, string> = { R: 'ℝ', Z: 'ℤ', N: 'ℕ', C: 'ℂ', Q: 'ℚ' }
        if (c in sets) {
          this.out += sets[c]
          return
        }
        // そのほかの 1 文字は、ベクトルの名前（\vec{F}・\mathbf{F} と同じ名前。黒板太字は、手書きの太字の書き方）
        if (/^[A-Za-z]$/.test(c)) {
          this.out += ` {vec_${c}} `
          return
        }
        const n = engineNameOf('mathbb', c)
        if (!n) throw new LatexError(`\\mathbb{${c}} はまだ使えません`)
        this.out += ` {${n}} `
        return
      }
      case 'mathcal':
      case 'mathscr':
      case 'mathfrak':
      case 'mathsf':
      case 'mathtt': {
        const c = this.rawGroup(true)
        const n = engineNameOf(name, c)
        if (!n) throw new LatexError(`\\${name}{ } の中には、英字を 1 つ書いてください`)
        this.out += ` {${n}} `
        return
      }
      case 'bar':
      case 'overline':
        // 共役（計画 M13）
        this.out += ` conj(${this.arg()}) `
        return
      case 'binom':
      case 'dbinom':
      case 'tbinom': {
        const n = this.arg()
        const k = this.arg()
        this.out += ` nCr(${n},${k}) `
        return
      }
      case 'boxed':
        this.out += `(${this.arg()})`
        return
      case 'underbrace':
      case 'overbrace': {
        // 中身だけ読む（下・上の注釈は捨てる）
        const body = this.arg()
        if (this.peek() === '_' || this.peek() === '^') {
          this.i++
          this.rawTokens()
        }
        this.out += `(${body})`
        return
      }
      case 'operatorname':
      case 'mathrm':
      case 'text':
      case 'mathit':
      case 'mathbf': {
        // 中身は名前の文字だけを取り出す。数式キーボードから入れると
        // \operatorname{\mathrm{Im}} のように命令が入れ子になることがあるため、命令名と {} を取り除く
        const s = this.rawGroup(true)
        // 太字の 1 文字 \mathbf{F} はベクトルの名前（\vec{F} と同じ名前。F とは別）
        if (name === 'mathbf' && /^[A-Za-z]$/.test(s)) {
          this.out += ` {vec_${s}} `
          return
        }
        // 最小公倍数・最大公約数（計画 M13）
        if (s === 'lcm' || s === 'gcd') {
          this.out += ` ${s} `
          return
        }
        // 像 im A・Image A・range A（Im A と同じ。引数がスカラーなら虚部）
        if (['im', 'Image', 'image', 'Ran', 'ran', 'range'].includes(s)) {
          this.out += ' Im '
          return
        }
        // 固有値・固有空間 eig A（Eig・Eigen も）
        if (['eig', 'Eig', 'eigen', 'Eigen'].includes(s)) {
          this.out += ' @eig '
          return
        }
        // テイラー多項式 Taylor(f, x, a, n)・Taylor(f, (x, y), (a, b), n)
        if (s === 'Taylor' || s === 'taylor') {
          this.out += ' @taylor '
          return
        }
        // 停留点（∇f = 0 の点）を求めて分類する crit f
        if (s === 'crit' || s === 'Crit') {
          this.out += ' @crit '
          return
        }
        // 行列の演算子の名前
        if (['tr', 'rank', 'det', 'ker', 'dim'].includes(s)) {
          this.out += ` @${s} `
          return
        }
        // ベクトル場の演算子の名前
        if (name === 'operatorname' && ['grad', 'div', 'rot', 'curl'].includes(s)) {
          this.out += ` @${s === 'curl' ? 'rot' : s} `
          return
        }
        this.out += s === 'e' || s === 'i' ? s : ` ${s} `
        return
      }
      case 'sum':
      case 'prod':
      case 'bigcup':
      case 'bigcap':
      case 'bigvee':
      case 'bigwedge': {
        // \sum_{n=1}^{10} → @sum(n,1,10)。中身は直後の項（計算エンジンの解析器が決める）。
        // 上が無ければ集合の上: \sum_{n\in S}・\sum_{1\le n\le10,\ …}・\sum_{d\mid12} → @sumset(n; 条件)。
        // \bigcup・\bigcap（\bigvee・\bigwedge）は、どちらの形も @bigcup(k; 条件)
        let lo: string | null = null
        let hi: string | null = null
        while (this.peek() === '_' || this.peek() === '^' || this.peek() === '\\limits' || this.peek() === '\\nolimits') {
          const t2 = this.toks[this.i++]
          if (t2 === '_') lo = this.arg()
          else if (t2 === '^') hi = this.arg()
        }
        const big = name.startsWith('big')
        const sym = name === 'sum' ? 'Σ' : name === 'prod' ? 'Π' : name === 'bigcup' ? '⋃' : name === 'bigcap' ? '⋂' : name === 'bigvee' ? '⋁' : '⋀'
        // 添字は 1 つか、カンマで並べた複数（\sum_{x,y,z=0}^{n} は x・y・z それぞれ 0〜n の 3 重の和）
        const m = lo?.match(/^\s*([A-Za-z]\w*(?:\s*,\s*[A-Za-z]\w*)*)\s*=(.+)$/)
        const idx = m ? m[1].split(',').map((v) => v.trim()) : []
        if (lo !== null && (hi === null || big) && !(m && hi !== null)) {
          // 集合の上: 添字は「n ∈ …」「(m,n) ∈ …」「d ∣ …」の左、または連鎖 a ≤ n ≤ b の真ん中
          const cond = lo.trim()
          const v =
            cond.match(/^\s*(\([^)]*\)|[A-Za-z]\w*)\s*[∈∉∣]/)?.[1] ??
            cond.match(/^[^,]*?(?:<=|<|>=|>)\s*([A-Za-z]\w*)\s*(?:<=|<|>=|>)/)?.[1] ??
            cond.match(/^\s*([A-Za-z]\w*)\s*(?:<=|<|>=|>)/)?.[1]
          if (!v) throw new LatexError(`${sym} の下に、n ∈ S や 1 ≤ n ≤ 10 のように添字を書いてください`)
          const op = big ? (name === 'bigcup' || name === 'bigvee' ? 'bigcup' : 'bigcap') : name === 'sum' ? 'sumset' : 'prodset'
          this.out += ` @${op}(${v}; ${cond}) `
          return
        }
        if (big) {
          // \bigcup_{k=1}^{3} → @bigcup(k; 1<=k<=3)
          if (!m || hi === null || hi.trim() === '') throw new LatexError(`${sym} の下と上に、k=1 と 3 のように添字の範囲を書いてください`)
          const op = name === 'bigcup' || name === 'bigvee' ? 'bigcup' : 'bigcap'
          for (const v of idx) this.out += ` @${op}(${v}; ${m[2].trim()}<=${v}<=${hi.trim()}) `
          return
        }
        if (!m) throw new LatexError(`${sym} の下に n=1 の形で書いてください`)
        if (hi === null || hi.trim() === '') throw new LatexError(`${sym} の上に終わりの値を書いてください`)
        for (const v of idx) this.out += ` @${name}(${v},${m[2].trim()},${hi.trim()}) `
        return
      }
      case 'Delta':
      case 'nabla':
      case 'delta': {
        // 差分（下付きがあるときだけ）: \Delta_{x} f → @fdiff(x) f、\nabla_{x,h} → @bdiff(x,h)、\delta_{x}^{2} → @cdiff(x,1,2)。
        // 下付きの無い \nabla・\Delta は、勾配・ラプラシアン（まだ使えない）、\delta は文字の名前（まだ使えない）
        let sub: string | null = null
        let sup: string | null = null
        while (this.peek() === '_' || this.peek() === '^') {
          if (this.toks[this.i++] === '_') sub = this.arg()
          else sup = this.arg()
        }
        if (sub === null) {
          // 下付きが無ければ、ベクトル場の演算子: ∇f（勾配）・∇·F（発散）・∇×F（回転）・∇²f と Δf（ラプラシアン）
          if (name === 'nabla' && (sup === null || sup.trim() === '2')) {
            if (sup !== null) this.out += ' @lap '
            else if (this.peek() === '\\cdot') {
              this.i++
              this.out += ' @div '
            } else if (this.peek() === '\\times') {
              this.i++
              this.out += ' @rot '
            } else this.out += ' @grad '
            return
          }
          if (name === 'Delta' && sup === null) {
            this.out += ' @lap '
            return
          }
          // 下付きの無い \delta は、名前（計画 M13）
          if (name === 'delta' && sup === null) {
            this.out += ' {delta} '
            return
          }
          throw new LatexError(`${t} はまだ使えません（差分なら ${t}_{x} のように変数を下に書いてください）`)
        }
        const op = name === 'Delta' ? 'fdiff' : name === 'nabla' ? 'bdiff' : 'cdiff'
        // 下付きは「変数」か「変数, 幅」。幅の中のカンマ（括弧の中）では分けない
        let depth = 0
        let cut = -1
        for (let k = 0; k < sub.length; k++) {
          const c = sub[k]
          if (c === '(' || c === '[') depth++
          else if (c === ')' || c === ']') depth--
          else if (c === ',' && depth === 0) {
            cut = k
            break
          }
        }
        const v = (cut < 0 ? sub : sub.slice(0, cut)).trim()
        const h = cut < 0 ? '' : sub.slice(cut + 1).trim()
        if (!/^[A-Za-z]\w*$/.test(v)) throw new LatexError(`${t} の下には、差分をとる変数を書いてください（例: ${t}_{x}、幅を変えるなら ${t}_{x, h}）`)
        const order = sup?.trim() ?? ''
        if (order !== '' && !/^\d+$/.test(order)) throw new LatexError(`${t} の右上には、階数（整数）を書いてください`)
        const parts = [v, ...(h !== '' || order !== '' ? [h === '' ? '1' : h] : []), ...(order !== '' ? [order] : [])]
        this.out += ` @${op}(${parts.join(',')}) `
        return
      }
      case 'int':
      case 'intop':
      case 'smallint':
      case 'oint':
      case 'iint':
      case 'oiint':
      case 'iiint': {
        // 定積分 \int_{a}^{b} … dt → @int(t,a,b)(…)、不定積分 \int … dx → @int(x)(…)、
        // 領域の上の積分 \iint_{\{(x,y)\mid …\}} … dA・\int_{[0,1]\times[0,2]} … dx\,dy → @intset((x,y),条件…)(…)
        const multi = name === 'iint' || name === 'oiint' ? 2 : name === 'iiint' ? 3 : 1
        let subToks: Token[] | null = null
        let hi: string | null = null
        while (this.peek() === '_' || this.peek() === '^' || this.peek() === '\\limits' || this.peek() === '\\nolimits') {
          const t2 = this.toks[this.i++]
          if (t2 === '_') subToks = this.rawTokens()
          else if (t2 === '^') hi = this.arg()
        }
        if (subToks !== null && hi === null) this.unwrapForm(multi)
        const integrand = this.integrand(multi)
        const { body, vector } = integrand
        const vars = integrand.vars.map(engineVar)
        /** 積分の d の後の文字（計算エンジンが、範囲の次元と合うかを確かめる。ds の範囲が曲面なら誤り） */
        const marks = (vs: string[]) => vs.map((v) => `{d_${v}}`).join(',')
        if (vector) {
          // ベクトル場の線積分・流束 ∫_C F · dr、∬_S F · dS: 曲線・曲面を集合で書く
          if (subToks === null || hi !== null) throw new LatexError('F · dr・F · dS の積分は、下に曲線・曲面を {(cos t, sin t) | 0 ≤ t ≤ 2π} の形で書いてください')
          const pieces = regionsOf(subToks, [], multi).map((r) => ` @intset(${r}; {vec},${marks(vars)})(${body}) `)
          this.out += pieces.length === 1 ? pieces[0] : ` (${pieces.join('+')}) `
          return
        }
        if (subToks === null && hi === null) {
          if (multi > 1 || vars.length !== 1) throw new LatexError('不定積分は 1 変数で、∫ … dx の形で書いてください')
          this.out += ` @int(${vars[0]})(${body}) `
          return
        }
        if (subToks !== null && hi !== null) {
          if (multi > 1) throw new LatexError('重積分は、∬ の下に領域を書いてください（{(x,y) | …} か [0,1]×[0,2]）')
          const lo = new Converter(subToks).run()
          if (lo === '' || hi.trim() === '') throw new LatexError('積分の範囲を書いてください（∫ の下と上）')
          this.out += ` @int(${vars[0]},${lo},${hi.trim()})(${body}) `
          return
        }
        if (subToks === null) throw new LatexError('積分の範囲を書いてください（∫ の下と上）')
        // 範囲が曲線・曲面の和（∪）なら、範囲ごとの積分の和
        const regions = regionsOf(subToks, vars, multi)
        const over = (b: string, vs: string[]) => regions.map((r) => `@intset(${r}; ${marks(vs)})(${b})`).join(' + ')
        let sum = ` ${over(body, vars)} `
        let terms = regions.length
        // 曲線の上の P dx + Q dy (+ R dz): 続く「± 中身 d座標」も、同じ曲線の上の積分にして足す
        if (vars.length === 1 && COORD_LETTERS.has(vars[0]) && isCurveRegion(subToks)) {
          const more = formTerms(this.toks, this.i, vars[0])
          for (const t of more.terms) {
            const term = new Converter(this.toks.slice(t.from, t.end)).run()
            sum += `${t.sign} ${regions.length > 1 ? `(${over(term, [t.v])})` : over(term, [t.v])} `
            terms++
          }
          this.i = more.next
        }
        this.out += terms > 1 ? ` (${sum}) ` : sum
        return
      }
      case 'begin': {
        const env = this.rawGroup(true)
        const rows = this.environment(env)
        if (env === 'cases' || env === 'dcases' || env === 'rcases') {
          // 場合分け: 行ごとに「値 & 条件」。条件が空・otherwise・それ以外 なら、それ以外の場合（最後に書く）
          const parts = rows.map((cells) => {
            const value = cells[0] ?? ''
            const cond = (cells[1] ?? '').replace(/^\s*(if|when|もし)\b/, '').trim()
            if (value === '') throw new LatexError('場合分けの値を書いてください')
            return /^(|otherwise|else|それ以外|その他)$/.test(cond) ? value : `${value}, ${cond}`
          })
          this.out += ` @cases(${parts.join('; ')}) `
          return
        }
        if (/^(p|b|B|v|V|small)?matrix\*?$/.test(env)) {
          // 行列: 空欄は 0 として読む（大きさを変えている途中でも、エラーにしない）
          if (rows.length === 0) throw new LatexError('行列の成分を書いてください')
          const cols = Math.max(...rows.map((r) => r.length))
          const body = rows.map((r) => Array.from({ length: cols }, (_, k) => (r[k] ?? '') || '0').join(',')).join(';')
          this.out += env.startsWith('v') ? ` @det(⟦${body}⟧) ` : ` ⟦${body}⟧ `
          return
        }
        throw new LatexError(`${env} はまだ使えません`)
      }
      case 'dot':
      case 'ddot':
      case 'dddot': {
        // 時間の導関数 \dot{x} → x'、\ddot{x} → x''（常微分方程式。計画 M9）
        const v = this.rawGroup(true)
        if (!/^[A-Za-z]$/.test(v)) throw new LatexError(`\\${name}{ } の中には、文字を 1 つ書いてください`)
        this.out += v + "'".repeat(name === 'dot' ? 1 : name === 'ddot' ? 2 : 3)
        return
      }
      case 'vec':
      case 'overrightarrow':
      case 'boldsymbol': {
        // \vec{v} は、v とは別の名前 {vec_v}（ベクトルの名前）。太字 \boldsymbol{v}（と \mathbf{v}）も同じ名前
        const v = this.rawGroup(true)
        if (!/^[A-Za-z]\w*$/.test(v)) throw new LatexError(`\\${name}{ } の中には、名前を 1 つ書いてください`)
        this.out += ` {vec_${v}} `
        return
      }
      case 'lVert':
      case 'Vert':
      case '|': {
        // \lVert a \rVert・\| a \|（\left の無いノルム）
        const saved = this.out
        this.out = ''
        while (this.i < this.toks.length && !['\\rVert', '\\Vert', '\\|'].includes(this.toks[this.i])) this.token(this.toks[this.i++])
        if (this.i >= this.toks.length) throw new LatexError('ノルムの ‖ が閉じていません')
        this.i++
        const inner = this.out.trim()
        this.out = `${saved} @norm(${inner}) `
        return
      }
      case 'placeholder':
        this.rawGroup()
        if (this.peek() === '{') this.rawGroup()
        return
      case 'mleft':
        this.token('\\left')
        return
    }
    throw new LatexError(`${t} はまだ使えません`)
  }

  /**
   * \\begin{env}{…} の中身を、\\end{env} まで読み、行（\\\\ で区切る）ごとの欄（& で区切る）を変換した文字列の表にする。
   * 入れ子の \\begin … \\end の中の & と \\\\ は区切りにしない
   */
  private environment(env: string): string[][] {
    const keepEmpty = /matrix\*?$/.test(env)
    const rows: Token[][][] = [[[]]]
    let depth = 0
    while (this.i < this.toks.length) {
      const t = this.toks[this.i++]
      if (t === '\\end' && depth === 0) {
        this.rawGroup()
        const out = rows
          .map((cells) => cells.map((c) => (c.length ? new Converter(c).run() : '')))
          .filter((cells) => keepEmpty || cells.some((c) => c !== ''))
        return out
      }
      if (t === '\\begin') depth++
      if (t === '\\end') depth--
      const row = rows[rows.length - 1]
      if (depth === 0 && t === '&') row.push([])
      else if (depth === 0 && t === '\\\\') rows.push([[]])
      else row[row.length - 1].push(t)
    }
    throw new LatexError(`\\end{${env}} がありません`)
  }

  /**
   * 集合の括弧の中身（`{` の後）が「点 \mid 条件」なら、⦃点∣条件⦄ を出力して true。区切り（\mid・:・\colon）が無ければ、何もせず false。
   * `end` が '\right' なら \right\} まで、null なら \} か \rbrace まで
   */
  private setBuilder(end: '\\right' | null): boolean {
    let depth = 0
    let sep = -1
    let close = -1
    /** 括弧の外にある裸の縦線（| ・\\vert）の位置。縦線を消して打ち直すと \\mid ではなくこれになる */
    const bars: number[] = []
    for (let k = this.i; k < this.toks.length; k++) {
      const t = this.toks[k]
      if (t === '{') depth++
      else if (t === '\\left') {
        depth++
        k++ // 括弧の形（\\left| の | など）は区切りに数えない
      } else if (t === '}' || t === '\\right') {
        if (depth === 0 && end === '\\right' && t === '\\right') {
          close = k
          break
        }
        depth--
        if (t === '\\right') k++
      } else if (depth === 0 && end === null && (t === '\\}' || t === '\\rbrace')) {
        close = k
        break
      } else if (depth === 0 && sep < 0 && (t === '\\mid' || t === ':' || t === '\\colon')) sep = k
      else if (depth === 0 && (t === '|' || t === '\\vert')) bars.push(k)
    }
    // 裸の縦線が 1 本だけ（絶対値の組になっていない）なら、それを区切りとして読む
    if (sep < 0 && bars.length === 1) sep = bars[0]
    if (sep < 0 || close < 0) return false
    const point = new Converter(this.toks.slice(this.i, sep)).run()
    const condToks = this.toks.slice(sep + 1, close)
    const cond = condToks.length === 0 ? '' : new Converter(expandListedRanges(condToks)).run()
    if (point === '' || cond === '') throw new LatexError('集合は {(x, y) | 条件} の形で、点と条件を書いてください')
    this.out += ` ⦃${point}∣${cond}⦄`
    this.i = close + (end === '\\right' ? 2 : 1)
    // 補集合 {…}^{c}・{…}^{\complement}
    if (this.toks[this.i] === '^') {
      const save = this.i
      this.i++
      const sup = this.rawTokens().filter((t) => t !== '\\mathrm' && t !== '\\mathsf' && t !== '{' && t !== '}')
      if (sup.length === 1 && (sup[0] === 'c' || sup[0] === 'C' || sup[0] === '\\complement')) this.out += 'ᶜ'
      else this.i = save
    }
    this.out += ' '
    return true
  }

  /** 引数 1 つ分（{…} か 1 トークン）を、変換せずにトークンのまま返す */
  private rawTokens(): Token[] {
    const t = this.toks[this.i++]
    if (t === undefined) throw new LatexError('式が途中で終わっています')
    if (t !== '{') return [t]
    const out: Token[] = []
    let depth = 1
    while (this.i < this.toks.length) {
      const u = this.toks[this.i++]
      if (u === '{') depth++
      if (u === '}' && --depth === 0) return out
      out.push(u)
    }
    throw new LatexError('括弧が閉じていません')
  }

  /**
   * 積分の中身が、括弧で囲んだ微分形式 \left(P\,dx + Q\,dy\right) だけなら、括弧を外す（∮_C (P dx + Q dy) と書ける）。
   * 括弧の外に d の文字があれば（∫ (x + 1) dx）、何もしない
   */
  private unwrapForm(multi: number) {
    if (this.peek() !== '\\left' || this.toks[this.i + 1] !== '(') return
    try {
      integrandSpan(this.toks, this.i, multi)
      return
    } catch {
      // 括弧の外に d が無い
    }
    let depth = 0
    let r = this.i
    for (; r < this.toks.length; r++) {
      if (this.toks[r] === '\\left') depth++
      else if (this.toks[r] === '\\right' && --depth === 0) break
    }
    if (r >= this.toks.length) return
    const inner = this.toks.slice(this.i + 2, r)
    let span
    try {
      span = integrandSpan(inner, 0, multi)
    } catch {
      return
    }
    let end = span.vars.length === 1 && COORD_LETTERS.has(span.vars[0]) ? formTerms(inner, span.next, span.vars[0]).next : span.next
    while (SPACES.has(inner[end] ?? '')) end++
    if (end !== inner.length) return
    this.toks = [...this.toks.slice(0, this.i), ...inner, ...this.toks.slice(r + 2)]
  }

  /**
   * 積分の中身と、その後の「d + 文字」の並び（dx\,dy など）を読む。中身は、同じ深さで最初に出てくる「d + 文字」の手前まで。
   * 中身の中の積分（入れ子）の「d + 文字」は、その積分のものとして飛ばす（∫∫ … dy dx の内側の dy を、外側の物と取り違えない）
   */
  private integrand(multi: number): { body: string; vars: string[]; vector: boolean } {
    const span = integrandSpan(this.toks, this.i, multi)
    const body = new Converter(this.toks.slice(this.i, span.end)).run()
    if (body === '') throw new LatexError('積分する式を書いてください')
    this.i = span.next
    return { body, vars: span.vars, vector: span.vector }
  }

  /** '{' を読んだ直後から、対応する '}' までを変換して返す */
  private argFromOpenBrace(): string {
    const saved = this.out
    this.out = ''
    this.seq('}')
    this.i++
    const r = this.out
    this.out = saved
    return r
  }
}

/**
 * 分数の形の微分（変換した後の分子 a・分母 b）を、計算エンジンの @diff にする。微分の形でなければ null。
 * d/dx → "@diff(x) "、d²/dx² → "@diff(x,2) "、∂²/∂x∂y → "@diff(x) @diff(y) "、df/dx → "@diff(x) f"
 */
function derivative(a: string, b: string): string | null {
  const num = a.replace(/\s+/g, '').match(/^(d|∂)(?:\^\((\d+)\))?([A-Za-z]\w*)?$/)
  if (!num) return null
  const den = b.replace(/\s+/g, '')
  const parts = [...den.matchAll(/(d|∂)([A-Za-z]\w*?)(?:\^\((\d+)\))?(?=d|∂|$)/g)]
  if (parts.length === 0 || parts.map((m) => m[0]).join('') !== den) return null
  if (parts.some((m) => m[1] !== num[1])) return null
  const order = Number(num[2] ?? 1)
  const total = parts.reduce((s, m) => s + Number(m[3] ?? 1), 0)
  if (order !== total) throw new LatexError('微分の分子と分母の階数をそろえてください')
  const ops = parts.map((m) => (m[3] && m[3] !== '1' ? `@diff(${m[2]},${m[3]}) ` : `@diff(${m[2]}) `)).join('')
  return ` ${ops}${num[3] ?? ''} `
}

/** 間に入ってよい空白の命令 */
const SPACES = new Set(['\\,', '\\;', '\\:', '\\ ', '\\!', '\\quad', '\\qquad'])

/** 積分の d の後に書けるギリシャ文字（極座標の dθ、面積要素の dσ など） */
const DIFF_GREEK = new Set(['\\theta', '\\vartheta', '\\phi', '\\varphi', '\\rho', '\\varrho', '\\sigma', '\\tau'])

/** 積分の d の後の文字を、計算エンジンの名前にする（\\theta → theta、\\varphi → phi） */
const engineVar = (v: string) => (v.startsWith('\\') ? v.slice(1).replace(/^var/, '') : v)

/** 位置 p から「d + 文字 1 つ」（\mathrm{d}x・\differentialD x も）なら、変数と、d の部分のトークンの数（変数の手前まで） */
function differentialAt(toks: Token[], p: number): { v: string; len: number; vector?: boolean } | null {
  const letter = (k: number) => (/^[A-Za-z]$/.test(toks[k] ?? '') || DIFF_GREEK.has(toks[k] ?? '') ? toks[k] : null)
  const t = toks[p]
  // ベクトルの微分 d\mathbf{r}・d\vec{S}（len は文字の手前まで。文字の後の } も読み飛ばすので、文字の位置に 1 足して返す）
  if ((t === 'd' || t === '\\differentialD') && ['\\mathbf', '\\vec', '\\boldsymbol', '\\overrightarrow', '\\mathbb'].includes(toks[p + 1] ?? '') && toks[p + 2] === '{' && letter(p + 3) && toks[p + 4] === '}') {
    return { v: toks[p + 3], len: 4, vector: true }
  }
  if ((t === 'd' || t === '\\differentialD') && letter(p + 1)) return { v: toks[p + 1], len: 1 }
  // \mathrm{d}\mathbf{r}
  if ((t === '\\mathrm' || t === '\\operatorname') && toks[p + 1] === '{' && toks[p + 2] === 'd' && toks[p + 3] === '}') {
    const inner = differentialAt(['d', ...toks.slice(p + 4)], 0)
    if (inner?.vector) return { ...inner, len: inner.len + 3 }
  }
  if ((t === '\\mathrm' || t === '\\operatorname') && toks[p + 1] === '{' && toks[p + 2] === 'd' && toks[p + 3] === '}' && letter(p + 4)) {
    return { v: toks[p + 4], len: 4 }
  }
  return null
}

/**
 * 中身の終わり（end の手前）が「\\cdot 単位ベクトル」（\\mathbf{T}・\\vec{n}・\\hat{n}・\\boldsymbol{N} など）なら、その文字と \\cdot の位置
 */
function unitVectorBefore(toks: Token[], start: number, end: number): { letter: string; at: number } | null {
  const wraps = ['\\mathbf', '\\vec', '\\hat', '\\boldsymbol', '\\mathbb', '\\overrightarrow']
  let k = end
  let letter: string | null = null
  if (toks[k - 1] === '}' && /^[A-Za-z]$/.test(toks[k - 2] ?? '') && toks[k - 3] === '{') {
    // \hat{\mathbf{n}} も
    if (wraps.includes(toks[k - 4])) {
      letter = toks[k - 2]
      k -= 4
    }
  } else if (/* \\hat{\\mathbf{n}} */ toks[k - 1] === '}' && toks[k - 2] === '}' && /^[A-Za-z]$/.test(toks[k - 3] ?? '') && toks[k - 4] === '{' && wraps.includes(toks[k - 5]) && toks[k - 6] === '{' && wraps.includes(toks[k - 7])) {
    letter = toks[k - 3]
    k -= 7
  }
  if (letter === null) return null
  while (k > start && SPACES.has(toks[k - 1])) k--
  if (toks[k - 1] !== '\\cdot' || k - 1 <= start) return null
  return { letter, at: k - 1 }
}

/**
 * 積分の中身の範囲。toks[start..end) が中身、vars はその後の「d + 文字」の変数、next は読み終わった位置。
 * 中身は、同じ深さで最初に出てくる「d + 文字」の手前まで。中身の中の積分（入れ子）の「d + 文字」は、その積分のものとして飛ばす
 */
function integrandSpan(toks: Token[], start: number, multi: number): { end: number; vars: string[]; vector: boolean; next: number } {
  let depth = 0
  let pending = 0
  for (let p = start; p < toks.length; p++) {
    const t2 = toks[p]
    if (t2 === '{' || t2 === '\\left') depth++
    else if (t2 === '}' || t2 === '\\right') {
      if (depth === 0) break
      depth--
    }
    if (depth !== 0) continue
    if (t2 === '\\int' || t2 === '\\intop' || t2 === '\\smallint' || t2 === '\\oint') pending += 1
    else if (t2 === '\\iint' || t2 === '\\oiint') pending += 2
    else if (t2 === '\\iiint') pending += 3
    const d = differentialAt(toks, p)
    if (!d) continue
    if (pending > 0) {
      pending--
      p += d.len
      continue
    }
    // F · dr の形なら、中身の最後の · は内積の印（中身に入れない）
    let end = p
    while (end > start && SPACES.has(toks[end - 1])) end--
    // F · T ds（単位接線）は F · dr、F · n dS（単位法線）は F · dS
    if (!d.vector && pending === 0) {
      const unit = unitVectorBefore(toks, start, end)
      if (unit && ((d.v === 's' && 'Tt'.includes(unit.letter)) || (['S', 'A', '\\sigma'].includes(d.v) && 'nN'.includes(unit.letter)))) {
        return { end: unit.at, vars: [d.v === 's' ? 'r' : 'S'], vector: true, next: p + d.len + 1 }
      }
    }
    if (d.vector) {
      if (toks[end - 1] !== '\\cdot') throw new LatexError('ベクトル場の積分は F \\cdot d\\mathbf{r} の形で書いてください')
      return { end: end - 1, vars: [d.v], vector: true, next: p + d.len + 1 }
    }
    // 続く「d + 文字」をまとめて読む（間の \, などの空白は飛ばす）
    const vars: string[] = []
    let q = p
    for (;;) {
      while (SPACES.has(toks[q] ?? '')) q++
      const e = differentialAt(toks, q)
      if (!e || vars.length >= Math.max(multi, 3)) break
      vars.push(e.v)
      q += e.len + 1
    }
    return { end, vars, vector: false, next: q }
  }
  throw new LatexError('積分の変数を dx のように書いてください')
}

/**
 * 積分する領域（∬ の下）を、計算エンジンの @intset の引数「(x,y),条件,…」にする。
 * 集合 \{(x,y) \mid 条件\}（\mid・|・: で区切る）と、区間の直積 [0,1]\times[0,2]（変数は dx\,dy の順）を読む
 */
function regionOf(toks: Token[], vars: string[]): string {
  // 名前で書いた範囲（∬_D の D）は、その範囲の欄の中身に置き換える
  const name = regionNameOf(toks)
  if (name !== null) {
    const def = (REGIONS[name] ?? '').trim()
    if (def === '' || def.includes('\\placeholder')) throw new LatexError(`${regionLabel(name)} の範囲を、下の欄に書いてください（例: {(x, y) | x² + y² < 1}、曲線なら {(cos t, sin t) | 0 ≤ t ≤ 2π}）`)
    toks = tokenize(def)
  }
  // 集合の括弧を外す
  let inner = toks
  const strip = (open: Token[], close: Token[]) => {
    if (open.every((t, k) => inner[k] === t) && close.every((t, k) => inner[inner.length - close.length + k] === t)) {
      inner = inner.slice(open.length, inner.length - close.length)
      return true
    }
    return false
  }
  const isSet = strip(['\\left', '\\{'], ['\\right', '\\}']) || strip(['\\{'], ['\\}']) || strip(['\\lbrace'], ['\\rbrace'])
  if (isSet) {
    let depth = 0
    for (let k = 0; k < inner.length; k++) {
      const t = inner[k]
      if (t === '{' || t === '\\left') depth++
      else if (t === '}' || t === '\\right') depth--
      if (depth === 0 && (t === '\\mid' || t === '|' || t === ':' || t === '\\vert')) {
        const point = new Converter(inner.slice(0, k)).run()
        const condToks = inner.slice(k + 1)
        const cond = condToks.length === 0 ? '' : new Converter(expandListedRanges(condToks)).run()
        if (point === '' || cond === '') break
        return `${point},${cond}`
      }
    }
    throw new LatexError('積分する領域の集合は {(x,y) | 条件} の形で書いてください')
  }
  // 区間の直積 [a,b]×[c,d]
  const text = new Converter(toks).run()
  const boxes = text.split(/[*×]/).map((p) => p.trim())
  const ranges = boxes.map((b) => b.match(/^\[(.+),(.+)\]$/))
  if (ranges.every((m) => m) && ranges.length > 0) {
    // dA・dV（大文字 1 つ）なら、変数は x, y, z
    const names = vars.length === 1 && /^[A-Z]$|^sigma$/.test(vars[0]) ? ['x', 'y', 'z'].slice(0, ranges.length) : vars
    if (ranges.length !== names.length) throw new LatexError(`区間が ${ranges.length} 個あるので、dx\\,dy のように変数を ${ranges.length} 個書いてください`)
    const conds = ranges.map((m, k) => `${m![1].trim()}<=${names[k]}<=${m![2].trim()}`)
    const point = names.length === 1 ? names[0] : `(${names.join(',')})`
    return `${point},${conds.join(',')}`
  }
  throw new LatexError('積分する領域は、{(x,y) | 条件} か [0,1]×[0,2] の形で書いてください')
}

/**
 * 曲線の上の P dx に続く「± Q dy」「± R dz」（座標の d が 1 つずつで、重ならないもの）。toks[start] から読み、
 * 項ごとの符号・中身の範囲・d の文字と、読み終わった位置を返す（続かなければ terms は空、next は start）
 */
function formTerms(toks: Token[], start: number, first: string): { terms: { sign: string; from: number; end: number; next: number; v: string }[]; next: number } {
  const used = new Set([first])
  const terms: { sign: string; from: number; end: number; next: number; v: string }[] = []
  let next = start
  for (;;) {
    let k = next
    while (SPACES.has(toks[k] ?? '')) k++
    const sign = toks[k]
    if (sign !== '+' && sign !== '-') break
    let span
    try {
      span = integrandSpan(toks, k + 1, 1)
    } catch {
      break
    }
    if (span.vector || span.vars.length !== 1 || !COORD_LETTERS.has(span.vars[0]) || used.has(span.vars[0]) || span.end <= k + 1) break
    if (toks.slice(k + 1, span.end).some((t) => INTEGRALS.has(t))) break // 別の積分の項
    used.add(span.vars[0])
    terms.push({ sign, from: k + 1, end: span.end, next: span.next, v: span.vars[0] })
    next = span.next
  }
  return { terms, next }
}

/**
 * 式の中の、積分記号の下に直接書いた範囲（集合・区間の直積。名前で書いた範囲と、上下の端のある積分は除く）。出てきた順
 */
export function inlineRegions(latex: string): string[] {
  const toks = tokenize(latex)
  const out: string[] = []
  for (let i = 0; i < toks.length; i++) {
    if (!INTEGRALS.has(toks[i])) continue
    const { sub, sup, k } = limitsAt(toks, i)
    if (sub === null || sup !== null || sub.length === 0 || regionNameOf(sub) !== null) continue
    // 条件だけの書き方（∬_{x² + y² ≤ 1}）は、d の文字と積分記号の数から点を決めた集合にする
    const multi = toks[i] === '\\iint' || toks[i] === '\\oiint' ? 2 : toks[i] === '\\iiint' ? 3 : 1
    let vars: string[] = []
    try {
      vars = integrandSpan(toks, k, multi).vars
    } catch {
      // d が無い（書いている途中）
    }
    out.push(joinToks(regionTokens(sub, vars, multi)))
  }
  return out
}

/** 曲線の上の dx・dy・dz として読む文字 */
const COORD_LETTERS = new Set(['x', 'y', 'z'])

/** 積分記号の下が、集合か範囲の名前（区間の直積 [0,1]×[0,2] ではない）か。P dx + Q dy を曲線の上の積分として読むのはこのときだけ */
function isCurveRegion(sub: Token[]): boolean {
  if (regionNameOf(sub) !== null) return true
  return sub[0] === '\\{' || (sub[0] === '\\left' && sub[1] === '\\{')
}

// ---- 名前で書いた積分の範囲（∬_D f dA の D）----
// 積分記号の下に範囲そのものを書くと、小さくて読みにくく、式も長くなる。下に 1 文字（D・C・V・Ω など）だけを書き、
// 範囲は式の下の欄に書く。同じ名前はどの式でも同じ範囲を指す

/** 範囲の名前（積分記号の下の文字。"D"・"\\Omega" など）→ 範囲の LaTeX（{(x,y) | 条件} か [0,1]×[0,2]） */
let REGIONS: Record<string, string> = {}

/** 範囲の欄の中身を知らせる（latexToText が積分の範囲を置き換えるのに使う） */
export function setRegions(regions: Record<string, string>) {
  REGIONS = regions
  textCache.clear() // 範囲の中身で変換の結果が変わるので、覚えた結果を捨てる
}

/**
 * 変換した結果（変換できなかったときは、その LatexError）。同じ LaTeX を何度も変換するので覚えておく
 * （再生中・計算し直しのたびに、全部の行の式を変換していた）。結果は LaTeX と範囲の中身だけで決まる
 */
const textCache = new Map<string, string | LatexError>()

const GREEK: Record<string, string> = {
  '\\Gamma': 'Γ', '\\Delta': 'Δ', '\\Theta': 'Θ', '\\Lambda': 'Λ', '\\Xi': 'Ξ', '\\Pi': 'Π', '\\Sigma': 'Σ', '\\Phi': 'Φ', '\\Psi': 'Ψ', '\\Omega': 'Ω',
  '\\alpha': 'α', '\\beta': 'β', '\\gamma': 'γ', '\\delta': 'δ', '\\sigma': 'σ', '\\omega': 'ω', '\\partial': '∂',
}

/** 積分記号の下が範囲の名前（1 文字）なら、その名前。そうでなければ null */
function regionNameOf(sub: Token[]): string | null {
  const head = (t: Token | undefined) => t !== undefined && (/^[A-Za-z]$/.test(t) || t in GREEK)
  if (sub.length === 1) return head(sub[0]) ? sub[0] : null
  // 添字の付いた名前 C_1・D_{12}・C_{a}（"C_1"・"D_12" を名前にする）
  if (!head(sub[0]) || sub[1] !== '_') return null
  const idx = sub[2] === '{' && sub[sub.length - 1] === '}' ? sub.slice(3, -1) : sub.slice(2)
  if (idx.length === 0 || !idx.every((t) => /^[A-Za-z0-9]$/.test(t)) || (sub[2] !== '{' && idx.length !== 1)) return null
  return `${sub[0]}_${idx.join('')}`
}

/** 範囲の名前の表示（"\\Omega" → "Ω"、"C_1" → "C₁"） */
export function regionLabel(name: string): string {
  const [head, idx] = name.split('_')
  const sub = idx === undefined ? '' : /^\d+$/.test(idx) ? [...idx].map((d) => '₀₁₂₃₄₅₆₇₈₉'[Number(d)]).join('') : `_${idx}`
  return (GREEK[head] ?? head) + sub
}

/**
 * 積分記号の下（名前なら範囲の欄の中身）を、集合か区間の直積の形のトークンにする。
 * 条件だけの書き方 ∬_{x² + y² ≤ 1}（よく使う略記）は、点を d の文字（dx dy）か積分記号の数（∬ なら (x, y)）にした集合にする
 */
function regionTokens(sub: Token[], vars: string[], multi: number): Token[] {
  let toks = sub
  const name = regionNameOf(sub)
  if (name !== null) {
    const def = (REGIONS[name] ?? '').trim()
    if (def === '' || def.includes('\\placeholder')) throw new LatexError(`${regionLabel(name)} の範囲を、下の欄に書いてください（例: {(x, y) | x² + y² < 1}、曲線なら {(cos t, sin t) | 0 ≤ t ≤ 2π}）`)
    toks = tokenize(def)
  }
  const isSet = toks[0] === '\\{' || (toks[0] === '\\left' && toks[1] === '\\{')
  const hasRel = splitTop(toks, (t) => t === ',').some((p) => p.some((t) => RELATIONS.has(t) || t === '=' || t === '\\in'))
  if (!isSet && hasRel && !toks.some((t) => SET_OPS.has(t))) {
    const coords = vars.length > 0 && vars.every((v) => /^[a-z]$/.test(v)) && !vars.includes('s') ? vars : null
    const n = vars.length === 1 && vars[0] === 'A' ? 2 : vars.length === 1 && vars[0] === 'V' ? 3 : Math.max(multi, coords?.length ?? 0)
    const names = coords ?? ['x', 'y', 'z'].slice(0, n)
    const point = names.flatMap((v, k) => (k === 0 ? [v] : [',', v]))
    return ['\\left', '\\{', '\\left', '(', ...point, '\\right', ')', '\\mid', ...toks, '\\right', '\\}']
  }
  return toks
}

/** 集合の演算の記号（積分の範囲・範囲の図で、集合どうしをつなぐもの） */
const SET_OPS = new Set(['\\cup', '\\cap', '\\setminus'])

/** 集合の括弧の中の点の部分（{(x, y) | …} の (x, y)）。集合でなければ null */
function setPoint(toks: Token[]): Token[] | null {
  const open = toks[0] === '\\left' && toks[1] === '\\{' ? 2 : toks[0] === '\\{' ? 1 : 0
  if (open === 0) return null
  const parts = splitTop(toks.slice(open, toks.length - open), (t) => t === '\\mid' || t === '|' || t === ':' || t === '\\vert' || t === '\\colon')
  return parts.length >= 2 ? parts[0] : null
}

/** トークンの並びを、集合の演算の記号で分けたものと、その間の記号 */
function splitSetOps(toks: Token[]): { parts: Token[][]; ops: Token[] } {
  const parts = splitTop(toks, (t) => SET_OPS.has(t))
  const ops: Token[] = []
  let depth = 0
  for (let k = 0; k < toks.length; k++) {
    const t = toks[k]
    if (t === '\\left' || t === '\\right') {
      depth += t === '\\left' ? 1 : -1
      k++
      continue
    }
    if (t === '{' || t === '(' || t === '[') depth++
    else if (t === '}' || t === ')' || t === ']') depth--
    if (depth === 0 && SET_OPS.has(t)) ops.push(t)
  }
  return { parts, ops }
}

/**
 * 積分の範囲（積分記号の下）を、計算エンジンの @intset の範囲の引数の並びにする（足し合わせる範囲ごとに 1 つ）。
 * - 集合どうしの ∩・∪・∖（点が同じ変数の組）は、条件「(x, y) ∈ A ∩ B」の 1 つの範囲
 * - 媒介変数で書いた曲線・曲面どうしの ∪（折れ線の経路など）は、それぞれの範囲の積分の和
 */
function regionsOf(sub: Token[], vars: string[], multi: number): string[] {
  const toks = regionTokens(sub, vars, multi)
  const { parts, ops } = splitSetOps(toks)
  if (parts.length === 1) return [regionOf(toks, vars)]
  const points = parts.map(setPoint)
  if (points.some((p) => p === null)) throw new LatexError('∪・∩・∖ でつなぐ積分の範囲は、どれも {(x, y) | 条件} の形の集合で書いてください')
  const comps = points.map((p) => splitTop(stripParens(p!), (t) => t === ','))
  const isPlain = comps.every((cs) => cs.every((c) => c.length === 1 && /^[A-Za-z]$/.test(c[0])))
  const key = (cs: Token[][]) => cs.map((c) => c[0]).join(',')
  if (isPlain && comps.every((cs) => key(cs) === key(comps[0]))) {
    const names = comps[0].map((c) => c[0])
    const point = names.length === 1 ? names[0] : `(${names.join(',')})`
    return [`${point},${point}∈${new Converter(toks).run()}`]
  }
  if (ops.every((t) => t === '\\cup')) return parts.map((p) => regionOf(p, vars))
  throw new LatexError('曲線・曲面（媒介変数で書いた集合）どうしは ∪ でだけつなげます（∩・∖ は、点を (x, y) と書いた集合どうしで使えます）')
}

const INTEGRALS = new Set(['\\int', '\\intop', '\\smallint', '\\oint', '\\iint', '\\oiint', '\\iiint'])

/**
 * 式の中の、名前で書いた積分の範囲（上端の無い積分記号の下の 1 文字）。出てきた順で、同じ名前は 1 回だけ。
 * template は、その範囲の欄に最初に入れておく雛形（積分の種類から決める）
 */
export function regionsUsed(latex: string): { name: string; template: string }[] {
  const toks = tokenize(latex)
  const out: { name: string; template: string }[] = []
  /** 位置 k から引数 1 つ分（{…} か 1 トークン）を読み、中身と次の位置を返す */
  const group = (k: number): [Token[], number] => {
    if (toks[k] !== '{') return [toks[k] === undefined ? [] : [toks[k]], k + 1]
    let depth = 1
    let j = k + 1
    for (; j < toks.length && depth > 0; j++) {
      if (toks[j] === '{') depth++
      else if (toks[j] === '}') depth--
    }
    return [toks.slice(k + 1, j - 1), j]
  }
  for (let i = 0; i < toks.length; i++) {
    if (!INTEGRALS.has(toks[i])) continue
    let k = i + 1
    let sub: Token[] | null = null
    let upper = false
    while (toks[k] === '_' || toks[k] === '^' || toks[k] === '\\limits' || toks[k] === '\\nolimits') {
      const t = toks[k++]
      if (t === '_') [sub, k] = group(k)
      else if (t === '^') {
        upper = true
        k = group(k)[1]
      }
    }
    const name = sub && !upper ? regionNameOf(sub) : null
    if (name === null || out.some((r) => r.name === name)) continue
    // 雛形: ∬ … dA は平面の領域、∭ は空間の領域、それ以外（線積分・面積分）は媒介変数で書いた曲線・曲面
    let end = toks.length
    for (let j = k; j < toks.length; j++) {
      if (INTEGRALS.has(toks[j])) {
        end = j
        break
      }
    }
    const rest = toks.slice(k, end).join('')
    const point =
      toks[i] === '\\iiint' ? '\\left(x,y,z\\right)' : (toks[i] === '\\iint' || toks[i] === '\\oiint') && !/d\\mathbf\{S\}|dS/.test(rest) ? '\\left(x,y\\right)' : '\\left(\\placeholder{}\\right)'
    out.push({ name, template: `\\left\\{${point}\\mid\\placeholder{}\\right\\}` })
  }
  return out
}

/** 媒介変数の名前になりうるギリシャ文字 */
const PARAM_GREEK = new Set(['\\theta', '\\vartheta', '\\varphi', '\\phi', '\\rho', '\\sigma', '\\tau', '\\alpha', '\\beta', '\\psi', '\\omega', '\\lambda', '\\mu', '\\nu'])

/**
 * 積分の範囲（範囲の欄の中身）を、グラフに描くための式（LaTeX）にする。描ける形でなければ null。
 * - 平面の領域 {(x, y) | 条件}・[0,1]×[0,2] は、3 次元でも柱にならないよう、高さ 0 の面 {(x, y, z) | 条件, z = 0} にする
 * - 点を式で書いた集合 {(r cos θ, r sin θ) | 0 ≤ r ≤ 1, …} は、媒介変数の名前を計算エンジンが読める t・u・v に付け替える
 */
export function regionDisplayLatex(def: string): string | null {
  let toks = tokenize(def).filter((t) => t !== ' ')
  if (toks.length === 0 || toks.includes('\\placeholder')) return null
  // 集合どうしの ∪・∩・∖ は、それぞれを描く式にしてつなぐ
  const { parts, ops } = splitSetOps(toks)
  if (parts.length > 1) {
    const shown = parts.map((p) => regionDisplayLatex(joinToks(p)))
    if (shown.some((x) => x === null)) return null
    return shown.reduce((a, b, k) => `${a}${ops[k - 1]} ${b}`)!
  }
  const text = (ts: Token[]) => ts.map((t) => (/^\\[A-Za-z]+$/.test(t) ? `${t} ` : t)).join('')
  const strip = (open: Token[], close: Token[]) => {
    if (open.every((t, k) => toks[k] === t) && close.every((t, k) => toks[toks.length - close.length + k] === t)) {
      toks = toks.slice(open.length, toks.length - close.length)
      return true
    }
    return false
  }
  const set = (point: string, cond: string) => `\\left\\{\\left(${point}\\right)\\mid ${cond}\\right\\}`
  if (!(strip(['\\left', '\\{'], ['\\right', '\\}']) || strip(['\\{'], ['\\}']) || strip(['\\lbrace'], ['\\rbrace']))) {
    // 区間の直積 [a,b]×[c,d]: 座標は x, y, z の順
    const boxes: Token[][] = [[]]
    let depth = 0
    for (let k = 0; k < toks.length; k++) {
      const t = toks[k]
      if (t === '\\left') depth++
      else if (t === '\\right') depth--
      if (depth === 0 && t === '\\times') boxes.push([])
      else boxes[boxes.length - 1].push(t)
    }
    const conds: string[] = []
    for (const [k, box] of boxes.entries()) {
      const inner = box[0] === '\\left' ? box.slice(2, -2) : box.slice(1, -1)
      const open = box[0] === '\\left' ? box[1] : box[0]
      const comma = inner.indexOf(',')
      if (open !== '[' || comma < 1 || comma === inner.length - 1 || k > 2) return null
      conds.push(`${text(inner.slice(0, comma))}\\le ${'xyz'[k]}\\le ${text(inner.slice(comma + 1))}`)
    }
    if (conds.length === 1) return null
    return conds.length === 2 ? set('x,y,z', `${conds.join(',')},z=0`) : set('x,y,z', conds.join(','))
  }
  // 点と条件に分ける
  let sep = -1
  let depth = 0
  for (let k = 0; k < toks.length && sep < 0; k++) {
    const t = toks[k]
    if (t === '{' || t === '\\left') depth++
    else if (t === '}' || t === '\\right') depth--
    else if (depth === 0 && (t === '\\mid' || t === '|' || t === ':' || t === '\\vert' || t === '\\colon')) sep = k
  }
  if (sep < 1 || sep === toks.length - 1) return null
  let point = toks.slice(0, sep)
  const cond = expandListedRanges(toks.slice(sep + 1))
  if (point[0] === '\\left' && point[1] === '(' && point[point.length - 2] === '\\right') point = point.slice(2, -2)
  else if (point[0] === '(' && point[point.length - 1] === ')') point = point.slice(1, -1)
  const names = text(point).split(',').map((n) => n.trim())
  if (names.every((n) => /^[A-Za-z]$/.test(n))) {
    // 変数の組: 2 つなら、3 つ目の座標 = 0 の面にする
    if (names.length !== 2) return set(names.join(','), text(cond))
    const third = ['z', 'w', 'c'].find((n) => !names.includes(n) && !cond.includes(n))
    return third ? set(`${names.join(',')},${third}`, `${text(cond)},${third}=0`) : set(names.join(','), text(cond))
  }
  // 点を式で書いた集合: 条件に出てくる文字（媒介変数）を t・u・v に付け替える
  const isName = (t: Token) => (/^[A-Za-z]$/.test(t) && t !== 'e' && t !== 'i' && t !== 'd') || PARAM_GREEK.has(t)
  const params = [...new Set(cond.filter(isName))].filter((t) => point.includes(t))
  if (params.length === 0 || params.length > 3) return null
  const targets = params.length === 1 ? ['t'] : ['u', 'v', 't']
  const free = targets.filter((n) => !params.includes(n))
  const rename = new Map<Token, Token>()
  for (const p of params) {
    if (!targets.includes(p)) rename.set(p, free.shift()!)
  }
  const re = (ts: Token[]) => text(ts.map((t) => rename.get(t) ?? t))
  return set(re(point), re(cond))
}

/** 不等号のトークン */
const RELATIONS = new Set(['<', '>', '\\lt', '\\gt', '\\le', '\\leq', '\\ge', '\\geq', '\\leqslant', '\\geqslant', '\\leqq', '\\geqq'])

/** 小さい方から書く不等号（≤・<）か */
const LESS = new Set(['<', '\\lt', '\\le', '\\leq', '\\leqslant', '\\leqq'])

/**
 * 条件の並びの省略形「a ≤ x, y ≤ b」「a ≤ x, y, z ≤ b」（どの文字も a 以上 b 以下）を、「a ≤ x ≤ b, a ≤ y ≤ b, …」に直す。
 * 先頭の条件が「… 不等号 文字」、最後の条件が「文字 不等号 …」で、不等号の向きがそろっていて、文字がどれも違い、
 * 両端（a・b）に並べた文字も座標（x, y, z, w）も出てこないときだけ直す（「x ≤ y, z ≤ 1」は x ≤ y と z ≤ 1 のまま）
 */
export function expandListedRanges(cond: Token[]): Token[] {
  const items = splitTop(cond, (t) => t === ',')
  const letter = (t: Token | undefined) => t !== undefined && /^[A-Za-z]$/.test(t)
  const isVar = (it: Token[]) => it.length === 1 && letter(it[0])
  /** 並べた文字が全部違い、端（ends）に並べた文字も座標も出てこない */
  const ok = (vars: Token[], ends: Token[]) => {
    const blocked = new Set([...vars, 'x', 'y', 'z', 'w'])
    return new Set(vars).size === vars.length && !ends.some((t) => blocked.has(t))
  }
  const out: Token[][] = []
  for (let k = 0; k < items.length; k++) {
    const first = items[k]
    // 先頭: 「a ≤ x」（最後の不等号の後が文字 1 つ）
    const relAt = first.findLastIndex((t) => RELATIONS.has(t))
    let j = k + 1
    while (j < items.length && isVar(items[j])) j++
    const last = items[j]
    const lastIsUpper = !!last && last.length > 2 && letter(last[0]) && RELATIONS.has(last[1])
    if (relAt > 0 && relAt === first.length - 2 && letter(first[relAt + 1])) {
      const lo = first.slice(0, relAt + 1)
      // 両側 a ≤ x, y ≤ b
      if (lastIsUpper) {
        const hi = last.slice(1)
        const vars = [first[relAt + 1], ...items.slice(k + 1, j).map((it) => it[0]), last[0]]
        if (LESS.has(first[relAt]) === LESS.has(last[1]) && ok(vars, [...lo, ...hi])) {
          for (const v of vars) out.push([...lo, v, ...hi])
          k = j
          continue
        }
      }
      // 片側 0 ≤ x, y（文字だけの条件は、それだけでは意味が無いので、前の条件の続き）
      const vars = [first[relAt + 1], ...items.slice(k + 1, j).map((it) => it[0])]
      if (j > k + 1 && ok(vars, lo)) {
        for (const v of vars) out.push([...lo, v])
        k = j - 1
        continue
      }
    }
    // 片側 x, y, z ≥ 0（先頭から文字だけの条件が続き、最後が「文字 不等号 …」）
    if (isVar(first) && lastIsUpper) {
      const hi = last.slice(1)
      const vars = [...items.slice(k, j).map((it) => it[0]), last[0]]
      if (!hi.slice(1).some((t) => RELATIONS.has(t)) && ok(vars, hi)) {
        for (const v of vars) out.push([v, ...hi])
        k = j
        continue
      }
    }
    out.push(first)
  }
  return out.reduce((a, b) => [...a, ',', ...b])
}

// ---- 積分を描く（被積分関数を、積分する範囲の上に描く）----
// 計算エンジンには手を入れず、積分の式から「描くための式」（LaTeX）を作って、ふつうの式と同じ道で描く

/** 積分を描くための式 */
export interface IntegralView {
  /** 描くための式（LaTeX） */
  latex: string
  /** 何を描いているかの説明 */
  note: string
  /** 3 重積分: 被積分関数の値を、領域の中の色（伝達関数）で描く */
  field?: boolean
  /** 進み具合のスライダーで動かす積分の変数（範囲を「下の端 〜 途中」に縮める）。動かせない形なら無い */
  sweep?: string
  /** 進み具合までの積分（値を出すための LaTeX）。sweep が無ければ無い */
  partial?: string
  /** 一緒に描くもの（ベクトル場の矢印・今の点・接線・法線・場の矢印）。latex の t は 0〜1 の矢印の媒介変数 */
  extras?: IntegralExtra[]
}

/** 積分の図に添える図形 */
export interface IntegralExtra {
  /** path: 積分する経路・領域そのもの（範囲全体）、field: ベクトル場の矢印、point: 今の点、tangent: 接線（単位）、normal: 法線（単位）、vector: 場の向き、component: 場の接線・法線の成分、riemann: リーマン和などの近似の図形 */
  role: 'path' | 'field' | 'point' | 'tangent' | 'normal' | 'vector' | 'component' | 'riemann'
  latex: string
  /** 説明（式の下に色と一緒に出す） */
  label: string
}

/** 式に積分記号があるか（「積分を描く」のボタンを出すかどうか） */
export function hasIntegral(latex: string): boolean {
  return /\\(?:o?i{1,3}nt|intop|smallint)(?![A-Za-z])/.test(latex)
}

/** トークンを LaTeX に戻す（コマンドの後に空白を入れる） */
const joinToks = (ts: Token[]) => ts.map((t) => (/^\\[A-Za-z]+$/.test(t) ? `${t} ` : t)).join('')

/** 同じ深さの区切り sep で分ける（括弧・{ }・\left \right の中では分けない） */
function splitTop(toks: Token[], sep: (t: Token) => boolean): Token[][] {
  const out: Token[][] = [[]]
  let depth = 0
  for (let k = 0; k < toks.length; k++) {
    const t = toks[k]
    if (t === '\\left' || t === '\\right') {
      depth += t === '\\left' ? 1 : -1
      out[out.length - 1].push(t)
      if (k + 1 < toks.length) out[out.length - 1].push(toks[++k]) // 括弧そのものは深さに数えない
      continue
    }
    if (t === '{' || t === '(' || t === '[') depth++
    else if (t === '}' || t === ')' || t === ']') depth--
    if (depth === 0 && sep(t)) out.push([])
    else out[out.length - 1].push(t)
  }
  return out
}

/** 外側の括弧 \left( … \right)・( … ) を 1 組外す */
function stripParens(toks: Token[]): Token[] {
  const pairs: [Token[], Token[]][] = [
    [['\\left', '('], ['\\right', ')']],
    [['('], [')']],
  ]
  for (const [open, close] of pairs) {
    if (toks.length < open.length + close.length) continue
    if (!open.every((t, k) => toks[k] === t) || !close.every((t, k) => toks[toks.length - close.length + k] === t)) continue
    // 先頭の括弧が最後の括弧と対になっているか（(a)+(b) を外さない）
    let depth = 0
    let ok = true
    for (let k = 0; k < toks.length - 1 && ok; k++) {
      if (toks[k] === '(') depth++
      else if (toks[k] === ')') depth--
      if (depth === 0 && k >= open.length - 1) ok = false
    }
    if (ok) return toks.slice(open.length, toks.length - close.length)
  }
  return toks
}

/** 積分記号の後の下付き・上付き（無ければ null）と、その後の位置 */
function limitsAt(toks: Token[], i: number): { sub: Token[] | null; sup: Token[] | null; k: number } {
  const group = (k: number): [Token[], number] => {
    if (toks[k] !== '{') return [toks[k] === undefined ? [] : [toks[k]], k + 1]
    let depth = 1
    let j = k + 1
    for (; j < toks.length && depth > 0; j++) {
      if (toks[j] === '{') depth++
      else if (toks[j] === '}') depth--
    }
    return [toks.slice(k + 1, j - 1), j]
  }
  let k = i + 1
  let sub: Token[] | null = null
  let sup: Token[] | null = null
  while (toks[k] === '_' || toks[k] === '^' || toks[k] === '\\limits' || toks[k] === '\\nolimits') {
    const t = toks[k++]
    if (t === '_') [sub, k] = group(k)
    else if (t === '^') [sup, k] = group(k)
  }
  return { sub, sup, k }
}

/** 積分する範囲: 変数の組と条件（{(x, y) | 条件}）か、点を式で書いた集合（{(r cos θ, r sin θ) | 範囲}） */
type IntegralRegion = { names: string[]; cond: Token[] } | { comps: Token[][]; params: Token[]; cond: Token[]; label?: Map<Token, Token> }

/** 積分記号の下の範囲（名前なら範囲の欄の中身）を読む。vars は dx\,dy の変数（区間の直積の変数の名前に使う） */
function integralRegion(sub: Token[], vars: string[], multi = 1): IntegralRegion {
  const toks = regionTokens(sub, vars, multi)
  if (splitSetOps(toks).parts.length > 1) throw new LatexError('∪・∩・∖ でつないだ範囲の積分は、まだ図にできません（値は計算します。「範囲を表示」で範囲は描けます）')
  const fenced = (open: Token[], close: Token[]) => open.every((t, k) => toks[k] === t) && close.every((t, k) => toks[toks.length - close.length + k] === t)
  const set = fenced(['\\left', '\\{'], ['\\right', '\\}']) ? 2 : fenced(['\\{'], ['\\}']) || fenced(['\\lbrace'], ['\\rbrace']) ? 1 : 0
  if (set === 0) {
    // 区間の直積 [a,b]×[c,d]。変数は dx\,dy の順（dA・dV なら x, y, z）
    const boxes = splitTop(toks, (t) => t === '\\times')
    const names = vars.length === boxes.length && vars.every((v) => /^[a-z]$/.test(v)) ? vars : ['x', 'y', 'z'].slice(0, boxes.length)
    const cond: Token[] = []
    for (const [k, box] of boxes.entries()) {
      const inner = box[0] === '\\left' ? box.slice(2, -2) : box.slice(1, -1)
      const ends = splitTop(inner, (t) => t === ',')
      if ((box[0] === '\\left' ? box[1] : box[0]) !== '[' || ends.length !== 2 || ends.some((e) => e.length === 0) || k > 2) {
        throw new LatexError('積分する範囲は、{(x,y) | 条件} か [0,1]×[0,2] の形で書いてください')
      }
      if (k > 0) cond.push(',')
      cond.push(...ends[0], '\\le', names[k], '\\le', ...ends[1])
    }
    return { names, cond }
  }
  const inner = toks.slice(set, toks.length - set)
  const parts = splitTop(inner, (t) => t === '\\mid' || t === '|' || t === ':' || t === '\\vert' || t === '\\colon')
  if (parts.length < 2 || parts[0].length === 0 || parts[1].length === 0) throw new LatexError('積分する範囲の集合は {(x,y) | 条件} の形で書いてください')
  // 2 本目より後の | は、条件の中の絶対値。そのまま戻す
  const cond = expandListedRanges(parts.slice(1).reduce((a, b) => [...a, '|', ...b]))
  const comps = splitTop(stripParens(parts[0]), (t) => t === ',')
  if (comps.every((c) => c.length === 1 && /^[A-Za-z]$/.test(c[0]))) {
    const names = comps.map((c) => c[0])
    // 等式の条件（y = x²・x = y = z）があれば、解いて曲線・曲面の点の式にする
    return splitTop(cond, (t) => t === ',').some((p) => p.includes('=')) ? solveEqualities(names, cond) : { names, cond }
  }
  // 媒介変数は、条件に出てくる文字のうち、点の式にも出てくるもの（点の式に出てくる順）
  const isName = (t: Token) => (/^[A-Za-z]$/.test(t) && t !== 'e' && t !== 'i' && t !== 'd') || PARAM_GREEK.has(t)
  const inCond = new Set(cond.filter(isName))
  const params = [...new Set(comps.flat().filter((t) => inCond.has(t)))]
  return renameCoordParams({ comps, params, cond })
}

/**
 * 変数の組の範囲 {(x, y, z) | 条件} の等式（`y = x^2`・`x = y = z`）を、片方が名前 1 つのものだけ解いて、媒介変数で書いた範囲にする
 * （計算エンジンの解き方より狭い。図にするためだけ）。解けなければ、描けない理由の LatexError
 */
function solveEqualities(names: string[], cond: Token[]): IntegralRegion {
  const solved = new Map<string, Token[]>()
  const put = (ts: Token[]): Token[] => ts.flatMap((t) => (solved.has(t) ? ['\\left', '(', ...solved.get(t)!, '\\right', ')'] : [t]))
  const rest: Token[][] = []
  for (const part of splitTop(cond, (t) => t === ',')) {
    if (!part.includes('=')) {
      rest.push(part)
      continue
    }
    const sides = splitTop(part, (t) => t === '=')
    if (sides.some((side) => side.length === 0 || side.some((t) => RELATIONS.has(t)))) throw new LatexError('等式と不等式を 1 つの条件にまとめた範囲は描けません（値は計算します）')
    for (let k = 0; k + 1 < sides.length; k++) {
      const [a, b] = [put(sides[k]), put(sides[k + 1])]
      const single = (ts: Token[]) => (ts.length === 1 && names.includes(ts[0]) && !solved.has(ts[0]) ? ts[0] : null)
      // 両方が名前なら、後ろの座標を前の座標で書く（x = y = z は y = x, z = x）
      const [na, nb] = [single(a), single(b)]
      let target = na && nb ? (names.indexOf(na) > names.indexOf(nb) ? na : nb) : (nb ?? na)
      let value = target === nb && na !== target ? a : b
      if (target === null) {
        // x + y + z = 1 のように、足し算の項の 1 つが座標だけなら、その座標について解く（後ろの座標から）
        const iso = isolateTerm(a, b, names.filter((n) => !solved.has(n)))
        if (iso === null) throw new LatexError('この等式の範囲は図にできません（y = x² や x + y + z = 1 のように、座標 1 つについて解ける形なら描けます。値は計算します）')
        ;[target, value] = iso
      }
      if (value.includes(target)) throw new LatexError('この等式の範囲は図にできません（値は計算します）')
      for (const [n, v] of solved) solved.set(n, v.flatMap((t) => (t === target ? ['\\left', '(', ...value, '\\right', ')'] : [t])))
      solved.set(target, value)
    }
  }
  const params = names.filter((n) => !solved.has(n))
  if (params.length === 0) throw new LatexError('等式で範囲が 1 点になります')
  const comps = names.map((n) => (solved.has(n) ? solved.get(n)! : [n]))
  const left = rest.map(put).reduce<Token[]>((a, b) => (a.length === 0 ? b : [...a, ',', ...b]), [])
  return renameCoordParams({ comps, params, cond: left })
}

/** 同じ深さの + と − で、符号付きの項に分ける（先頭の符号の無い項は +） */
function signedTerms(toks: Token[]): { sign: '+' | '-'; toks: Token[] }[] {
  const out: { sign: '+' | '-'; toks: Token[] }[] = [{ sign: '+', toks: [] }]
  let depth = 0
  for (let k = 0; k < toks.length; k++) {
    const t = toks[k]
    if (t === '\\left' || t === '\\right') {
      depth += t === '\\left' ? 1 : -1
      out[out.length - 1].toks.push(t, toks[++k])
      continue
    }
    if (t === '{' || t === '(' || t === '[') depth++
    else if (t === '}' || t === ')' || t === ']') depth--
    const prev = toks[k - 1]
    if (depth === 0 && (t === '+' || t === '-') && !(prev === '^' || prev === '_')) {
      if (out[out.length - 1].toks.length === 0 && out.length === 1) out[0].sign = t
      else out.push({ sign: t, toks: [] })
      continue
    }
    out[out.length - 1].toks.push(t)
  }
  return out
}

/**
 * 等式 a = b の、ほかに出てこない座標 1 つだけの項（x + y + z = 1 の z）を選んで、その座標と値（トークン）を返す。無ければ null
 */
function isolateTerm(a: Token[], b: Token[], free: string[]): [string, Token[]] | null {
  const paren = (ts: Token[]) => ['\\left', '(', ...ts, '\\right', ')']
  for (const [x, y] of [[a, b], [b, a]]) {
    const terms = signedTerms(x)
    for (const n of [...free].reverse()) {
      const k = terms.findIndex((t) => t.toks.length === 1 && t.toks[0] === n)
      if (k < 0) continue
      const others = terms.filter((_, j) => j !== k)
      if (others.some((t) => t.toks.includes(n)) || y.includes(n)) continue
      const rest = others.flatMap((t, j) => (j === 0 && t.sign === '+' ? t.toks : [t.sign, ...t.toks]))
      // +n + rest = y なら n = y − (rest)、−n + rest = y なら n = (rest) − y
      const value = terms[k].sign === '+' ? (rest.length ? [...paren(y), '-', ...paren(rest)] : y) : rest.length ? [...paren(rest), '-', ...paren(y)] : ['-', ...paren(y)]
      return [n, value]
    }
  }
  return null
}

/**
 * 座標と同じ名前の媒介変数（{(x, x²) | 0 ≤ x ≤ 1} の x）を、使われていない文字に付け替える。
 * 被積分関数の x, y, z に点の成分を入れるとき、媒介変数の x と座標の x を取り違えないように
 */
function renameCoordParams(region: { comps: Token[][]; params: Token[]; cond: Token[] }): IntegralRegion {
  // label: 付け替えた名前 → 書いた名前（進み具合のスライダーの見出しは、書いた名前で出す）
  const clash = region.params.filter((p) => p === 'x' || p === 'y' || p === 'z')
  if (clash.length === 0) return region
  const used = new Set([...region.comps.flat(), ...region.cond])
  const free = ['t', 's', 'u', 'v', 'p', 'q'].filter((n) => !used.has(n))
  const to = new Map<Token, Token>()
  for (const p of clash) to.set(p, free.shift() ?? p)
  const re = (ts: Token[]) => ts.map((t) => to.get(t) ?? t)
  const label = new Map([...to].map(([a, b]) => [b, a]))
  return { comps: region.comps.map(re), params: region.params.map((p) => to.get(p) ?? p), cond: re(region.cond), label }
}

/** 中身の x, y, z に点の成分を入れる（\operatorname{max} などの名前の中の文字は変えない） */
function substitute(body: Token[], comps: Token[][]): Token[] {
  const out: Token[] = []
  for (let k = 0; k < body.length; k++) {
    const t = body[k]
    if ((t === '\\operatorname' || t === '\\mathrm' || t === '\\text' || t === '\\mathbf') && body[k + 1] === '{') {
      const end = body.indexOf('}', k)
      out.push(...body.slice(k, end < 0 ? body.length : end + 1))
      k = end < 0 ? body.length : end
      continue
    }
    const c = comps['xyz'.indexOf(t)]
    if (t.length === 1 && c) out.push('\\left', '(', ...c, '\\right', ')')
    else out.push(t)
  }
  return out
}

/**
 * 式の最初の積分を、グラフに描くための式にする。積分が無ければ null、描けない形なら error（理由）。
 * - 1 変数の定積分: 曲線と軸の間の面積（高さ 0 の面）
 * - 2 重積分（累次積分・領域の上の積分）: 領域の上の、高さが被積分関数の立体（体積が積分の値）。
 *   範囲が変数の式（1 ≤ x ≤ y）でも、条件をそのまま不等式として描くので、そのまま描ける
 * - 3 重積分: 領域の中を、被積分関数の値の色で塗る
 * - 平面の曲線の上の線積分: 曲線の上に立てた、高さが被積分関数の幕（F · dr は接線の成分）
 * - 曲面の上の面積分: 曲面を、法線の向きに被積分関数の分だけ厚くした立体（F · dS は法線の成分）
 */
export function integralDisplay(latex: string, progress = 1, scale = 1, flat = true): IntegralView | { error: string } | null {
  if (!hasIntegral(latex)) return null
  const toks = tokenize(latex)
  const i = toks.findIndex((t) => INTEGRALS.has(t))
  if (i < 0) return null
  try {
    return integralView(toks, i, Math.min(1, Math.max(0, progress)), scale, flat)
  } catch (e) {
    if (e instanceof LatexError) return { error: e.message }
    throw e
  }
}

/** 定積分の近似の方法（左端・右端・中点のリーマン和、台形則、シンプソン則） */
export type RiemannMethod = 'left' | 'right' | 'mid' | 'trap' | 'simpson'

export const RIEMANN_METHODS: { id: RiemannMethod; label: string; order: string }[] = [
  { id: 'left', label: '左端', order: 'O(1/n)' },
  { id: 'right', label: '右端', order: 'O(1/n)' },
  { id: 'mid', label: '中点', order: 'O(1/n²)' },
  { id: 'trap', label: '台形', order: 'O(1/n²)' },
  { id: 'simpson', label: 'シンプソン', order: 'O(1/n⁴)' },
]

/** 近似の図形（latex）・近似の値の式（sum）・正確な積分の式（exact） */
export interface RiemannView {
  latex: string
  sum: string
  exact: string
  /** 説明（「左端のリーマン和（n = 8）」） */
  label: string
  /** 実際に使った n（シンプソンは偶数に切り上げる） */
  n: number
}

/**
 * 1 変数の定積分 ∫_a^b f dx を、n 等分した近似で描く（学習用。計画のおすすめ 2）。
 * 図形は 1 つの集合: Δx = (b − a)/n、K = min(⌊(x − a)/Δx⌋, n − 1) として、
 * 長方形の高さ f(a + (K + c)Δx)（左端 c = 0・右端 c = 1・中点 c = 1/2）、台形は両端を結ぶ線分、シンプソンは 2 区間ごとの放物線
 * の、0 との間の領域 { (x, h, k) | a ≤ x ≤ b, min(0, F) ≤ h ≤ max(0, F), k = 0 }
 */
export function riemannDisplay(latex: string, method: RiemannMethod, nIn: number): RiemannView | { error: string } | null {
  if (!hasIntegral(latex)) return null
  try {
    const toks = tokenize(latex)
    const i = toks.findIndex((t) => INTEGRALS.has(t))
    if (i < 0) return null
    if (toks[i] !== '\\int') throw new LatexError('近似は、1 変数の定積分 ∫_a^b f dx で描けます')
    const head = limitsAt(toks, i)
    if (head.sub === null || head.sup === null || head.sub.length === 0 || head.sup.length === 0) throw new LatexError('近似は、上下の端のある定積分で描けます')
    const span = integrandSpan(toks, head.k, 1)
    if (span.vector) throw new LatexError('近似は、値が数になる被積分関数で描けます')
    const body = toks.slice(head.k, span.end)
    if (body.length === 0) throw new LatexError('積分する式を書いてください')
    if (INTEGRALS.has(body[0])) throw new LatexError('近似は、1 変数の定積分（入れ子でないもの）で描けます')
    const v = span.vars[0]
    const used = new Set([...body, ...head.sub, ...head.sup].filter((t) => /^[A-Za-z]$/.test(t)))
    const free = ['x', 'y', 'z'].find((c) => used.has(c) && c !== v)
    if (free) throw new LatexError(`範囲か中身に、積分の変数でない ${free} があるので描けません`)
    if ([...head.sub, ...head.sup].includes(v)) throw new LatexError('積分の端に、積分の変数は使えません')
    let n = Math.max(1, Math.min(400, Math.round(nIn)))
    if (method === 'simpson' && n % 2 === 1) n += 1
    const paren = (x: string) => `\\left(${x}\\right)`
    const a = paren(joinToks(head.sub))
    const b = paren(joinToks(head.sup))
    const D = `\\frac{${b}-${a}}{${n}}`
    /** 被積分関数の、点 p での値 */
    const f = (p: string) => paren(joinToks(body.flatMap((t) => (t === v ? tokenize(paren(p)) : [t]))))
    const idx = ['k', 'j', 'm', 'p', 'q'].find((c) => !used.has(c) && c !== v) ?? 'k'
    const [h, kk] = ['y', 'z', 'w', 'h', 'q', 'p'].filter((c) => c !== v && !used.has(c) && c !== idx)
    const X = (c: string) => `${a}+${paren(c)}${D}`
    const K = `\\min\\left(\\left\\lfloor\\frac{${v}-${a}}{${D}}\\right\\rfloor,${n - 1}\\right)`
    let F: string
    let sum: string
    if (method === 'left' || method === 'right' || method === 'mid') {
      const c = method === 'left' ? '0' : method === 'right' ? '1' : '\\frac{1}{2}'
      F = f(X(`${K}+${c}`))
      sum = `${D}\\sum_{${idx}=0}^{${n - 1}}${f(X(`${idx}+${c}`))}`
    } else if (method === 'trap') {
      const x0 = X(K)
      F = `${f(x0)}+\\frac{${f(X(`${K}+1`))}-${f(x0)}}{${D}}${paren(`${v}-${paren(x0)}`)}`
      sum = `\\frac{${D}}{2}\\left(${f(a)}+${f(b)}\\right)+${D}\\sum_{${idx}=1}^{${n - 1}}${f(X(idx))}`
    } else {
      const P = `\\min\\left(\\left\\lfloor\\frac{${v}-${a}}{2${D}}\\right\\rfloor,${n / 2 - 1}\\right)`
      const [x0, x1, x2] = [X(`2${paren(P)}`), X(`2${paren(P)}+1`), X(`2${paren(P)}+2`)]
      const d = (p: string, q: string) => paren(`${v}-${paren(p)}`) + paren(`${v}-${paren(q)}`)
      F = `\\frac{${f(x0)}${d(x1, x2)}-2${f(x1)}${d(x0, x2)}+${f(x2)}${d(x0, x1)}}{2${paren(D)}^{2}}`
      sum = `\\frac{${D}}{3}\\left(${f(a)}+${f(b)}+4\\sum_{${idx}=1}^{${n / 2}}${f(X(`2${idx}-1`))}+2\\sum_{${idx}=1}^{${n / 2 - 1}}${f(X(`2${idx}`))}\\right)`
    }
    const set = `\\left\\{\\left(${v},${h},${kk}\\right)\\mid ${a}\\le ${v}\\le ${b},\\min\\left(0,${F}\\right)\\le ${h}\\le\\max\\left(0,${F}\\right),${kk}=0\\right\\}`
    const exact = `\\int_{${joinToks(head.sub)}}^{${joinToks(head.sup)}}${joinToks(body)}\\,d${v}`
    const name = RIEMANN_METHODS.find((m) => m.id === method)!.label
    const label = method === 'trap' ? `台形則（n = ${n}）` : method === 'simpson' ? `シンプソン則（n = ${n}）` : `${name}のリーマン和（n = ${n}）`
    return { latex: set, sum, exact, label, n }
  } catch (e) {
    if (e instanceof LatexError) return { error: e.message }
    throw e
  }
}

/** 数を LaTeX に書く（短く） */
const numTex = (v: number) => {
  const r = String(Math.round(v * 1e6) / 1e6)
  return r.startsWith('-') ? `\\left(${r}\\right)` : r
}

/**
 * 条件の中の「下の端 ≤ 変数 ≤ 上の端」を探す（names の順に、端にほかの変数を含まないものを優先）。
 * 見つかれば、その変数・条件の位置・両端
 */
function findSweep(cond: Token[], names: string[]): { name: string; part: number; parts: Token[][]; lo: Token[]; hi: Token[] }[] {
  const parts = splitTop(cond, (t) => t === ',')
  const found: { name: string; part: number; parts: Token[][]; lo: Token[]; hi: Token[] }[] = []
  parts.forEach((p, k) => {
    const pieces = splitTop(p, (t) => RELATIONS.has(t))
    if (pieces.length !== 3) return
    const rels = p.filter((t) => RELATIONS.has(t))
    if (!rels.every((r) => ['<', '\\lt', '\\le', '\\leq', '\\leqslant', '\\leqq'].includes(r))) return
    if (pieces[1].length !== 1 || !names.includes(pieces[1][0])) return
    found.push({ name: pieces[1][0], part: k, parts, lo: pieces[0], hi: pieces[2] })
  })
  const free = (f: (typeof found)[number]) => ![...f.lo, ...f.hi].some((t) => names.includes(t))
  return names.flatMap((n) => found.filter((f) => f.name === n)).sort((a, b) => Number(free(b)) - Number(free(a)))
}

/** 数だけの端（3・2\\pi・\\frac{\\pi}{2}・-1.5 など）の値。式なら null */
function plainNumber(latex: string): number | null {
  const t = latex.replace(/\s+/g, '').replace(/^\\left\((.*)\\right\)$/, '$1')
  const frac = /^(-?)\\frac\{(.+)\}\{(.+)\}$/.exec(t)
  if (frac) {
    const [n, d] = [plainNumber(frac[2]), plainNumber(frac[3])]
    return n === null || d === null || d === 0 ? null : (frac[1] ? -1 : 1) * (n / d)
  }
  const m = /^(-?)(\d+(?:\.\d+)?|\.\d+)?(\\pi)?$/.exec(t)
  if (!m || (m[2] === undefined && m[3] === undefined)) return null
  return (m[1] ? -1 : 1) * (m[2] === undefined ? 1 : Number(m[2])) * (m[3] ? Math.PI : 1)
}

/** 「下の端 + s (上の端 − 下の端)」（両端が数なら、計算した数） */
const between = (lo: Token[], hi: Token[], s: number): string => {
  const [a, b] = [joinToks(lo), joinToks(hi)]
  if (s >= 1) return b
  const [na, nb] = [plainNumber(a), plainNumber(b)]
  if (na !== null && nb !== null) return numTex(na + s * (nb - na))
  return `\\left(${a}\\right)+${numTex(s)}\\left(\\left(${b}\\right)-\\left(${a}\\right)\\right)`
}

/** 被積分関数がベクトル場の演算（∇ · 𝐅）なら、その場 𝐅（矢印で添える）。ベクトル場の積分なら中身そのもの */
function fieldOf(body: Token[], vector: boolean): Token[] | null {
  // 定義した場の名前だけ（F・𝐅）なら、定義の行がもう矢印を描いているので、重ねない
  const nameOnly = (ts: Token[]) =>
    (ts.length === 1 && /^[A-Za-z]$/.test(ts[0])) || (ts.length === 4 && ['\\vec', '\\mathbf', '\\boldsymbol', '\\mathbb'].includes(ts[0]) && ts[1] === '{' && ts[3] === '}')
  if (vector) return nameOnly(body) ? null : body
  const k = body.findIndex((t, j) => t === '\\nabla' && body[j + 1] === '\\cdot')
  return k === 0 && !nameOnly(body.slice(2)) ? body.slice(2) : null
}

function integralView(toks: Token[], i: number, progress: number, scale: number, flat = true): IntegralView {
  const multi = toks[i] === '\\iint' || toks[i] === '\\oiint' ? 2 : toks[i] === '\\iiint' ? 3 : 1
  const head = limitsAt(toks, i)
  let span = integrandSpan(toks, head.k, multi)
  let body = toks.slice(head.k, span.end)
  let region: IntegralRegion
  if (head.sub !== null && head.sup !== null) {
    // 上下の端のある積分（入れ子なら累次積分）: 外側の変数から順に、範囲を条件にする
    const names: string[] = []
    const cond: Token[] = []
    let lim = head
    for (;;) {
      if (span.vector || lim.sub === null || lim.sup === null || lim.sub.length === 0 || lim.sup.length === 0) throw new LatexError('積分の範囲を書いてください（∫ の下と上）')
      if (names.length > 0) cond.push(',')
      names.push(span.vars[0])
      cond.push(...lim.sub, '\\le', span.vars[0], '\\le', ...lim.sup)
      // 中身が丸ごと、上下の端のある積分なら、その中へ進む
      if (!INTEGRALS.has(body[0]) || body[0] !== '\\int') break
      const inner = limitsAt(body, 0)
      if (inner.sub === null || inner.sup === null) break
      let innerSpan
      try {
        innerSpan = integrandSpan(body, inner.k, 1)
      } catch {
        break
      }
      let rest = innerSpan.next
      while (SPACES.has(body[rest] ?? '')) rest++
      if (rest < body.length || innerSpan.vector) break
      lim = inner
      span = innerSpan
      body = body.slice(inner.k, innerSpan.end)
    }
    region = { names, cond }
  } else if (head.sub !== null) {
    region = integralRegion(head.sub, span.vars, multi)
  } else {
    throw new LatexError('不定積分は描けません（範囲のある積分を描きます）')
  }
  if (body.length === 0) throw new LatexError('積分する式を書いてください')
  const paren = (ts: Token[] | string) => `\\left(${typeof ts === 'string' ? ts : joinToks(ts)}\\right)`
  const set = (point: string, cond: string) => `\\left\\{${point}\\mid ${cond}\\right\\}`
  // 媒介変数そのものを使う被積分関数（(t + 1, t²)）は、空間の場ではないので矢印の場を添えない
  const usesParams = !('names' in region) && body.some((t) => region.params.includes(t))
  const field = usesParams ? null : fieldOf(body, span.vector)
  /** 積分する経路・領域そのもの（範囲全体。名前で書いた範囲も、積分記号の下に直接書いた範囲も） */
  const pathExtra = (): IntegralExtra[] => {
    // 立体（3 重積分・媒介変数 3 つ）は、積分の図そのものが立体なので重ねない
    if ('names' in region ? region.names.length === 3 : region.params.length === 3) return []
    if ('names' in region && region.names.length === 1) {
      // 1 変数の区間: 横の軸の上の線分（変数の名前は x に読み替える）
      const n = region.names[0]
      const cond = joinToks(region.cond.map((t) => (t === n ? 'x' : t)))
      return [{ role: 'path', latex: set(paren('x,y,z'), `${cond},y=0,z=0`), label: '積分する区間' }]
    }
    const src =
      'names' in region
        ? set(paren(region.names.join(',')), joinToks(region.cond))
        : set(paren(region.comps.map(joinToks).join(',')), joinToks(region.cond))
    const latex = regionDisplayLatex(src)
    if (!latex) return []
    const label = 'names' in region ? (region.names.length === 1 ? '積分する区間' : '積分する領域') : region.params.length === 1 ? '経路' : region.params.length === 2 ? '積分する曲面・領域' : '積分する立体'
    return [{ role: 'path', latex, label }]
  }
  const fieldExtra = (): IntegralExtra[] => [...pathExtra(), ...(field ? [{ role: 'field' as const, latex: joinToks(field), label: 'ベクトル場' }] : [])]

  if ('names' in region) {
    const { names } = region
    if (span.vector) throw new LatexError('F · dr・F · dS の積分は、曲線・曲面を {(cos t, sin t) | 0 ≤ t ≤ 2π} の形で書いたものを描けます')
    if (new Set(names).size !== names.length) throw new LatexError('積分の変数の名前が重なっています')
    // 進み具合: 1 つの変数の範囲を「下の端 〜 途中」に縮める
    const sw = findSweep(region.cond, names)[0]
    let cond = region.cond
    if (sw && progress < 1) {
      const parts = sw.parts.map((p) => [...p])
      parts[sw.part] = [...sw.lo, '\\le', sw.name, '\\le', ...tokenize(between(sw.lo, sw.hi, progress))]
      cond = parts.flatMap((p, k) => (k === 0 ? p : [',', ...p]))
    }
    const sym = ['\\int', '\\iint', '\\iiint'][names.length - 1] ?? '\\int'
    const partial = sw ? `${sym}_{${set(paren(names.join(',')), joinToks(cond))}}${joinToks(body)}\\,${names.map((n) => `d${n}`).join('\\,')}` : undefined
    const swept = { sweep: sw?.name, partial, extras: fieldExtra() }
    // 積分の変数でない座標 x, y, z が残っていると、1 つの図形にならない（∫_0^x … dt は x ごとに違う図形）
    const used = new Set([...body, ...cond].filter((t) => /^[A-Za-z]$/.test(t)))
    const free = ['x', 'y', 'z'].find((n) => used.has(n) && !names.includes(n))
    if (free) throw new LatexError(`範囲か中身に、積分の変数でない ${free} があるので描けません（${free} が数なら描けます）`)
    /** 積分の変数でも式の中の文字でもない、足す座標の名前 */
    const extra = ['z', 'y', 'x', 'w', 'h', 'k', 'q', 'p'].filter((n) => !names.includes(n) && !used.has(n))
    const f = joinToks(body)
    const betweenZero = (h: string) => `\\min\\left(0,${f}\\right)\\le ${h}\\le\\max\\left(0,${f}\\right)`
    if (names.length === 1) {
      const [h, k] = [extra.find((n) => n !== 'z') ?? extra[0], extra.find((n) => n === 'z') ?? extra[1]]
      // 2D 表示: 横の軸の上に、縦（2 つ目の座標）の向きに高さ f の面。3D 表示: 線積分と同じく、横の軸の上に立てた高さ f（3 つ目の座標 z）の幕
      if (!flat) {
        return {
          latex: set(paren(`${names[0]},${h},${k}`), `${joinToks(cond)},${h}=0,${betweenZero(k)}`),
          note: '軸の上に立てた幕（高さが被積分関数。面積が積分の値。高さ 0 より下は負）',
          ...swept,
        }
      }
      return {
        latex: set(paren(`${names[0]},${h},${k}`), `${joinToks(cond)},${betweenZero(h)},${k}=0`),
        note: '曲線と軸の間（面積が積分の値。軸より下は負）',
        ...swept,
      }
    }
    if (names.length === 2) {
      return {
        latex: set(paren(`${names.join(',')},${extra[0]}`), `${joinToks(cond)},${betweenZero(extra[0])}`),
        note: '領域の上の、高さが被積分関数の立体（体積が積分の値。高さ 0 より下は負）',
        ...swept,
      }
    }
    if (names.length === 3 && !body.some((t) => names.includes(t)) && !field) {
      // 被積分関数が定数なら、色で塗っても一色になって形が分からない。領域そのもの（境目の面）を描く
      return {
        latex: set(paren(names.join(',')), joinToks(cond)),
        note: '被積分関数が定数なので、領域を描く（体積 × 値が積分の値）',
        ...swept,
      }
    }
    if (names.length === 3) {
      // 値を色で塗る描き方（陰関数の F の値）は、名前を読み替えないので、変数を x, y, z に直す
      const to = new Map(names.map((n, k) => [n, 'xyz'[k]]))
      const re = (ts: Token[]) => joinToks(ts.map((t) => to.get(t) ?? t))
      return {
        latex: `\\begin{cases}${re(body)}&${re(cond)}\\end{cases}=0`,
        note: '領域の中を、被積分関数の値の色で塗る（赤は正・青は負、|値| が大きいほど濃い）',
        field: true,
        ...swept,
      }
    }
    throw new LatexError('4 重以上の積分は描けません')
  }

  const { comps, params } = region
  const dim = comps.length
  if (params.length === 0) throw new LatexError('範囲の条件に、媒介変数の範囲（0 ≤ t ≤ 2π など）を書いてください')
  if (params.length > 3 || dim < 2 || dim > 3) throw new LatexError('この形の範囲の上の積分は描けません')
  // 進み具合: 最初の媒介変数の範囲を「下の端 〜 途中」に縮める
  const sweeps = findSweep(region.cond, params)
  const sw = sweeps.find((f) => f.name === params[0]) ?? sweeps[0]
  let cond = region.cond
  if (sw && progress < 1) {
    const parts = sw.parts.map((p) => [...p])
    parts[sw.part] = [...sw.lo, '\\le', sw.name, '\\le', ...tokenize(between(sw.lo, sw.hi, progress))]
    cond = parts.flatMap((p, k) => (k === 0 ? p : [',', ...p]))
  }
  // 曲線の上の P dx + Q dy は、続く項も同じ曲線の上の積分にして足す
  const tail = span.vars.length === 1 && COORD_LETTERS.has(span.vars[0]) && params.length === 1 ? formTerms(toks, span.next, span.vars[0]) : { terms: [], next: span.next }
  const partialRegion = set(paren(comps.map(joinToks).join(',')), joinToks(cond))
  const partial = sw
    ? `${toks[i]}_{${partialRegion}}${joinToks(toks.slice(head.k, span.next))}${tail.terms.map((t) => `${t.sign}${toks[i]}_{${partialRegion}}${joinToks(toks.slice(t.from, t.next))}`).join('')}`
    : undefined

  // 媒介変数を、計算エンジンが読める u・v・t に付け替える（足す 1 つは、残った名前）
  // 曲線の幕は媒介変数 2 つの曲面なので u・v（t と u を混ぜると、計算エンジンは 3 つの媒介変数の立体として読む）
  const targets = params.length === 1 ? ['u', 'v'] : ['u', 'v', 't']
  const free = targets.filter((n) => !params.includes(n))
  const rename = new Map<Token, Token>()
  for (const p of params) {
    if (!targets.includes(p)) rename.set(p, free.shift()!)
  }
  const s = free.shift() ?? 't'
  const r = paren(comps.map(joinToks).join(','))
  /** 被積分関数の、点 r での値（\left. 中身 \right|_{x = …}。∇ × 𝐅 のように、展開してから点を入れる） */
  // 中身が名前だけ（定義した F・𝐅）か、∇ のように展開してはじめて x, y, z が出てくる式なら、点での値の形にする。
  // それ以外は、中身の x, y, z に点の成分を直接入れる
  const opaque = body.some((t) => t === '\\nabla' || t === '\\vec' || t === '\\mathbf' || t === '\\boldsymbol' || t === '\\mathbb' || t === '\\operatorname') || (body.length === 1 && /^[A-Za-z]$/.test(body[0]))
  const valueAt = (pt: string[]) =>
    opaque
      ? `\\left.${joinToks(body)}\\right|_{${pt.map((c, k) => `${'xyz'[k]}=${c}`).join(',')}}`
      : paren(substitute(body, pt.map((c) => tokenize(c))))
  const value = valueAt(comps.map(joinToks))
  const range = `${joinToks(cond)},0\\le ${s}\\le 1`
  const finish = (latex: string) => joinToks(tokenize(latex).map((t) => rename.get(t) ?? t))

  // 今の点（掃いている媒介変数は途中の値、ほかの媒介変数は範囲の真ん中）と、そこでの矢印
  const extras: IntegralExtra[] = fieldExtra()
  if (sw) {
    const at = new Map<Token, string>()
    at.set(sw.name, paren(between(sw.lo, sw.hi, progress)))
    for (const p of params) {
      if (at.has(p)) continue
      const f = sweeps.find((x) => x.name === p)
      if (f) at.set(p, paren(between(f.lo, f.hi, 0.5)))
    }
    if (at.size === params.length && params.length < 3) {
      const put = (latex: string) => joinToks(tokenize(latex).flatMap((t) => (at.has(t) ? tokenize(at.get(t)!) : [t])))
      const point = comps.map((c) => put(joinToks(c)))
      const P = paren(point.join(','))
      extras.push({ role: 'point', latex: P, label: '今の点' })
      const arrow = (v: string) => `${P}+t${paren(v)}`
      const unit = (v: string) => `\\frac{${numTex(scale)}${paren(v)}}{\\left|${paren(v)}\\right|}`
      const deriv = (p: string) => `\\left.\\frac{\\partial}{\\partial ${p}}${r}\\right|_{${params.map((q) => `${q}=${at.get(q)}`).join(',')}}`
      // 被積分関数に媒介変数そのもの（F = (t + 1, t²)）があれば、それにも今の値を入れる
      const fAt = put(valueAt(point))
      if (params.length === 1) {
        const T = deriv(params[0])
        extras.push({ role: 'tangent', latex: arrow(unit(T)), label: '接線の向き r′' })
        if (span.vector) {
          extras.push({ role: 'vector', latex: arrow(`\\frac{${numTex(scale)}${fAt}}{\\left|${fAt}\\right|}`), label: 'F（向き）' })
          extras.push({ role: 'component', latex: arrow(`\\frac{${numTex(scale)}\\left(${fAt}\\cdot ${paren(T)}\\right)${paren(T)}}{\\left|${fAt}\\right|\\left|${paren(T)}\\right|^{2}}`), label: 'F の接線の成分（F · dr の F · T）' })
        }
      } else if (params.length === 2 && dim === 3) {
        const N = `${paren(deriv(params[0]))}\\times ${paren(deriv(params[1]))}`
        extras.push({ role: 'normal', latex: arrow(unit(N)), label: '法線の向き r_u × r_v' })
        if (span.vector) {
          extras.push({ role: 'vector', latex: arrow(`\\frac{${numTex(scale)}${fAt}}{\\left|${fAt}\\right|}`), label: 'F（向き）' })
          extras.push({ role: 'component', latex: arrow(`\\frac{${numTex(scale)}\\left(${fAt}\\cdot ${paren(N)}\\right)${paren(N)}}{\\left|${fAt}\\right|\\left|${paren(N)}\\right|^{2}}`), label: 'F の法線の成分（F · dS の F · n）' })
        }
      }
    }
  }
  const swept = { sweep: sw ? (region.label?.get(sw.name) ?? sw.name) : undefined, partial, extras }

  if (params.length === 3) {
    // 媒介変数 3 つの立体（体積分）: 立体そのもの（進み具合で、最初の媒介変数の範囲まで）
    if (span.vector) throw new LatexError('立体の上の F · d… は描けません')
    return {
      latex: finish(set(r, joinToks(cond))),
      note:
        body[0] === '\\nabla' && body[1] === '\\cdot'
          ? '積分する立体（発散を足し合わせる。値は、境目の面を通る外向きの流束と同じ: ガウスの定理）'
          : '積分する立体（体積要素 |det J| で被積分関数を足し合わせる）',
      ...swept,
    }
  }
  if (params.length === 1) {
    const d = `\\left(\\frac{d}{d${joinToks([params[0]])}}${r}\\right)`
    const height = span.vector ? `\\frac{${value}\\cdot ${d}}{\\left|${d}\\right|}` : value
    if (dim === 2) {
      // 平面の曲線: 高さが被積分関数の幕。F · dr は接線の向きの成分 F · r′ / |r′|
      return {
        latex: finish(set(paren(`${comps.map(joinToks).join(',')},${s}${height}`), range)),
        note: span.vector ? '曲線の上の幕（高さは F の接線の向きの成分。面積が積分の値）' : '曲線の上の幕（高さが被積分関数。面積が積分の値）',
        ...swept,
      }
    }
    // 空間の曲線: 曲線に垂直で、できるだけ上を向いた向き N = (e_z − (e_z · T) T) / |…| に、高さの分だけ広げた帯
    // （N ⊥ r′ なので、帯の面積がちょうど積分の値）
    const ez = '\\left(0,0,1\\right)'
    const perp = `\\left(${ez}-\\frac{${d}\\cdot ${ez}}{${d}\\cdot ${d}}${d}\\right)`
    const N = `\\frac{${perp}}{\\left|${perp}\\right|}`
    return {
      latex: finish(set(`${r}+${s}\\left(${height}\\right)${N}`, range)),
      note: span.vector
        ? '空間の曲線に沿った帯（幅は F の接線の向きの成分。曲線に垂直な、上向きの向きに広げる。面積が積分の値）'
        : '空間の曲線に沿った帯（幅が被積分関数。曲線に垂直な、上向きの向きに広げる。面積が積分の値）',
      ...swept,
    }
  }
  if (dim === 2) {
    if (span.vector) throw new LatexError('平面の領域の上の F · dS は描けません')
    return {
      latex: finish(set(paren(`${comps.map(joinToks).join(',')},${s}${value}`), range)),
      note: '領域の上の、高さが被積分関数の立体（体積が積分の値）',
      ...swept,
    }
  }
  // 曲面: 法線 n = r_u × r_v / |r_u × r_v| の向きに、被積分関数の分だけ厚くする。F · dS は法線の向きの成分 F · n
  const du = (p: Token) => `\\left(\\frac{\\partial}{\\partial ${joinToks([p])}}${r}\\right)`
  const cross = `${du(params[0])}\\times ${du(params[1])}`
  const normal = `\\frac{${cross}}{\\left|${cross}\\right|}`
  const thick = span.vector ? paren(`${value}\\cdot ${paren(normal)}`) : opaque ? paren(value) : value
  return {
    latex: finish(set(`${r}+${s}${thick}${normal}`, range)),
    note: span.vector ? '曲面を、法線の向きに F の法線の成分だけ厚くした立体（体積が積分の値）' : '曲面を、法線の向きに被積分関数の分だけ厚くした立体（体積が積分の値）',
    ...swept,
  }
}

/** LaTeX を計算エンジン用の式の文字列に変換する。変換できなければ LatexError を投げる */
export function latexToText(latex: string): string {
  const hit = textCache.get(latex)
  if (hit !== undefined) {
    if (hit instanceof LatexError) throw hit
    return hit
  }
  if (textCache.size >= 2000) textCache.clear()
  try {
    const text = convertLatex(latex)
    textCache.set(latex, text)
    return text
  } catch (e) {
    if (e instanceof LatexError) textCache.set(latex, e)
    throw e
  }
}

function convertLatex(latex: string): string {
  // a ≡ b \pmod{n} は、mod(a − b, n) = 0（計画 M13）
  const pm = latex.match(/^(.*?)\\equiv(.*?)\\pmod\s*\{(.*)\}\s*$/s) ?? latex.match(/^(.*?)\\equiv(.*?)\\pmod\s*(\S)\s*$/s)
  if (pm) {
    const [a, b, n] = [pm[1], pm[2], pm[3]].map((x) => new Converter(tokenize(x)).run())
    return `mod((${a})-(${b}),${n})=0`
  }
  // 写像の書き方 f\colon x\mapsto x^2 は、関数の定義 f(x)=x^2 と同じ
  const map = latex.match(/^\s*([A-Za-z])\s*(?:\\colon|:)\s*([A-Za-z])\s*\\mapsto(.*)$/s)
  if (map) latex = `${map[1]}\\left(${map[2]}\\right)=${map[3]}`
  return new Converter(tokenize(latex)).run()
}

/** 裸の床・天井の括弧（\left / \right の付いていないもの）の開き → 閉じ */
const BARE_FENCES: Record<string, string> = { '\\lfloor': '\\rfloor', '\\lceil': '\\rceil' }

/**
 * 対になった裸の \lfloor … \rfloor / \lceil … \rceil を \left\lfloor … \right\rfloor の形に直す。
 * MathLive は裸の ⌊ ⌋ を1組の括弧として扱わないので、その後ろで / を打つと、分子に式の先頭（z= など）まで入ってしまう。
 * 同じ { } の中で対になっているものだけを直し、対にならないものはそのまま残す。
 * 例) "z=\lfloor x\rfloor" → "z=\left\lfloor x\right\rfloor"
 */
export function fenceBareDelimiters(latex: string): string {
  /** 直す所（文字の位置 → 前に入れる \left / \right） */
  const inserts = new Map<number, string>()
  /** 開いている { と裸の開き括弧（{ は null） */
  const stack: ({ name: string; at: number } | null)[] = []
  /** 直前の \left / \right の終わりの位置（無ければ -1） */
  let leftRightEnd = -1
  for (const m of latex.matchAll(/\\(?:left|right|[lr](?:floor|ceil))(?![a-zA-Z])|\\.|[{}]/g)) {
    const t = m[0]
    const at = m.index
    // \left / \right の直後の括弧はそのまま（間の空白は飛ばす）
    const fenced = leftRightEnd >= 0 && latex.slice(leftRightEnd, at).trim() === ''
    leftRightEnd = t === '\\left' || t === '\\right' ? at + t.length : -1
    if (fenced) continue
    if (t === '{') stack.push(null)
    else if (t === '}') {
      while (stack.length && stack[stack.length - 1] !== null) stack.pop() // この { } の中で閉じなかった開き括弧
      stack.pop()
    } else if (t in BARE_FENCES) stack.push({ name: t, at })
    else if (t === '\\rfloor' || t === '\\rceil') {
      const top = stack[stack.length - 1]
      if (top && BARE_FENCES[top.name] === t) {
        stack.pop()
        inserts.set(top.at, '\\left')
        inserts.set(at, '\\right')
      }
    }
  }
  if (inserts.size === 0) return latex
  let out = ''
  for (let i = 0; i < latex.length; i++) out += (inserts.get(i) ?? '') + latex[i]
  return out
}

// ---------------------------------------------------------------------------
// 媒介変数表示の組 \left(a, b, c\right) を成分に分けたり、つなげたりする
// ---------------------------------------------------------------------------

/**
 * 式全体が1組の括弧で、その一番外側にカンマがあれば、成分の LaTeX の配列を返す。そうでなければ null。
 * 例) "\left(\cos t,\sin t,t\right)" → ["\cos t", "\sin t", "t"]
 */
export function splitTuple(latex: string): string[] | null {
  const s = latex.trim()
  let inner: string
  if (s.startsWith('\\left(') && s.endsWith('\\right)')) inner = s.slice(6, -7)
  else if (s.startsWith('(') && s.endsWith(')')) inner = s.slice(1, -1)
  else return null

  const parts: string[] = []
  let depth = 0
  let start = 0
  for (let i = 0; i < inner.length; i++) {
    if (inner.startsWith('\\left', i)) {
      depth++
      i += 4
    } else if (inner.startsWith('\\right', i)) {
      depth--
      i += 5
    } else if (inner[i] === '\\') {
      i++ // \{ \, などの次の1文字は飛ばす
    } else if ('{(['.includes(inner[i])) depth++
    else if ('})]'.includes(inner[i])) depth--
    else if (inner[i] === ',' && depth === 0) {
      parts.push(inner.slice(start, i).trim())
      start = i + 1
    }
    // 外側の括弧が途中で閉じている（例: (x+1)(y+1)）なら組ではない
    if (depth < 0) return null
  }
  if (depth !== 0) return null
  parts.push(inner.slice(start).trim())
  return parts.length >= 2 ? parts : null
}

/** 成分の配列を "\left(a,b,c\right)" に戻す */
export function joinTuple(parts: string[]): string {
  return `\\left(${parts.join(',')}\\right)`
}
