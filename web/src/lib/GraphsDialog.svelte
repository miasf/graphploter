<script lang="ts">
  // グラフの一覧の画面（Desmos の「マイグラフ」）: 新しいグラフ・開く・名前の変更・複製・削除・共有リンク
  import { formatDate, type GraphMeta } from './library'
  import { onMount } from 'svelte'

  interface Props {
    graphs: GraphMeta[]
    currentId: string
    saveFailed: boolean
    shareMessage: string
    onClose: () => void
    onOpen: (id: string) => void
    onNew: () => void
    onDelete: (id: string) => void
    onDuplicate: (id: string) => void
    onRename: (id: string, title: string) => void
    onShare: () => void
  }
  let { graphs, currentId, saveFailed, shareMessage, onClose, onOpen, onNew, onDelete, onDuplicate, onRename, onShare }: Props = $props()

  let query = $state('')
  /** 名前を変えている最中のグラフ */
  let editing = $state<string | null>(null)

  const shown = $derived(
    query.trim() === '' ? graphs : graphs.filter((g) => g.title.toLowerCase().includes(query.trim().toLowerCase())),
  )
  const now = Date.now()
  let dialog: HTMLDialogElement

  onMount(() => {
    const previous = document.activeElement
    const modal = dialog
    modal.showModal()
    return () => {
      modal.close()
      if (previous instanceof HTMLElement) previous.focus({ preventScroll: true })
    }
  })

  function backdropClick(e: MouseEvent) {
    if (e.target !== dialog) return
    const bounds = dialog.getBoundingClientRect()
    if (e.clientX < bounds.left || e.clientX > bounds.right || e.clientY < bounds.top || e.clientY > bounds.bottom) onClose()
  }

  function focusSelect(node: HTMLInputElement) {
    node.focus()
    node.select()
  }
</script>

<dialog
  bind:this={dialog}
  class="dialog"
  aria-label="グラフの一覧"
  oncancel={(e) => { e.preventDefault(); onClose() }}
  onkeydown={(e) => { if (e.key === 'Escape') e.stopPropagation() }}
  onclick={backdropClick}
