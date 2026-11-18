// 名前に使える文字（計画 docs/plan-math-notation.md の M13）。LaTeX の命令 ↔ 計算エンジンの名前 ↔ 画面の表示の文字の表。
//
// 計算エンジンでは、2 文字以上の名前は `{名前}` と書く（`{alpha}`・`{hat_x}`・`{cal_A}`）。
// θ・φ・ρ・σ・τ・μ・ν は座標系の名前（計画 S38）として、今までどおり `theta` などの名前のまま（保存データを変えない）。
// π（定数）・Γ・ζ・ψ（関数）は名前にしない

/** ギリシャ文字の小文字（LaTeX の命令名 → 文字）。δ は下付きがあれば差分なので、latex.ts が別に扱う（表示用に下の表へ） */
const GREEK_LOWER: Record<string, string> = {
  alpha: 'α', beta: 'β', gamma: 'γ', epsilon: 'ϵ', varepsilon: 'ε', eta: 'η', iota: 'ι', kappa: 'κ', varkappa: 'ϰ',
  lambda: 'λ', xi: 'ξ', omicron: 'ο', varpi: 'ϖ', upsilon: 'υ', chi: 'χ', omega: 'ω', digamma: 'ϝ',
}
/** ギリシャ文字の大文字（Γ は関数、Π・Σ・Δ は別の意味があるので、名前として使えるのは次のもの） */
const GREEK_UPPER: Record<string, string> = {
  Theta: 'Θ', Lambda: 'Λ', Xi: 'Ξ', Phi: 'Φ', Psi: 'Ψ', Omega: 'Ω', Upsilon: 'Υ', Pi: 'Π', Sigma: 'Σ',
}
/** 文字の形の記号 */
const LETTERLIKE: Record<string, string> = {
  ell: 'ℓ', hbar: 'ℏ', hslash: 'ℏ', imath: 'ı', jmath: 'ȷ', aleph: 'ℵ', beth: 'ℶ', gimel: 'ℷ', daleth: 'ℸ', wp: '℘', eth: 'ð', mho: '℧',
}
/** アクセント（LaTeX の命令名 → 合成用の文字） */
const ACCENTS: Record<string, string> = {
  hat: '̂', widehat: '̂', tilde: '̃', widetilde: '̃', check: '̌', acute: '́', grave: '̀',
  breve: '̆', mathring: '̊',
}
/** 書体（LaTeX の命令名 → 名前の頭） */
const FONTS: Record<string, string> = { mathcal: 'cal', mathscr: 'scr', mathfrak: 'frak', mathsf: 'sf', mathtt: 'tt', mathbb: 'bb' }

/** 名前の頭 → 書体の Unicode の始まり（大文字 A・小文字 a）と、例外の文字 */
const FONT_UNICODE: Record<string, { upper: number; lower: number; except: Record<string, string> }> = {
  cal: { upper: 0x1d49c, lower: 0x1d4b6, except: { B: 'ℬ', E: 'ℰ', F: 'ℱ', H: 'ℋ', I: 'ℐ', L: 'ℒ', M: 'ℳ', R: 'ℛ', e: 'ℯ', g: 'ℊ', o: 'ℴ' } },
  scr: { upper: 0x1d49c, lower: 0x1d4b6, except: { B: 'ℬ', E: 'ℰ', F: 'ℱ', H: 'ℋ', I: 'ℐ', L: 'ℒ', M: 'ℳ', R: 'ℛ', e: 'ℯ', g: 'ℊ', o: 'ℴ' } },
  frak: { upper: 0x1d504, lower: 0x1d51e, except: { C: 'ℭ', H: 'ℌ', I: 'ℑ', R: 'ℜ', Z: 'ℨ' } },
  bb: { upper: 0x1d538, lower: 0x1d552, except: { C: 'ℂ', H: 'ℍ', N: 'ℕ', P: 'ℙ', Q: 'ℚ', R: 'ℝ', Z: 'ℤ' } },
  sf: { upper: 0x1d5a0, lower: 0x1d5ba, except: {} },
  tt: { upper: 0x1d670, lower: 0x1d68a, except: {} },
}

