<script lang="ts">
  let { values }: { values?: Float64Array } = $props()

  const COLUMNS = 4
  const PAGE_SIZE = 12
  let page = $state(0)
  const count = $derived((values?.length ?? 0) / COLUMNS)
  const lastPage = $derived(Math.max(0, Math.ceil(count / PAGE_SIZE) - 1))
  const currentPage = $derived(Math.min(page, lastPage))
  const first = $derived(currentPage * PAGE_SIZE)
  const last = $derived(Math.min(first + PAGE_SIZE, count))
  const rows = $derived.by(() => {
    if (!values) return []
    return Array.from({ length: last - first }, (_, index) => {
      const offset = (first + index) * COLUMNS
      return {
        index: values[offset],
        value: formatValue(values[offset + 1], values[offset + 2]),
        argument: values[offset + 3],
      }
    })
  })

  function formatValue(re: number, im: number): string {
    if (Number.isNaN(re) || Number.isNaN(im)) return '未定義'
    const format = (n: number) => Number.isFinite(n) ? String(Number(n.toPrecision(9))).replace(/^-/g, '−') : n < 0 ? '−∞' : '∞'
    const absImag = Math.abs(im)
    if (im === 0) return format(re)
    const imaginary = `${absImag === 1 ? '' : format(absImag)}i`
    if (re === 0) return (im < 0 ? '−' : '') + imaginary
    return `${format(re)} ${im < 0 ? '−' : '+'} ${imaginary}`
  }
</script>

{#if !values}
  <p class="message" role="status">項を読み込み中…</p>
{:else if count === 0}
  <p class="message">表示範囲で計算できる項がありません。</p>
{:else}
  <table aria-label="数列の項の表">
    <thead>
      <tr><th scope="col">添字</th><th scope="col">項の値</th><th scope="col" title="この項を求めるため、規則の中の変数に代入した値">代入した添字</th></tr>
    </thead>
    <tbody>
      {#each rows as row (row.index)}
        <tr>
          <th scope="row">{row.index}</th>
          <td>{row.value}</td>
          <td>{Number.isNaN(row.argument) ? '初期値' : row.argument}</td>
        </tr>
      {/each}
    </tbody>
  </table>
  {#if lastPage > 0}
    <div class="pages">
      <button type="button" aria-label="前の項" disabled={currentPage === 0} onclick={() => (page = currentPage - 1)}>‹</button>
      <span>{first + 1}–{last} / {count} 項</span>
      <button type="button" aria-label="次の項" disabled={currentPage === lastPage} onclick={() => (page = currentPage + 1)}>›</button>
    </div>
  {/if}
{/if}

<style>
  table {
    width: 100%;
    table-layout: fixed;
    border-collapse: collapse;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  th, td {
    padding: 4px 6px;
    border-bottom: 1px solid var(--border);
    overflow-wrap: anywhere;
  }
  th:first-child { width: 19%; }
  th:last-child { width: 32%; }
  thead, .message, .pages { color: var(--muted); }
  tbody th { font-weight: normal; }
  .message { margin: 5px 0; font-size: 12px; }
  .pages { display: flex; align-items: center; justify-content: center; gap: 8px; margin-top: 6px; font-size: 12px; }
  button {
    min-width: 32px;
    min-height: 32px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    font-size: 18px;
    cursor: pointer;
  }
  button:disabled { opacity: 0.4; cursor: default; }
</style>
