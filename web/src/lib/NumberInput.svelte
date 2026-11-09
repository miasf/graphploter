<script lang="ts">
  // 数値の入力欄。スマホでは端末の数字キーボードではなく、サイトの数式キーボードで入力する。
  // 2π や 1/3 のような式も書ける（値に直して onChange に渡す）
  import { evalNumber } from './evalExpr'
  import MathInput from './MathInput.svelte'

  interface Props {
    value: number
    label: string
    onChange: (value: number) => void
  }
  let { value, label, onChange }: Props = $props()

  /** 入力欄の中身（LaTeX） */
  let text = $state('')
  /** 最後に外へ知らせた値。これと違う値が来たときだけ「外から変わった」とみなして欄を書き換える */
  let emitted: number | null = null
  let invalid = $state(false)

  $effect(() => {
    if (value !== emitted) {
      emitted = value
      text = String(Number(value.toPrecision(12))) // 0.30000000000000004 → 0.3
      invalid = false
    }
  })

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

<div class="num" class:invalid title={invalid ? '数値として読めません' : undefined}>
  <MathInput
    small
    value={text}
    {label}
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
</style>
