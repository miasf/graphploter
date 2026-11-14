// 画面上の数式キーボードの「abc」タブ: 文字と、LaTeX で書ける数学記号（MathLive で表示できるもの）を全部、ページに分けて並べる。
//
// 1 ページは 10 個 × 4 段まで（スマホの幅でもはみ出さない数）。多い分類は「関係 1」「関係 2」のように分ける。
// 計算エンジンが読めない記号も入れる（入れると「… はまだ使えません」と出る。数式を書きとめる用・これから使えるようにする分）。

/** キー 1 つ。文字列なら LaTeX（表示も入力も同じ） */
export type SymbolKey = string | { latex: string; label?: string; insert?: string; tooltip?: string }

export type SymbolPage = {
  /** ページの名前（下の段に出す） */
  title: string
  /** 目次の項目（同じ項目のページは続けて並ぶ。目次からは最初のページへ飛ぶ） */
  section: string
  /** キー。文字のページは段ごと（rows）、記号のページは並べるだけ（keys。10 個ずつ段にする） */
  rows?: SymbolKey[][]
  keys?: SymbolKey[]
}

/** 空白で区切った LaTeX の一覧 */
const list = (s: string): string[] => s.split(/\s+/).filter((t) => t !== '')

/** 文字を書体の命令で包んだキーの一覧（\mathbb{A} など） */
const styled = (cmd: string, letters: string): SymbolKey[] =>
  [...letters].map((c) => ({ latex: `\\${cmd}{${c}}`, tooltip: `\\${cmd}{${c}}` }))

