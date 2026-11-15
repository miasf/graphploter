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
  bot: '⊥',
  complement: '∁',
  mid: '∣',
  // 微分（\frac{d}{dx} などの部品。単独では計算エンジンが読めない）とプライム
  partial: '∂',
  infty: '∞',
  differentialD: 'd',
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
      // 度: x^{\circ} → x·(π/180)
      if (this.peek() === '\\circ' || (this.peek() === '{' && this.toks[this.i + 1] === '\\circ' && this.toks[this.i + 2] === '}')) {
        this.i += this.peek() === '{' ? 3 : 1
        this.out += '*(pi/180)'
        return
      }
      const a = this.arg()
      // 補集合 A^{\complement}
      if (a.trim() === '∁') {
        this.out += 'ᶜ'
        return
      }
      // f^{\prime} は f'（導関数）
      this.out += /^'+$/.test(a.replace(/\s+/g, '')) ? a.replace(/\s+/g, '') : '^(' + a + ')'
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
        if (!(c in sets)) throw new LatexError(`\\mathbb{${c}} はまだ使えません`)
        this.out += sets[c]
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
        const m = lo?.match(/^\s*([A-Za-z]\w*)\s*=(.+)$/)
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
          this.out += ` @${op}(${m[1]}; ${m[2].trim()}<=${m[1]}<=${hi.trim()}) `
          return
        }
        if (!m) throw new LatexError(`${sym} の下に n=1 の形で書いてください`)
        if (hi === null || hi.trim() === '') throw new LatexError(`${sym} の上に終わりの値を書いてください`)
        this.out += ` @${name}(${m[1]},${m[2].trim()},${hi.trim()}) `
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
        const { body, vars, vector } = this.integrand(multi)
        if (vector) {
          // ベクトル場の線積分・流束 ∫_C F · dr、∬_S F · dS: 曲線・曲面を集合で書く
          if (subToks === null || hi !== null) throw new LatexError('F · dr・F · dS の積分は、下に曲線・曲面を {(cos t, sin t) | 0 ≤ t ≤ 2π} の形で書いてください')
          this.out += ` @intset(${regionOf(subToks, [])}; {vec})(${body}) `
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
        this.out += ` @intset(${regionOf(subToks, vars)})(${body}) `
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
        throw new LatexError(`${env} はまだ使えません`)
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
    const rows: Token[][][] = [[[]]]
    let depth = 0
    while (this.i < this.toks.length) {
      const t = this.toks[this.i++]
      if (t === '\\end' && depth === 0) {
        this.rawGroup()
        const out = rows
          .map((cells) => cells.map((c) => (c.length ? new Converter(c).run() : '')))
          .filter((cells) => cells.some((c) => c !== ''))
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
    for (let k = this.i; k < this.toks.length; k++) {
      const t = this.toks[k]
      if (t === '{' || t === '\\left') depth++
      else if (t === '}' || t === '\\right') {
        if (depth === 0 && end === '\\right' && t === '\\right') {
          close = k
          break
        }
        depth--
      } else if (depth === 0 && end === null && (t === '\\}' || t === '\\rbrace')) {
        close = k
        break
      }
      if (depth === 0 && sep < 0 && (t === '\\mid' || t === ':' || t === '\\colon')) sep = k
    }
    if (sep < 0 || close < 0) return false
    const point = new Converter(this.toks.slice(this.i, sep)).run()
    const cond = new Converter(this.toks.slice(sep + 1, close)).run()
    if (point === '' || cond === '') throw new LatexError('集合は {(x, y) | 条件} の形で、点と条件を書いてください')
    this.out += ` ⦃${point}∣${cond}⦄ `
    this.i = close + (end === '\\right' ? 2 : 1)
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
   * 積分の中身と、その後の「d + 文字」の並び（dx\,dy など）を読む。中身は、同じ深さで最初に出てくる「d + 文字」の手前まで。
   * 中身の中の積分（入れ子）の「d + 文字」は、その積分のものとして飛ばす（∫∫ … dy dx の内側の dy を、外側の物と取り違えない）
   */
  private integrand(multi: number): { body: string; vars: string[]; vector: boolean } {
    const start = this.i
    let depth = 0
    let pending = 0
    for (let p = this.i; p < this.toks.length; p++) {
      const t2 = this.toks[p]
      if (t2 === '{' || t2 === '\\left') depth++
      else if (t2 === '}' || t2 === '\\right') {
        if (depth === 0) break
        depth--
      }
      if (depth !== 0) continue
      if (t2 === '\\int' || t2 === '\\intop' || t2 === '\\smallint') pending += 1
      else if (t2 === '\\iint') pending += 2
      else if (t2 === '\\iiint') pending += 3
      const d = differentialAt(this.toks, p)
      if (!d) continue
      if (pending > 0) {
        pending--
        p += d.len
        continue
      }
      // F · dr の形なら、中身の最後の · は内積の印（中身に入れない）
      let end = p
      while (end > start && SPACES.has(this.toks[end - 1])) end--
      if (d.vector) {
        if (this.toks[end - 1] !== '\\cdot') throw new LatexError('ベクトル場の積分は F \\cdot d\\mathbf{r} の形で書いてください')
        end--
      }
      const body = new Converter(this.toks.slice(start, end)).run()
      if (body === '') throw new LatexError('積分する式を書いてください')
      if (d.vector) {
        this.i = p + d.len + 1
        return { body, vars: [d.v], vector: true }
      }
      // 続く「d + 文字」をまとめて読む（間の \, などの空白は飛ばす）
      const vars: string[] = []
      let q = p
      for (;;) {
        while (SPACES.has(this.toks[q] ?? '')) q++
        const e = differentialAt(this.toks, q)
        if (!e || vars.length >= Math.max(multi, 3)) break
        vars.push(e.v)
        q += e.len + 1
      }
      this.i = q
      return { body, vars, vector: false }
    }
    throw new LatexError('積分の変数を dx のように書いてください')
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
const SPACES = new Set(['\\,', '\;', '\\:', '\\ ', '\\!', '\\quad', '\\qquad'])

/** 位置 p から「d + 文字 1 つ」（\mathrm{d}x・\differentialD x も）なら、変数と、d の部分のトークンの数（変数の手前まで） */
function differentialAt(toks: Token[], p: number): { v: string; len: number; vector?: boolean } | null {
  const letter = (k: number) => (/^[A-Za-z]$/.test(toks[k] ?? '') ? toks[k] : null)
  const t = toks[p]
  // ベクトルの微分 d\mathbf{r}・d\vec{S}（len は文字の手前まで。文字の後の } も読み飛ばすので、文字の位置に 1 足して返す）
  if ((t === 'd' || t === '\\differentialD') && ['\\mathbf', '\\vec', '\\boldsymbol', '\\overrightarrow'].includes(toks[p + 1] ?? '') && toks[p + 2] === '{' && letter(p + 3) && toks[p + 4] === '}') {
    return { v: toks[p + 3], len: 4, vector: true }
  }
  if ((t === 'd' || t === '\\differentialD') && letter(p + 1)) return { v: toks[p + 1], len: 1 }
  if ((t === '\\mathrm' || t === '\\operatorname') && toks[p + 1] === '{' && toks[p + 2] === 'd' && toks[p + 3] === '}' && letter(p + 4)) {
    return { v: toks[p + 4], len: 4 }
  }
  return null
}

/**
 * 積分する領域（∬ の下）を、計算エンジンの @intset の引数「(x,y),条件,…」にする。
 * 集合 \{(x,y) \mid 条件\}（\mid・|・: で区切る）と、区間の直積 [0,1]\times[0,2]（変数は dx\,dy の順）を読む
 */
function regionOf(toks: Token[], vars: string[]): string {
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
        const cond = new Converter(inner.slice(k + 1)).run()
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
    if (ranges.length !== vars.length) throw new LatexError(`区間が ${ranges.length} 個あるので、dx\\,dy のように変数を ${ranges.length} 個書いてください`)
    const conds = ranges.map((m, k) => `${m![1].trim()}<=${vars[k]}<=${m![2].trim()}`)
    const point = vars.length === 1 ? vars[0] : `(${vars.join(',')})`
    return `${point},${conds.join(',')}`
  }
  throw new LatexError('積分する領域は、{(x,y) | 条件} か [0,1]×[0,2] の形で書いてください')
}

/** LaTeX を計算エンジン用の式の文字列に変換する。変換できなければ LatexError を投げる */
export function latexToText(latex: string): string {
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
