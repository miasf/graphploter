// 打った文字の並びを、関数名（\sin・\operatorname{floor} など）や記号（\theta・\le など）に変える（入力の自動変換）。
// MathLive の inlineShortcuts は物理キーボードのキー入力でしか効かず、スマホの文字入力（IME。Android のキーボードなど）で
// 1 文字ずつ insertText が来るときは変わらない（sin が斜体の s·i·n のまま）。そこで入力のたびに、カーソルの直前の文字の並びを
// 調べて、こちらでも置き換える。物理キーボードでは MathLive が先に変えるので、ここでは何もしない（直前が 1 つの命令になっている）

/** \sin のように、MathLive・LaTeX に命令がある関数（立体で表示される） */
export const BUILTIN_FUNCTIONS = ['sin', 'cos', 'tan', 'sec', 'csc', 'cot', 'sinh', 'cosh', 'tanh', 'arcsin', 'arccos', 'arctan', 'exp', 'ln', 'log']

/**
 * \operatorname{…} で立体にする関数名（計算エンジンの関数 core/src/functions.rs のうち、変数の掛け算と紛れにくいもの）。
 * 2 文字の名前（Ci・Si・li など）と、ギリシャ文字と同じ名前（beta・zeta・psi）は、変数の掛け算・文字のつもりのことがあるので入れない
 */
export const OPERATOR_FUNCTIONS = [
  'sinc', 'asin', 'acos', 'atan', 'arcsinh', 'arccosh', 'arctanh', 'asinh', 'acosh', 'atanh',
  'abs', 'floor', 'ceil', 'round', 'sign', 'mod', 'max', 'min', 'gcd', 'lcm', 'arg', 'conj', 'Re', 'Im',
  'nCr', 'nPr', 'binomial', 'pochhammer',
  'erf', 'erfc', 'erfi', 'dawson', 'fresnelC', 'fresnelS',
  'gammaP', 'gammaQ', 'loggamma', 'lngamma', 'digamma', 'polygamma', 'betaRegularized',
  'polylog', 'dirichletEta', 'lambertW', 'productLog',
  'expIntegralE', 'expIntegralEi', 'logIntegral', 'sinIntegral', 'cosIntegral', 'sinhIntegral', 'coshIntegral',
  'besselJ', 'besselY', 'besselI', 'besselK', 'hankelH1', 'hankelH2',
  'sphericalBesselJ', 'sphericalBesselY', 'sphericalJ', 'sphericalY', 'sphericalHarmonicY',
  'airyAi', 'airyBi', 'airyAiPrime', 'airyBiPrime',
  'legendreP', 'hermiteH', 'laguerreL', 'chebyshevT', 'chebyshevU', 'gegenbauerC', 'jacobiP',
  'hyp0F1', 'hyp1F1', 'hyp2F1', 'hypU',
  'ellipticK', 'ellipticE', 'ellipticF', 'ellipticPi', 'jacobiSN', 'jacobiCN', 'jacobiDN',
  'det', 'rank', 'cobweb',
]

/** 関数名の自動変換の表（名前 → 入れる LaTeX） */
export const FUNCTION_SHORTCUTS: Record<string, string> = {
  ...Object.fromEntries(OPERATOR_FUNCTIONS.map((f) => [f, `\\operatorname{${f}}`])),
  ...Object.fromEntries(BUILTIN_FUNCTIONS.map((f) => [f, `\\${f}`])),
}

/**
 * 1 つの要素（カーソルの直前の 1 つ。MathLive の getValue(k − 1, k) の LaTeX）が表す文字の並び。
 * 文字（x）・記号（<）はそのまま、\sin・\operatorname{sinc} は名前、自動変換で入る \theta などは打った名前（`back`）。
 * それ以外（分数・括弧など）は null（そこで並びが切れる）
 */
export function atomText(latex: string, back: Record<string, string>): string | null {
  const t = latex.trim()
  if (/^[A-Za-z0-9]$/.test(t) || /^[<>=!]$/.test(t)) return t
  const op = t.match(/^\\operatorname\{(?:\\mathrm\{)?([A-Za-z0-9]+)\}?\}$/)
  if (op) return op[1]
  const cmd = t.match(/^\\([A-Za-z]+)$/)
  if (cmd) {
    if (t in back) return back[t]
    if (BUILTIN_FUNCTIONS.includes(cmd[1])) return cmd[1]
  }
  return null
}

/** 自動変換の表から、入れる LaTeX → 打った名前（\theta → theta。引数の無い命令だけ） */
export function reverseShortcuts(table: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {}
  for (const [k, v] of Object.entries(table)) if (/^\\[A-Za-z]+$/.test(v) && /^[A-Za-z]+$/.test(k)) out[v] = k
  return out
}

/**
 * カーソルの直前の要素の列 `atoms`（古い順。LaTeX）の後ろの部分で、表 `table` の名前になるものを探す（長いものを優先）。
 * 2 つ以上の要素をまとめるときだけ（1 つなら、もう変わっているか、1 文字の変数）。見つかれば、まとめる要素の数と入れる LaTeX
 */
export function findShortcut(atoms: string[], table: Record<string, string>, back = reverseShortcuts(table)): { count: number; latex: string } | null {
  const names: string[] = []
  for (let k = atoms.length - 1; k >= 0; k--) {
    const n = atomText(atoms[k], back)
    if (n === null) break
    names.unshift(n)
  }
  for (let count = names.length; count >= 2; count--) {
    const word = names.slice(names.length - count).join('')
    if (Object.hasOwn(table, word)) {
      const latex = table[word]
      // もう同じものになっている（\sin の後に何も足していない）なら変えない
      if (count === 1) return null
      return { count, latex }
    }
  }
  return null
}