/** 部品（□ に入れる形）のキー。latex は表示、insert は入力（#0 は選んでいた部分、#? は空欄） */
const tpl = (insert: string, tooltip: string, label?: string): SymbolKey => ({
  latex: insert.replace(/#[0@?]/g, '\\placeholder{}'),
  insert,
  tooltip,
  ...(label ? { label } : {}),
})

/** 記号のページの 1 ページあたりのキーの数（10 個 × 4 段） */
const PAGE_SIZE = 40

const UPPER = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
const LOWER = 'abcdefghijklmnopqrstuvwxyz'

/**
 * 文字のページの 4 段目に置く、よく使う記号（ほかのタブにも置いてある）。
 * 不等号・等号の仲間と、座標・角度によく使うギリシャ文字
 */
export const COMMON_SYMBOLS: SymbolKey[] = [
  { latex: '<', tooltip: '小なり <' },
  { latex: '>', tooltip: '大なり >' },
  { latex: '\\le', tooltip: '小なりイコール \\le' },
  { latex: '\\ge', tooltip: '大なりイコール \\ge' },
  { latex: '\\ne', tooltip: '等しくない \\ne' },
  { latex: '\\approx', tooltip: 'ほぼ等しい \\approx' },
  { latex: '\\pm', tooltip: 'プラスマイナス \\pm' },
  { latex: '\\infty', tooltip: '無限大 \\infty' },
  { latex: '\\theta', tooltip: 'シータ \\theta' },
  { latex: '\\varphi', tooltip: 'ファイ \\varphi' },
]

const qwerty = (upper: boolean): SymbolKey[][] =>
  ['qwertyuiop', 'asdfghjkl', 'zxcvbnm'].map((row) => [...(upper ? row.toUpperCase() : row)])

/** 記号の名前（日本語）。無いものは LaTeX の命令だけを出す */
const NAMES: Record<string, string> = {
  '\\alpha': 'アルファ', '\\beta': 'ベータ', '\\gamma': 'ガンマ', '\\delta': 'デルタ', '\\epsilon': 'イプシロン',
  '\\varepsilon': 'イプシロン（別の形）', '\\zeta': 'ゼータ', '\\eta': 'イータ', '\\theta': 'シータ', '\\vartheta': 'シータ（別の形）',
  '\\iota': 'イオタ', '\\kappa': 'カッパ', '\\varkappa': 'カッパ（別の形）', '\\lambda': 'ラムダ', '\\mu': 'ミュー', '\\nu': 'ニュー',
  '\\xi': 'クシー', '\\omicron': 'オミクロン', '\\pi': 'パイ（円周率）', '\\varpi': 'パイ（別の形）', '\\rho': 'ロー',
  '\\varrho': 'ロー（別の形）', '\\sigma': 'シグマ', '\\varsigma': 'シグマ（語末の形）', '\\tau': 'タウ', '\\upsilon': 'ウプシロン',
  '\\phi': 'ファイ', '\\varphi': 'ファイ（別の形）', '\\chi': 'カイ', '\\psi': 'プサイ', '\\omega': 'オメガ',
  '\\digamma': 'ディガンマ', '\\koppa': 'コッパ', '\\coppa': 'コッパ（古い形）', '\\sampi': 'サンピ',
  '\\Gamma': 'ガンマ（大文字。ガンマ関数）', '\\Delta': 'デルタ（大文字）', '\\Theta': 'シータ（大文字）', '\\Lambda': 'ラムダ（大文字）',
  '\\Xi': 'クシー（大文字）', '\\Pi': 'パイ（大文字）', '\\Sigma': 'シグマ（大文字）', '\\Upsilon': 'ウプシロン（大文字）',
  '\\Phi': 'ファイ（大文字）', '\\Psi': 'プサイ（大文字）', '\\Omega': 'オメガ（大文字）',
  '<': '小なり', '>': '大なり', '\\le': '小なりイコール', '\\ge': '大なりイコール', '\\leqq': '小なりイコール（二重線）',
  '\\geqq': '大なりイコール（二重線）', '\\leqslant': '小なりイコール（斜め）', '\\geqslant': '大なりイコール（斜め）',
  '\\ne': '等しくない', '\\equiv': '合同・恒等的に等しい', '\\approx': 'ほぼ等しい', '\\sim': '相似・同程度', '\\simeq': 'ほぼ等しい',
  '\\cong': '合同・同型', '\\propto': '比例', '\\ll': '非常に小さい', '\\gg': '非常に大きい', '\\doteq': '近似的に等しい',
  '\\coloneq': '定義（:=）', '\\triangleq': '定義', '\\eqdef': '定義', '\\perp': '垂直', '\\parallel': '平行', '\\mid': '割り切る',
  '\\nmid': '割り切らない', '\\models': '充足する', '\\vdash': '証明できる',
  '\\pm': 'プラスマイナス', '\\mp': 'マイナスプラス', '\\times': '掛け算（×）', '\\div': '割り算（÷）', '\\cdot': '掛け算（・）',
  '\\ast': 'アスタリスク', '\\star': '星', '\\circ': '合成（∘）', '\\bullet': '黒丸', '\\oplus': '直和', '\\otimes': 'テンソル積',
  '\\odot': '丸の中の点', '\\ominus': '丸の中のマイナス', '\\cap': '共通部分', '\\cup': '和集合', '\\setminus': '差集合',
  '\\wedge': 'かつ・外積', '\\vee': 'または', '\\in': '属する', '\\notin': '属さない', '\\ni': '含む（元として）',
  '\\subset': '部分集合', '\\supset': '含む（集合として）', '\\subseteq': '部分集合（等しくてもよい）', '\\supseteq': '含む（等しくてもよい）',
  '\\subsetneq': '真部分集合', '\\supsetneq': '真に含む', '\\emptyset': '空集合', '\\varnothing': '空集合（別の形）',
  '\\forall': '任意の（全称）', '\\exists': '存在する', '\\nexists': '存在しない', '\\neg': '否定', '\\land': 'かつ', '\\lor': 'または',
  '\\implies': 'ならば', '\\impliedby': 'の必要条件', '\\iff': '同値', '\\therefore': 'ゆえに', '\\because': 'なぜならば',
  '\\to': '写像・極限（→）', '\\mapsto': '元の対応（↦）', '\\Rightarrow': 'ならば（⇒）', '\\Leftrightarrow': '同値（⇔）',
  '\\infty': '無限大', '\\partial': '偏微分', '\\nabla': 'ナブラ', '\\hbar': 'エイチバー（換算プランク定数）', '\\ell': '小文字のエル',
  '\\aleph': 'アレフ', '\\beth': 'ベート', '\\gimel': 'ギメル', '\\daleth': 'ダレット', '\\Re': '実部', '\\Im': '虚部',
  '\\wp': 'ワイエルシュトラスのペー', '\\imath': '点の無い i', '\\jmath': '点の無い j', '\\angle': '角', '\\degree': '度（°）',
  '\\prime': 'プライム', '\\ldots': '点々（下）', '\\cdots': '点々（中央）', '\\vdots': '点々（縦）', '\\ddots': '点々（斜め）',
  '\\complement': '補集合',
}

/** キーにする（名前があれば、名前と LaTeX の命令を出す） */
function named(latex: string): SymbolKey {
  const name = NAMES[latex]
  return { latex, tooltip: name ? `${name}\n${latex}` : latex }
}

/** 記号の分類（並べる順） */
const SYMBOL_GROUPS: { title: string; section: string; keys: SymbolKey[] }[] = [
  {
    title: '書体・文字',
    section: '書体',
    keys: [
      tpl('\\mathbb{#0}', '黒板太字 \\mathbb{…}（ℝ, ℤ など）', '𝔸'),
      tpl('\\mathcal{#0}', '筆記体 \\mathcal{…}', '𝒜'),
      tpl('\\mathscr{#0}', 'スクリプト体 \\mathscr{…}', '𝒜'),
      tpl('\\mathfrak{#0}', 'フラクトゥール \\mathfrak{…}', '𝔄'),
      tpl('\\mathbf{#0}', '太字 \\mathbf{…}', '𝐀'),
      tpl('\\boldsymbol{#0}', '太字（記号も） \\boldsymbol{…}', '𝜶'),
      tpl('\\mathrm{#0}', '立体 \\mathrm{…}', 'A'),
      tpl('\\mathit{#0}', '斜体 \\mathit{…}', '𝐴'),
      tpl('\\mathsf{#0}', 'サンセリフ \\mathsf{…}', '𝖠'),
      tpl('\\mathtt{#0}', '等幅 \\mathtt{…}', '𝙰'),
      tpl('\\text{#0}', '文章 \\text{…}', 'Aa'),
      tpl('\\operatorname{#0}', '関数名 \\operatorname{…}', 'op'),
      ...list(String.raw`\aleph \beth \gimel \daleth \hbar \hslash \ell \wp \Re \Im \imath \jmath \eth \mho \Finv \Game \Bbbk
        \partial \nabla \infty \emptyset \varnothing \complement \circledS \Laplace \laplace \differencedelta \imageof \originalof`).map(named),
    ],
  },
  {
    title: '等号・不等号',
    section: '関係',
    keys: list(String.raw`= \ne < > \le \ge \leqq \geqq \leqslant \geqslant \ll \gg \lll \ggg \llless \gggtr \lessdot \gtrdot
      \lesssim \gtrsim \lessapprox \gtrapprox \lessgtr \gtrless \lesseqgtr \gtreqless \lesseqqgtr \gtreqqless
      \eqslantless \eqslantgtr \nless \ngtr \nleq \ngeq \nleqq \ngeqq \nleqslant \ngeqslant \lneq \gneq \lneqq \gneqq
      \lvertneqq \gvertneqq \lnsim \gnsim \lnapprox \gnapprox`).map(named),
  },
  {
    title: '似ている・定義',
    section: '関係',
    keys: list(String.raw`\equiv \approx \approxeq \sim \nsim \simeq \backsim \backsimeq \eqsim \thicksim \thickapprox \cong \ncong
      \propto \varpropto \asymp \doteq \Doteq \doteqdot \fallingdotseq \risingdotseq \bumpeq \Bumpeq \circeq \eqcirc
      \triangleq \coloneq \coloneqq \colonequals \eqdef \measeq \questeq \between \pitchfork \smile \frown \smallsmile \smallfrown`).map(named),
  },
  {
    title: '順序・論理の関係',
    section: '関係',
    keys: list(String.raw`\prec \succ \preceq \succeq \preccurlyeq \succcurlyeq \precsim \succsim \precapprox \succapprox
      \curlyeqprec \curlyeqsucc \nprec \nsucc \npreceq \nsucceq \precneqq \succneqq \precnsim \succnsim \precnapprox \succnapprox
      \perp \parallel \nparallel \shortparallel \nshortparallel \mid \nmid \shortmid \nshortmid \divides
      \models \vdash \dashv \vDash \Vdash \Vvdash \nvdash \nvDash \nVdash \nVDash`).map(named),
  },
  {
    title: '三角の関係',
    section: '関係',
    keys: list(String.raw`\lhd \rhd \unlhd \unrhd \triangleleft \triangleright \trianglelefteq \trianglerighteq
      \ntriangleleft \ntriangleright \ntrianglelefteq \ntrianglerighteq \vartriangleleft \vartriangleright
      \bowtie \Join \multimap \leftslice \rightslice`).map(named),
  },
  {
    title: '演算',
    section: '演算',
    keys: list(String.raw`+ - \pm \mp \times \div \cdot \ast \star \circ \bullet \centerdot \dotplus \divideontimes
      \oplus \ominus \otimes \oslash \odot \circledast \circledcirc \circleddash \bigcirc
      \boxplus \boxminus \boxtimes \boxdot \boxast \boxbar \boxbox \boxbslash \boxcircle \boxslash
      \cap \cup \Cap \Cup \sqcap \sqcup \uplus \setminus \smallsetminus \backslash \sslash`).map(named),
  },
  {
    title: '演算（論理など）',
    section: '演算',
    keys: list(String.raw`\wedge \vee \curlywedge \curlyvee \barwedge \veebar \doublebarwedge \And \nor \amalg \wr
      \diamond \bigtriangleup \bigtriangledown \ltimes \rtimes \leftthreetimes \rightthreetimes \intercal
      \dagger \ddagger \dag \ddag \interleave \biginterleave \fatsemi`).map(named),
  },
  {
    title: '集合・論理',
    section: '集合・論理',
    keys: [
      ...list(String.raw`\in \notin \ni \owns \backepsilon \subset \supset \subseteq \supseteq \subsetneq \supsetneq \varsubsetneq \varsupsetneq
        \subseteqq \supseteqq \subsetneqq \supsetneqq \varsubsetneqq \varsupsetneqq \nsubset \nsupset \nsubseteq \nsupseteq
        \nsubseteqq \nsupseteqq \Subset \Supset \sqsubset \sqsupset \sqsubseteq \sqsupseteq`).map(named),
      ...['N', 'Z', 'Q', 'R', 'C'].map((c) => ({ latex: `\\mathbb{${c}}`, tooltip: `\\mathbb{${c}}` })),
      ...list(String.raw`\emptyset \varnothing \complement`).map(named),
      tpl('\\left\\{#0\\right\\}', '集合の括弧 \\{ … \\}', '{ }'),
      tpl('\\left\\{#0\\mid#?\\right\\}', '条件で書いた集合 \\{ … \\mid … \\}', '{|}'),
    ],
  },
  {
    title: '論理',
    section: '集合・論理',
    keys: list(String.raw`\forall \exists \nexists \neg \lnot \land \lor \implies \impliedby \iff \Rightarrow \Leftarrow
      \Leftrightarrow \biconditional \roundimplies \therefore \because \top \bot \lightning \checkmark \maltese \S`).map(named),
  },
  {
    title: '矢印',
    section: '矢印',
    keys: list(String.raw`\to \gets \rightarrow \leftarrow \leftrightarrow \Rightarrow \Leftarrow \Leftrightarrow
      \longrightarrow \longleftarrow \longleftrightarrow \Longrightarrow \Longleftarrow \Longleftrightarrow
      \uparrow \downarrow \updownarrow \Uparrow \Downarrow \Updownarrow \nearrow \searrow \swarrow \nwarrow
      \mapsto \longmapsto \mapsfrom \longmapsfrom \Mapsfrom \Longmapsfrom \MapsTo \Longmapsto
      \hookrightarrow \hookleftarrow \twoheadrightarrow \twoheadleftarrow \rightarrowtail \leftarrowtail`).map(named),
  },
  {
    title: '矢印（その他）',
    section: '矢印',
    keys: [
      ...list(String.raw`\nrightarrow \nleftarrow \nleftrightarrow \nRightarrow \nLeftarrow \nLeftrightarrow
        \rightrightarrows \leftleftarrows \rightleftarrows \leftrightarrows \upuparrows \downdownarrows \Rrightarrow \Lleftarrow
        \rightharpoonup \rightharpoondown \leftharpoonup \leftharpoondown \rightleftharpoons \leftrightharpoons
        \upharpoonleft \upharpoonright \downharpoonleft \downharpoonright \restriction
        \curvearrowright \curvearrowleft \circlearrowright \circlearrowleft \looparrowright \looparrowleft \Rsh \Lsh
        \leadsto \rightsquigarrow \leftrightsquigarrow \dashrightarrow \dashleftarrow`).map(named),
    ],
  },
  {
    title: '矢印（三角・縦棒）',
    section: '矢印',
    keys: [
      ...list(String.raw`\rightarrowtriangle \leftarrowtriangle \leftrightarrowtriangle \twoheadrightarrowtail
        \nvrightarrow \nvrightarrowtail \nvtwoheadrightarrow \nvtwoheadrightarrowtail`).map(named),
      tpl('\\xrightarrow{#?}', '上に文字のある右矢印 \\xrightarrow{…}', '→ᵃ'),
      tpl('\\xleftarrow{#?}', '上に文字のある左矢印 \\xleftarrow{…}', 'ᵃ←'),
      tpl('\\xRightarrow{#?}', '上に文字のある右矢印（二重） \\xRightarrow{…}', '⇒ᵃ'),
      tpl('\\xleftrightarrow{#?}', '上に文字のある両矢印 \\xleftrightarrow{…}', '↔ᵃ'),
      tpl('\\xmapsto{#?}', '上に文字のある ↦ \\xmapsto{…}', '↦ᵃ'),
      tpl('\\xhookrightarrow{#?}', '上に文字のある ↪ \\xhookrightarrow{…}', '↪ᵃ'),
    ],
  },
  {
    title: '和・積分',
    section: '和・積分',
    keys: [
      tpl('\\sum_{#?}^{#?}#?', '和 \\sum', '∑'),
      tpl('\\prod_{#?}^{#?}#?', '積 \\prod', '∏'),
      tpl('\\coprod_{#?}^{#?}#?', '余積 \\coprod', '∐'),
      tpl('\\int_{#?}^{#?}#?\\,d#?', '定積分 \\int', '∫'),
      tpl('\\iint_{#?}#?', '2 重積分 \\iint', '∬'),
      tpl('\\iiint_{#?}#?', '3 重積分 \\iiint', '∭'),
      tpl('\\oint_{#?}#?', '周回積分 \\oint', '∮'),
      tpl('\\oiint_{#?}#?', '閉曲面の積分 \\oiint', '∯'),
      tpl('\\oiiint_{#?}#?', '閉じた体積の積分 \\oiiint', '∰'),
      tpl('\\intop_{#?}^{#?}#?', '積分（上下に添字） \\intop'),
      tpl('\\smallint', '小さい積分 \\smallint', '∫'),
      tpl('\\bigcup_{#?}^{#?}#?', '和集合（大） \\bigcup', '⋃'),
      tpl('\\bigcap_{#?}^{#?}#?', '共通部分（大） \\bigcap', '⋂'),
      tpl('\\bigsqcup_{#?}#?', '直和（大） \\bigsqcup', '⨆'),
      tpl('\\bigvee_{#?}#?', 'または（大） \\bigvee', '⋁'),
      tpl('\\bigwedge_{#?}#?', 'かつ（大） \\bigwedge', '⋀'),
      tpl('\\bigoplus_{#?}#?', '直和（大） \\bigoplus', '⨁'),
      tpl('\\bigotimes_{#?}#?', 'テンソル積（大） \\bigotimes', '⨂'),
      tpl('\\bigodot_{#?}#?', '\\bigodot', '⨀'),
      tpl('\\biguplus_{#?}#?', '\\biguplus', '⨄'),
      tpl('\\lim_{#?\\to#?}#?', '極限 \\lim', 'lim'),
      tpl('\\limsup_{#?}#?', '上極限 \\limsup', 'lim sup'),
      tpl('\\liminf_{#?}#?', '下極限 \\liminf', 'lim inf'),
      tpl('\\sup_{#?}#?', '上限 \\sup', 'sup'),
      tpl('\\inf_{#?}#?', '下限 \\inf', 'inf'),
      tpl('\\max_{#?}#?', '最大 \\max', 'max'),
      tpl('\\min_{#?}#?', '最小 \\min', 'min'),
      tpl('\\frac{d#?}{d#?}', '微分 d/dx', 'd/dx'),
      tpl('\\frac{\\partial#?}{\\partial#?}', '偏微分 ∂/∂x', '∂/∂x'),
      tpl('\\frac{d^{#?}#?}{d#?^{#?}}', '高階の微分', 'dⁿ/dxⁿ'),
      ...list(String.raw`\partial \nabla \Delta`).map(named),
      ...['det', 'dim', 'ker', 'deg', 'gcd', 'hom', 'Pr', 'arg'].map((f) => tpl(`\\${f}#?`, `\\${f}`, f)),
      tpl('\\bmod', '余り \\bmod', 'mod'),
      tpl('\\pmod{#?}', '合同式の法 \\pmod{…}', '(mod)'),
    ],
  },
  {
    title: '括弧・分数・根号',
    section: '括弧・分数',
    keys: [
      tpl('\\left(#0\\right)', '丸括弧', '( )'),
      tpl('\\left[#0\\right]', '角括弧', '[ ]'),
      tpl('\\left\\{#0\\right\\}', '波括弧', '{ }'),
      tpl('\\left\\langle#0\\right\\rangle', '山括弧（内積）', '⟨ ⟩'),
      tpl('\\left|#0\\right|', '絶対値', '| |'),
      tpl('\\left\\Vert#0\\right\\Vert', 'ノルム', '‖ ‖'),
      tpl('\\left\\lfloor#0\\right\\rfloor', '床関数', '⌊ ⌋'),
      tpl('\\left\\lceil#0\\right\\rceil', '天井関数', '⌈ ⌉'),
      tpl('\\left\\llbracket#0\\right\\rrbracket', '二重の角括弧', '⟦ ⟧'),
      tpl('\\left\\lgroup#0\\right\\rgroup', '\\lgroup … \\rgroup', '⟮ ⟯'),
      tpl('\\left(#0\\right]', '半開区間 ( ]', '( ]'),
      tpl('\\left[#0\\right)', '半開区間 [ )', '[ )'),
      tpl('\\left\\lmoustache#0\\right\\rmoustache', '\\lmoustache … \\rmoustache', '⎰ ⎱'),
      tpl('\\ulcorner#0\\urcorner', '\\ulcorner … \\urcorner', '⌜ ⌝'),
      tpl('\\llcorner#0\\lrcorner', '\\llcorner … \\lrcorner', '⌞ ⌟'),
      tpl('\\Lbag#0\\Rbag', '\\Lbag … \\Rbag', '⟅ ⟆'),
      tpl('\\frac{#0}{#?}', '分数 \\frac', 'a/b'),
      tpl('\\dfrac{#0}{#?}', '大きい分数 \\dfrac', 'a/b'),
      tpl('\\tfrac{#0}{#?}', '小さい分数 \\tfrac', 'a/b'),
      tpl('\\cfrac{#0}{#?}', '連分数 \\cfrac', 'a/b…'),
      tpl('\\binom{#0}{#?}', '二項係数 \\binom', '(ⁿₖ)'),
      tpl('\\sqrt{#0}', '平方根', '√'),
      tpl('\\sqrt[#?]{#0}', 'n 乗根', 'ⁿ√'),
      tpl('#@^{#?}', '上付き（べき乗）', 'xⁿ'),
      tpl('#@_{#?}', '下付き（添字）', 'xₙ'),
      tpl('#@_{#?}^{#?}', '上付きと下付き', 'xₙᵐ'),
      tpl('\\overset{#?}{#0}', '上に置く \\overset', 'ẋ'),
      tpl('\\underset{#?}{#0}', '下に置く \\underset', 'x̣'),
      tpl('\\stackrel{#?}{#0}', '上に置く（関係） \\stackrel', '=ᵃ'),
      tpl('\\overbrace{#0}^{#?}', '上の波括弧 \\overbrace', '⏞'),
      tpl('\\underbrace{#0}_{#?}', '下の波括弧 \\underbrace', '⏟'),
      tpl('\\boxed{#0}', '囲み \\boxed', '▭'),
      tpl('\\cancel{#0}', '打ち消し \\cancel', '╱'),
      tpl('\\begin{pmatrix}#0 & #? \\\\ #? & #?\\end{pmatrix}', '行列（丸括弧）', '(⋮)'),
      tpl('\\begin{bmatrix}#0 & #? \\\\ #? & #?\\end{bmatrix}', '行列（角括弧）', '[⋮]'),
      tpl('\\begin{vmatrix}#0 & #? \\\\ #? & #?\\end{vmatrix}', '行列式', '|⋮|'),
      tpl('\\begin{cases}#0 & #? \\\\ #? & #?\\end{cases}', '場合分け', '{⋮'),
    ],
  },
  {
    title: 'アクセント',
    section: 'アクセント',
    keys: [
      tpl('\\hat{#@}', '\\hat', 'â'),
      tpl('\\widehat{#0}', '\\widehat', 'âb'),
      tpl('\\check{#@}', '\\check', 'ǎ'),
      tpl('\\widecheck{#0}', '\\widecheck', 'ǎb'),
      tpl('\\tilde{#@}', '\\tilde', 'ã'),
      tpl('\\widetilde{#0}', '\\widetilde', 'ãb'),
      tpl('\\utilde{#0}', '下のチルダ \\utilde'),
      tpl('\\acute{#@}', '\\acute', 'á'),
      tpl('\\grave{#@}', '\\grave', 'à'),
      tpl('\\breve{#@}', '\\breve', 'ă'),
      tpl('\\dot{#@}', '時間微分 \\dot', 'ȧ'),
      tpl('\\ddot{#@}', '2 階の時間微分 \\ddot', 'ä'),
      tpl('\\mathring{#@}', '\\mathring', 'å'),
      tpl('\\bar{#@}', '\\bar', 'ā'),
      tpl('\\overline{#0}', '上線（共役・閉包） \\overline', 'āb'),
      tpl('\\underline{#0}', '下線 \\underline', 'a̲'),
      tpl('\\vec{#@}', 'ベクトル \\vec', 'a⃗'),
      tpl('\\overrightarrow{#0}', '\\overrightarrow'),
      tpl('\\overleftarrow{#0}', '\\overleftarrow'),
      tpl('\\overleftrightarrow{#0}', '\\overleftrightarrow'),
      tpl('\\underrightarrow{#0}', '\\underrightarrow'),
      tpl('\\underleftarrow{#0}', '\\underleftarrow'),
      tpl('\\overarc{#0}', '弧 \\overarc', '⌒'),
      tpl('\\overparen{#0}', '\\overparen', '⏜'),
      tpl('\\underparen{#0}', '\\underparen', '⏝'),
      tpl("#@'", 'プライム（微分）', "x'"),
      tpl("#@''", '2 つのプライム', "x''"),
      tpl('#@^{\\circ}', '度 °', 'x°'),
      tpl('#@^{\\ast}', 'アスタリスク（共役など）', 'x*'),
      tpl('#@^{\\dagger}', 'ダガー（随伴）', 'x†'),
      tpl('#@^{T}', '転置', 'xᵀ'),
      tpl('#@^{-1}', '逆', 'x⁻¹'),
      ...list(String.raw`\prime \doubleprime \backprime \backdoubleprime \degree`).map(named),
    ],
  },
  {
    title: '点・区切り',
    section: '点・図形',
    keys: list(String.raw`\ldots \cdots \vdots \ddots \ldotp \cdotp \mathellipsis \colon \Colon : ; ! ? ' |
      \vert \Vert \lvert \rvert \lVert \rVert \mvert \mVert \backslash / \langle \rangle \lfloor \rfloor \lceil \rceil
      \lbrace \rbrace \lbrack \rbrack \lparen \rparen \llbracket \rrbracket \lgroup \rgroup`).map(named),
  },
  {
    title: '図形・その他',
    section: '点・図形',
    keys: list(String.raw`\angle \measuredangle \sphericalangle \triangle \triangledown \vartriangle \blacktriangle \blacktriangledown
      \blacktriangleleft \blacktriangleright \square \Box \blacksquare \Diamond \lozenge \blacklozenge \bigstar \diagup
      \parallelogram \talloblong \surd \top \bot \clubsuit \diamondsuit \heartsuit \spadesuit \flat \natural \sharp
      \circledR \yen \pounds \euro \% \# \& \$ \_ \Yup`).map(named),
  },
]

/** abc タブのページ（文字のページ → 記号のページ） */
export const SYMBOL_PAGES: SymbolPage[] = [
  { title: 'abc', section: 'abc', rows: [...qwerty(false), COMMON_SYMBOLS] },
  { title: 'ABC', section: 'abc', rows: [...qwerty(true), COMMON_SYMBOLS] },
  {
    title: 'ギリシャ文字', section: 'ギリシャ',
    keys: list(String.raw`\alpha \beta \gamma \delta \epsilon \varepsilon \zeta \eta \theta \vartheta \iota \kappa \varkappa
      \lambda \mu \nu \xi \omicron \pi \varpi \rho \varrho \sigma \varsigma \tau \upsilon \phi \varphi \chi \psi \omega
      \digamma \koppa \coppa \sampi`).map(named),
  },
  {
    title: 'ギリシャ大文字', section: 'ギリシャ',
    keys: [
      ...list(String.raw`\Gamma \Delta \Theta \Lambda \Xi \Pi \Sigma \Upsilon \Phi \Psi \Omega \Koppa \Coppa \Sampi`).map(named),
      // 斜体の大文字（変数として使うとき）
      ...list(String.raw`\Gamma \Delta \Theta \Lambda \Xi \Pi \Sigma \Upsilon \Phi \Psi \Omega`).map((g) => ({
        latex: `\\mathit{${g}}`,
        tooltip: `${NAMES[g] ?? g}（斜体）\n\\mathit{${g}}`,
      })),
    ],
  },
  { title: '黒板太字 𝔸', section: '𝔸 𝒜 𝔄 𝐀', keys: [...styled('mathbb', UPPER), { latex: '\\Bbbk', tooltip: '\\Bbbk' }, ...styled('mathbb', LOWER).slice(0, 13)] },
  { title: '黒板太字 𝕒', section: '𝔸 𝒜 𝔄 𝐀', keys: [...styled('mathbb', LOWER).slice(13), ...styled('mathbb', '0123456789')] },
  { title: '筆記体 𝒜', section: '𝔸 𝒜 𝔄 𝐀', keys: styled('mathcal', UPPER) },
  { title: 'スクリプト体 𝒜', section: '𝔸 𝒜 𝔄 𝐀', keys: styled('mathscr', UPPER) },
  { title: 'フラクトゥール 𝔄', section: '𝔸 𝒜 𝔄 𝐀', keys: styled('mathfrak', UPPER) },
  { title: 'フラクトゥール 𝔞', section: '𝔸 𝒜 𝔄 𝐀', keys: styled('mathfrak', LOWER) },
  { title: '太字 𝐀', section: '𝔸 𝒜 𝔄 𝐀', keys: [...styled('mathbf', UPPER), ...styled('mathbf', LOWER).slice(0, 14)] },
  { title: '太字 𝐚', section: '𝔸 𝒜 𝔄 𝐀', keys: [...styled('mathbf', LOWER).slice(14), ...list(String.raw`\alpha \beta \gamma \delta \theta \lambda \mu \pi \sigma \phi \psi \omega \nabla`).map((g) => ({ latex: `\\boldsymbol{${g}}`, tooltip: `\\boldsymbol{${g}}` }))] },
  // 記号の分類を、10 個 × 4 段ずつのページに分ける
  // （40 個を超える分類は、同じくらいの数ずつに分ける。最後のページに 1〜2 個だけ残らないように）
  ...SYMBOL_GROUPS.flatMap((g) => {
    const n = Math.ceil(g.keys.length / PAGE_SIZE)
    const size = Math.ceil(g.keys.length / n)
    return Array.from({ length: n }, (_, k) => ({
      title: n > 1 ? `${g.title} ${k + 1}` : g.title,
      section: g.section,
      keys: g.keys.slice(k * size, (k + 1) * size),
    }))
  }),
]
