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
  }
  let { value, label, onChange, format, prefix = '', suffix = '' }: Props = $props()
</script>

<span class="ev" title="タップして値を入力">
  {#if prefix}<span class="fix">{prefix}</span>{/if}
  <span class="field"><NumberInput plain {value} {label} {format} {onChange} /></span>
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
    min-width: 3.2em;
  }
  .fix {
    white-space: nowrap;
  }
</style>
