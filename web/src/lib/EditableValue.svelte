<script lang="ts">
  // スライダーの横の数値。タップ（クリック）すると、その場で数式キーボードから値を入れられる。
  // （スマホでは、こちらから入力を始めると数式の欄がすぐ閉じてしまうので、最初から入力欄にしておき、
  //   ふだんは枠を消して文字のように見せる）
  import NumberInput from './NumberInput.svelte'

  interface Props {
    value: number
    /** 何の値か（読み上げ・入力欄の説明） */
    label: string
    onChange: (value: number) => void
    /** 値を欄に書くときの形 */
    format?: (value: number) => string
    /** 値の前後に付ける文字（"= 0.3" の "= "、"30°" の "°"） */
    prefix?: string
    suffix?: string
    /** 入れられる値の範囲（外なら端の値にする）。重すぎる・意味の無い値を防ぐ */
    min?: number
    max?: number
    /** 整数に丸める */
    integer?: boolean
    /** 欄の幅（文字数）を固定する。スライダーと同じ行に置くときに使う
     *  （値の桁数で幅が変わるとスライダーの幅も変わり、指の下の値が変わって数値が揺れ続けるため） */
    chars?: number
  }
  let { value, label, onChange, format, prefix = '', suffix = '', min = -Infinity, max = Infinity, integer = false, chars }: Props =
    $props()

  function change(v: number) {
    const r = integer ? Math.round(v) : v
    onChange(Math.min(max, Math.max(min, r)))
  }
</script>

<span class="ev" title="タップして値を入力">
  {#if prefix}<span class="fix">{prefix}</span>{/if}
  <span class="field" style:--chars={chars ?? String(format ? format(value) : Number(value.toPrecision(6))).length}><NumberInput plain {value} {label} {format} onChange={change} /></span>
  {#if suffix}<span class="fix">{suffix}</span>{/if}
</span>

<style>
  .ev {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    vertical-align: middle;
  }
  .field {
    display: block;
    width: calc(var(--chars, 4) * 0.75em + 14px); /* 数値の桁数に合わせた幅（単位の文字との間を空けすぎない） */
    min-width: 2.2em;
  }
  .fix {
    white-space: nowrap;
  }
</style>
