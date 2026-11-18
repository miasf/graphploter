// autoNames.ts のテスト。実行: npm test
import { findShortcut } from './autoNames.ts'

let failures = 0
function check(name: string, got: unknown, want: unknown) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    failures++
    console.log(`NG  ${name}\n    結果: ${JSON.stringify(got)}\n    期待: ${JSON.stringify(want)}`)
  }
}

const TABLE: Record<string, string> = {
  sin: '\\sin', sinh: '\\sinh', sinc: '\\operatorname{sinc}', floor: '\\operatorname{floor}', theta: '\\theta',
  gamma: '\\Gamma', gammaP: '\\operatorname{gammaP}', '<=': '\\le', sqrt: '\\sqrt{#?}', asin: '\\operatorname{asin}', arcsin: '\\arcsin',
}
const f = (atoms: string[]) => findShortcut(atoms, TABLE)

check('s・i・n → \\sin', f(['z', '=', 's', 'i', 'n']), { count: 3, latex: '\\sin' })
check('x の後の sin', f(['x', 's', 'i', 'n']), { count: 3, latex: '\\sin' })
check('\\sin の後の h → sinh', f(['\\sin', 'h']), { count: 2, latex: '\\sinh' })
check('\\sin の後の c → sinc', f(['\\sin', 'c']), { count: 2, latex: '\\operatorname{sinc}' })
check('\\Gamma の後の P → gammaP', f(['\\Gamma', 'P']), { count: 2, latex: '\\operatorname{gammaP}' })
check('floor', f(['f', 'l', 'o', 'o', 'r']), { count: 5, latex: '\\operatorname{floor}' })
check('theta', f(['2', 't', 'h', 'e', 't', 'a']), { count: 5, latex: '\\theta' })
check('<=', f(['x', '<', '=']), { count: 2, latex: '\\le' })
check('長い名前を優先（arcsin）', f(['a', 'r', 'c', 's', 'i', 'n']), { count: 6, latex: '\\arcsin' })
check('途中の文字は変えない', f(['s', 'i']), null)
check('もう \\sin なら何もしない', f(['\\sin']), null)
check('\\sin の後の x は何もしない', f(['\\sin', 'x']), null)
check('分数などで切れる', f(['s', '\\frac{1}{2}', 'i', 'n']), null)
check('sqrt', f(['s', 'q', 'r', 't']), { count: 4, latex: '\\sqrt{#?}' })

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('autoNames.ts: すべて成功')