/** 1 文字（英字・ギリシャ文字の命令）の LaTeX と表示 */
function letter(s: string): { latex: string; text: string } | null {
  if (/^[A-Za-z]$/.test(s)) return { latex: s, text: s }
  const g = GREEK_LOWER[s] ?? GREEK_UPPER[s] ?? { theta: 'θ', phi: 'φ', rho: 'ρ', sigma: 'σ', tau: 'τ', mu: 'μ', nu: 'ν' }[s]
  return g ? { latex: `\\${s}`, text: g } : null
}

/**
 * LaTeX の命令（`\alpha`・`\hat` など。`arg` は `\hat{x}` の中身の文字）から、計算エンジンの名前。名前でなければ null。
 * `\mathbb{R}` などの集合は、呼ぶ側で先に処理する
 */
export function engineNameOf(cmd: string, arg?: string): string | null {
  if (cmd in GREEK_LOWER || cmd in GREEK_UPPER || cmd in LETTERLIKE) return cmd
  if (arg !== undefined && cmd in ACCENTS) {
    const a = arg.replace(/^\\/, '')
    if (letter(a)) return `${cmd === 'widehat' ? 'hat' : cmd === 'widetilde' ? 'tilde' : cmd}_${a}`
  }
  if (arg !== undefined && cmd in FONTS && /^[A-Za-z]$/.test(arg)) return `${FONTS[cmd]}_${arg}`
  return null
}

/** 計算エンジンの名前の LaTeX（`alpha` → `\alpha`、`hat_x` → `\hat{x}`、`cal_A` → `\mathcal{A}`）。表に無ければ null */
export function nameLatex(name: string): string | null {
  if (name === 'delta') return '\\delta'
  if (name in GREEK_LOWER || name in GREEK_UPPER || name in LETTERLIKE) return `\\${name}`
  const m = /^([a-z]+)_(\\?[A-Za-z]+)$/.exec(name)
  if (!m) return null
  // ベクトルの名前（\vec{F}・\mathbf{F}・\mathbb{F} はどれもこの名前）
  if (m[1] === 'vec') return /^[A-Za-z]\w*$/.test(m[2]) ? `\\vec{${m[2]}}` : null
  if (m[1] in ACCENTS) {
    const l = letter(m[2])
    return l ? `\\${m[1]}{${l.latex}}` : null
  }
  const font = Object.entries(FONTS).find(([, v]) => v === m[1])
  return font && /^[A-Za-z]$/.test(m[2]) ? `\\${font[0]}{${m[2]}}` : null
}

/** 計算エンジンの名前の、画面に出す文字（`alpha` → α、`hat_x` → x̂、`cal_A` → 𝒜）。表に無ければ null */
export function nameText(name: string): string | null {
  if (name === 'delta') return 'δ'
  if (name in GREEK_LOWER) return GREEK_LOWER[name]
  if (name in GREEK_UPPER) return GREEK_UPPER[name]
  if (name in LETTERLIKE) return LETTERLIKE[name]
  const m = /^([a-z]+)_([A-Za-z]+)$/.exec(name)
  if (!m) return null
  // ベクトルの名前は「ベクトル F」と出す（矢印を重ねる文字 U+20D7 や太字の数学用文字は、スマホの字体によって □ になるため）
  if (m[1] === 'vec') return /^[A-Za-z]$/.test(m[2]) ? `ベクトル ${m[2]}` : null
  if (m[1] in ACCENTS) {
    const l = letter(m[2])
    return l ? l.text + ACCENTS[m[1]] : null
  }
  const f = FONT_UNICODE[m[1]]
  if (!f || !/^[A-Za-z]$/.test(m[2])) return null
  const c = m[2]
  if (c in f.except) return f.except[c]
  const base = c >= 'a' ? f.lower + c.charCodeAt(0) - 97 : f.upper + c.charCodeAt(0) - 65
  return String.fromCodePoint(base)
}

/** 名前に使える命令か（latex.ts が、まだ使えない命令と分けるのに使う） */
export const isNameCommand = (cmd: string) => cmd in GREEK_LOWER || cmd in GREEK_UPPER || cmd in LETTERLIKE
export const isAccentCommand = (cmd: string) => cmd in ACCENTS
export const isFontCommand = (cmd: string) => cmd in FONTS
