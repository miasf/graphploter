// params.ts のテスト。実行: npm test
import { complexLatex, fitRotation, formatNumber, pairs, plainNumber, sequenceDefinitionName, sequenceInitialLatex } from './params.ts'

let failures = 0
function check(name: string, got: unknown, want: unknown) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    failures++
    console.log(`NG  ${name}\n    結果: ${JSON.stringify(got)}\n    期待: ${JSON.stringify(want)}`)
  }
}

// スライダーの値を式に書く形
check('刻み 0.25 の 0.25', formatNumber(0.25, 0.25), '0.25')
check('刻み 0.25 の 0.75', formatNumber(0.75, 0.25), '0.75')
check('最小値 0.05・刻み 0.1 の 0.15', formatNumber(0.15, 0.1, 0.05), '0.15')
check('丸め誤差を落とす', formatNumber(0.30000000000000004, 0.1), '0.3')
check('最小値 2.5・刻み 5 の 7.5', formatNumber(7.5, 5, 2.5), '7.5')
check('刻みより細かい値は刻みの桁まで', formatNumber(Math.PI, 0.01), '3.14')
check('-0 は 0', formatNumber(-0.0001, 0.1), '0')
// 小さい・大きい数は指数表記にしない（計算エンジンは 1e-8 の e をネイピア数と読む。#177）
check('小さい数', formatNumber(0.0000000101, 1e-10), '0.0000000101')
check('小さい負の数', formatNumber(-2e-8, 1e-10), '-0.00000002')
check('条件で止めた値', plainNumber(1.01e-8), '0.0000000101')
check('桁の多い小さい数', plainNumber(1.2345678901234e-15), '0.0000000000000012345678901234')
check('大きい数', plainNumber(1.5e21), '1500000000000000000000')
check('ふつうの数はそのまま', plainNumber(-3.25), '-3.25')
check('読み直すと同じ数', [1.01e-8, 7.000000000000001e-12, 1.5e21, 9.87654321e-300].every((v) => Number(plainNumber(v)) === v), true)
check('極小の刻み 1e-101', formatNumber(1e-101, 1e-101), plainNumber(1e-101))
check('非正規化数の刻み', Number(formatNumber(3e-309, 1e-309)), 3e-309)
check('極小の刻みの複素数', Number(complexLatex(2e-101, 0, 1e-101)), 2e-101)
check('小さい複素数', complexLatex(3e-9, -2e-9, 1e-10), '0.000000003-0.000000002i')

check('べき付きの数列定義', sequenceDefinitionName('a_(n+1)^(2)=a_(n)+1'), 'a')
check('名前付きの数列定義', sequenceDefinitionName('{alpha}_(n-2)^3=1'), 'alpha')
check('対数と偶奇の添字', sequenceDefinitionName('log (a_(2(n+1)+1)^(2))=a_(n)+a_1'), 'a')
check('固定の添字は数列定義ではない', sequenceDefinitionName('a_(3)^2=1'), null)
check('項を足した左辺は数列定義ではない', sequenceDefinitionName('a_(n)+b_(n)=1'), null)
check('数列の比較は定義ではない', sequenceDefinitionName('a_(n)^2>=1'), null)
check('括弧を並べた数列の積', sequenceDefinitionName('(a_(n+1))(a_(n))=6'), 'a')
check('数列の積と固定の項', sequenceDefinitionName('a_(2n)*a_(2n+2)^(2)*a_2=36'), 'a')
check('異なる数列の積', sequenceDefinitionName('a_(n+1)*b_(n)=6'), null)
check('分数の形の漸化式', sequenceDefinitionName('((a_(n+1))/(2))=sin a_(n)+1'), 'a')
check('手入力の除算の漸化式', sequenceDefinitionName('a_(n+1)/a_(n)=2'), 'a')
check('分数の外に足し算', sequenceDefinitionName('a_(n+1)/2+1=2'), null)
const hasSequence = (name: string) => name === 'a'
check('初項の添字を補う', sequenceInitialLatex('a1=1.7', hasSequence), 'a_{1}=1.7')
check('初項の右辺と空白を保つ', sequenceInitialLatex(' a12 = b3+1', hasSequence), ' a_{12} = b3+1')
check('対応する数列がない式はそのまま', sequenceInitialLatex('b2=4', hasSequence), 'b2=4')
check('すでに添字がある初項はそのまま', sequenceInitialLatex('a_1=1.7', hasSequence), 'a_1=1.7')

// 保存データの回転角を、今の並び（5次元の 10 個）にそろえる
const want10 = pairs(5).map(([i, j]) => (i === 0 && j === 2 ? 30 : i === 1 && j === 2 ? 45 : 0))
check('古い3次元の保存データ（3個）', fitRotation([0, 30, 45], [1, 2, 3], 3), { rotation: want10, center: [1, 2, 3, 0, 0] })
check('今の形はそのまま', fitRotation(want10, [1, 2, 3, 0, 0], 3), { rotation: want10, center: [1, 2, 3, 0, 0] })
check('壊れた値は 0', fitRotation(null, ['a'], 4).rotation.length, 10)
check('6次元', fitRotation([], [], 6).rotation.length, 15)

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('params.ts: すべて成功')
