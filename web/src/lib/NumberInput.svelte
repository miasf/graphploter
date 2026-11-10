<script lang="ts">
  // 数値の入力欄。スマホでは端末の数字キーボードではなく、サイトの数式キーボードで入力する。
  // 2π や 1/3 のような式も書ける（値に直して onChange に渡す）
  import { evalNumber } from './evalExpr'
  import MathInput from './MathInput.svelte'

  interface Props {
    value: number
    label: string
    onChange: (value: number) => void
    /** 表示されたらすぐ入力を始める */
    autofocus?: boolean
    /** 入力欄から出た・エンターを押したとき */
    onDone?: () => void
    /** 値を欄に書くときの形（例: 小数第1位まで）。無ければ有効数字 12 桁 */
    format?: (value: number) => string
    /** 枠を消して文字のように見せる（入力中だけ枠を出す） */
    plain?: boolean
  }
  let { value, label, onChange, autofocus = false, onDone, format, plain = false }: Props = $props()

  /** 入力欄の中身（LaTeX） */
  let text = $state('')
  /** 最後に外へ知らせた値。これと違う値が来たときだけ「外から変わった」とみなして欄を書き換える */
  let emitted: number | null = null
  let invalid = $state(false)
  /** 入力中か。入力中は、外から値が変わっても欄を書き換えない */
  let editing = false

  const show = (v: number) => (format ? format(v) : String(Number(v.toPrecision(12)))) // 0.30000000000000004 → 0.3

  $effect(() => {
    if (value !== emitted) {
      emitted = value
      // 入力中に知らせた値が範囲の端にそろえられて戻ってきた（下限 0.25 の欄で 0.25 → 0.2 と消した途中など）ときも、
      // ここで書き換えると打っている途中の文字が消えて先へ進めない。欄を出たときに書き直す
      if (editing) return
      text = show(value)
      invalid = false
    }
  })

  /**
   * 入力欄から出たとき: 入れた値が範囲の端にそろえられていた・数値として読めない（空など）なら、実際の値を書く
   * （欄の文字と、実際に使っている値が食い違ったままにしない）
   */
  function finish() {
    editing = false
    const r = evalNumber(text, () => {})
    if (r && ('error' in r || r.value !== value)) {
      emitted = value
      text = show(value)
      invalid = false
    }
  }

  function run() {
    const r = evalNumber(text, run)
    if (r === null) return // 計算エンジンの準備待ち（できたら run が呼ばれる）
    invalid = 'error' in r
    if ('value' in r && r.value !== emitted) {
      emitted = r.value
      onChange(r.value)
    }
  }
</script>

<div class="num" class:invalid class:plain title={invalid ? '数値として読めません' : undefined}>
  <MathInput
    small
    value={text}
    {label}
    {autofocus}
    onFocusChange={(f) => {
      if (f) editing = true
      else {
        finish()
        onDone?.()
      }
    }}
    onEnter={() => onDone?.()}
    onInput={(v) => {
      text = v
      run()
    }}
  />
</div>

<style>
  .num {
    min-width: 0;
  }
  .invalid :global(math-field) {
    border-color: #c74440;
  }
  /* 枠を消して文字のように見せる。押せることが分かるよう下に点線を引き、入力中だけ枠を出す */
  .plain :global(math-field.small) {
    padding: 0 2px;
    border-color: transparent;
    border-bottom: 1px dashed var(--border-strong);
    border-radius: 0;
    background: transparent;
    font-size: 13px;
  }
  .plain :global(math-field.small:focus-within) {
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
  }
</style>
