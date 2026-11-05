// 媒介変数表示の式 "(a, b, c)" を、成分ごとに分けたりつなげたりする。
// 縦に並べた入力欄（x = …, y = …, z = …）で編集するために使う。

/**
 * 式全体が1組の括弧 (…, …) で、括弧の一番外側にカンマがあれば、成分の文字列の配列を返す。
 * そうでなければ null。例: "(cos t, sin t, t)" → ["cos t", "sin t", "t"]
 */
export function splitTuple(source: string): string[] | null {
  const s = source.trim()
  if (!s.startsWith('(') || !s.endsWith(')')) return null
  const parts: string[] = []
  let depth = 0
  let start = 1
  for (let i = 0; i < s.length; i++) {
    const c = s[i]
    if (c === '(') depth++
    else if (c === ')') {
      depth--
      // 最初の括弧が途中で閉じる場合（例: "(x+1)(y+1)"）は組ではない
      if (depth === 0 && i !== s.length - 1) return null
    } else if (c === ',' && depth === 1) {
      parts.push(s.slice(start, i).trim())
      start = i + 1
    }
  }
  if (depth !== 0) return null
  parts.push(s.slice(start, s.length - 1).trim())
  return parts.length >= 2 ? parts : null
}

/** 成分の配列を "(a, b, c)" の形に戻す */
export function joinTuple(parts: string[]): string {
  return `(${parts.join(', ')})`
}
