<script lang="ts">
  // 式の行の右下の厳密値（計画 docs/plan-exact-core.md の EX5）。
  // 見つかった: 「= 厳密値」と小さく「≈ 近似値」（押すと 10 → 17 → 50 桁。D9）。答えが文字を含むなら「行に追加」（D2）。
  // 推定: 「= 式 ？（推定）」を薄く（D8）。存在しない: 理由。見つからない: 何も出さない（近似値は今までどおり説明の行）
  import { convertLatexToMarkup } from 'mathlive'
  import 'mathlive/static.css'
  import type { ExactState } from './types'
  import { formatApprox } from './valueText'

  let { exact, onAddRow }: { exact: ExactState; onAddRow?: (latex: string) => void } = $props()

  /** 近似値の桁: 0 は 10 桁、1 は 17 桁、2 は 50 桁 */
  let level = $state(0)
  /** 厳密値の LaTeX を、MathLive の静的な表示（入力欄ではない）にする */
  const markup = $derived(exact.kind === 'found' || exact.kind === 'guess' ? convertLatexToMarkup(exact.latex) : '')
  const digitsText = $derived.by(() => {
    if (exact.kind !== 'found' && exact.kind !== 'guess') return ''
    const [re, im] = exact.approx
    if (level === 2 && exact.kind === 'found' && exact.digits) return exact.digits
    return formatApprox(re, im, level === 1 ? 17 : 10)
  })
  function cycle() {
    const max = exact.kind === 'found' && exact.digits ? 2 : 1
    level = level >= max ? 0 : level + 1
  }
</script>

{#if exact.kind === 'computing'}
  <p class="exact computing" aria-live="polite">厳密値を計算中…</p>
{:else if exact.kind === 'found' || exact.kind === 'guess'}
  <div class="exact" class:guess={exact.kind === 'guess'} aria-live="polite">
    <span class="rel">=</span>
    <span class="exact-math" role="math" aria-label={`厳密値 ${exact.latex}`}>{@html markup}</span>
    {#if exact.kind === 'guess'}
      <span class="guess-mark" title="数値から推定した形です。証明ではありません">？（推定）</span>
    {/if}
    {#if digitsText}
      <button type="button" class="approx" title="押すと桁の数が変わります" onclick={cycle}>≈ {digitsText}</button>
    {/if}
    {#if exact.kind === 'found' && exact.addable && onAddRow}
      <button type="button" class="add-row" onclick={() => onAddRow?.(exact.inputLatex)}>行に追加</button>
    {/if}
  </div>
{:else if exact.kind === 'notExist'}
  <p class="exact not-exist" aria-live="polite">存在しません{exact.reason ? `（${exact.reason}）` : ''}</p>
{/if}

<style>
  .exact {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 0.35em;
    margin: 0.1rem 0 0.2rem;
    font-size: 0.85rem;
    line-height: 1.2;
    color: var(--text, inherit);
  }
  .computing,
  .not-exist {
    justify-content: flex-end;
    text-align: right;
    color: var(--muted, #777);
  }
  .exact-math {
    font-size: 1.05rem;
    max-width: 100%;
    overflow-x: auto;
  }
  .guess .exact-math,
  .guess .rel {
    opacity: 0.6;
  }
  .guess-mark {
    color: var(--muted, #777);
    font-size: 0.8rem;
  }
  .approx {
    border: none;
    background: transparent;
    color: var(--muted, #777);
    font: inherit;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
    padding: 0 0.2em;
    word-break: break-all;
  }
  .add-row {
    font: inherit;
    font-size: 0.8rem;
    padding: 0.05rem 0.5rem;
    cursor: pointer;
  }
</style>
