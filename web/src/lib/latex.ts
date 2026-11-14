// 数式入力欄（MathLive）の LaTeX を、計算エンジン（Rust）が読める式の文字列に変換する。
//
// 例) \frac{x^{2}+1}{y}            → ((x^(2)+1)/(y))
//     \sin\left(x\right)\cdot y     → sin (x)*y
//     \operatorname{nCr}\left(5,2\right) → nCr(5,2)
//     \left|x\right|                → abs(x)
//
// 文字が続けて書かれた部分（例: "floor"）はそのまま残し、エンジン側の「関数名の最長一致」で読ませる。

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
  psi: ' psi ',
  exponentialE: 'e',
  imaginaryI: 'i',
  cdot: '*',
  times: '*',
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
    this.toks = toks
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
      this.out += '^(' + this.arg() + ')'
      return
    }
    if (t === '_') {
      // 添字: w_{2} → w_2、t_1 → t_1（文字の後ろに数字だけを付けられる）
      const sub = this.arg()
      if (!/^\d+$/.test(sub)) throw new LatexError('添字には数字だけを書いてください（例: w₂）')
      if (!/[a-zA-Z]$/.test(this.out)) throw new LatexError('添字は文字の後ろに付けてください（例: w₂）')
      this.out += `_${sub}`
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
      case 'tfrac': {
        const a = this.arg()
        const b = this.arg()
        this.out += `((${a})/(${b}))`
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
        if (!(d in OPENERS)) throw new LatexError(`括弧 ${d} は使えません`)
        this.out += OPENERS[d]
        this.seq('\\right')
        if (this.toks[this.i] !== '\\right') throw new LatexError('括弧が閉じていません')
        this.i += 2 // \right と閉じ括弧
        this.out += OPENERS[d] === '[' ? ']' : ')'
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
        this.out += s === 'e' || s === 'i' ? s : ` ${s} `
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

/** LaTeX を計算エンジン用の式の文字列に変換する。変換できなければ LatexError を投げる */
export function latexToText(latex: string): string {
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