>
    <div class="head">
      <span class="head-title">グラフ</span>
      <button class="close" onclick={onClose} aria-label="閉じる">×</button>
    </div>

    <div class="actions">
      <button class="primary" onclick={onNew}>＋ 新しいグラフ</button>
      <button class="secondary" onclick={onShare} title="今のグラフの中身を入れたリンクをコピーする">🔗 共有リンクをコピー</button>
      {#if graphs.length > 5}
        <input class="search" type="search" placeholder="名前で探す" aria-label="グラフを名前で探す" bind:value={query} />
      {/if}
    </div>
    {#if shareMessage}<p class="note ok">{shareMessage}</p>{/if}
    {#if saveFailed}
      <p class="note warn">このブラウザに保存できませんでした（容量がいっぱい、またはプライベートモード）。使わないグラフを削除するか、共有リンクで残してください。</p>
    {/if}

    {#if graphs.length === 0}
      <p class="empty">保存したグラフはまだありません。式を書くと、このブラウザに自動で保存されます。</p>
    {:else}
      <ul class="grid">
        {#each shown as g (g.id)}
          <li class="card" class:current={g.id === currentId}>
            <button class="thumb" onclick={() => onOpen(g.id)} aria-label="「{g.title}」を開く" title="開く">
              {#if g.thumb}
                <img src={g.thumb} alt="" />
              {:else}
                <span class="no-thumb">∿</span>
              {/if}
              {#if g.id === currentId}<span class="badge">開いているグラフ</span>{/if}
            </button>
            <div class="meta">
              {#if editing === g.id}
                <input
                  class="rename"
                  value={g.title}
                  aria-label="グラフの名前"
                  use:focusSelect
                  onkeydown={(e) => {
                    if (e.key === 'Enter') e.currentTarget.blur()
                    if (e.key === 'Escape') {
                      e.preventDefault()
                      e.stopPropagation()
                      editing = null
                    }
                  }}
                  onblur={(e) => {
                    if (editing === g.id) onRename(g.id, e.currentTarget.value)
                    editing = null
                  }}
                />
              {:else}
                <button class="name" onclick={() => onOpen(g.id)} title={g.title}>{g.title}</button>
              {/if}
              <span class="date">{formatDate(g.updated, now)}</span>
            </div>
            <div class="tools">
              <button onclick={() => (editing = g.id)} title="名前を変える" aria-label="「{g.title}」の名前を変える">✎</button>
              <button onclick={() => onDuplicate(g.id)} title="複製して開く" aria-label="「{g.title}」を複製">⧉</button>
              <button onclick={() => onDelete(g.id)} title="削除" aria-label="「{g.title}」を削除">🗑</button>
            </div>
          </li>
        {/each}
      </ul>
      {#if shown.length === 0}<p class="empty">「{query}」を含む名前のグラフはありません。</p>{/if}
    {/if}
    <p class="note">グラフはこのブラウザだけに保存されます（別の端末・別のブラウザでは見えません）。ほかの人や別の端末へは、共有リンクで渡してください。</p>
</dialog>

<style>
  .dialog::backdrop {
    background: rgb(0 0 0 / 0.35);
  }
  .dialog {
    width: min(760px, calc(100% - 32px));
    max-height: min(86dvh, calc(100% - 32px));
    overflow-y: auto;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.3);
    padding: 16px 18px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }
  .head-title {
    font-size: 18px;
    font-weight: 700;
  }
  .close {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 22px;
    line-height: 1;
    cursor: pointer;
    padding: 4px 8px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }
  .primary,
  .secondary {
    font: inherit;
    font-size: 14px;
    padding: 7px 14px;
    border-radius: 8px;
    cursor: pointer;
  }
  .primary {
    border: 1px solid var(--accent);
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 600;
  }
  .secondary {
    border: 1px solid var(--border-strong);
    background: var(--bg);
    color: var(--text);
  }
  .search {
    flex: 1;
    min-width: 140px;
    font: inherit;
    font-size: 14px;
    padding: 6px 10px;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--bg);
    color: var(--text);
  }
  .note {
    font-size: 12px;
    color: var(--muted);
    margin: 10px 0 0;
  }
  .note.ok {
    color: var(--accent);
  }
  .note.warn {
    color: var(--error);
  }
  .empty {
    color: var(--muted);
    font-size: 14px;
    margin: 20px 0;
    text-align: center;
  }
  .grid {
    list-style: none;
    margin: 14px 0 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
    gap: 12px;
  }
  .card {
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .card.current {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .thumb {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 4 / 3;
    padding: 0;
    border: none;
    border-bottom: 1px solid var(--border);
    background: var(--gutter);
    cursor: pointer;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .no-thumb {
    font-size: 32px;
    color: var(--muted);
  }
  .badge {
    position: absolute;
    left: 6px;
    top: 6px;
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--accent);
    color: var(--on-accent);
  }
  .meta {
    padding: 6px 8px 0;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name {
    font: inherit;
    font-size: 14px;
    font-weight: 600;
    text-align: left;
    border: none;
    background: transparent;
    color: var(--text);
    padding: 0;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .rename {
    font: inherit;
    font-size: 14px;
    padding: 1px 4px;
    border: 1px solid var(--accent);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    min-width: 0;
  }
  .date {
    font-size: 12px;
    color: var(--muted);
  }
  .tools {
    display: flex;
    justify-content: flex-end;
    gap: 2px;
    padding: 2px 4px 4px;
  }
  .tools button {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 15px;
    padding: 4px 7px;
    border-radius: 6px;
    cursor: pointer;
  }
  .tools button:hover {
    background: var(--hover);
    color: var(--text);
  }
  @media (max-width: 480px) {
    .dialog {
      width: 100%;
      height: 100%;
      max-height: 100%;
      border-radius: 0;
    }
    .grid {
      grid-template-columns: repeat(2, 1fr);
      gap: 8px;
    }
  }
</style>
