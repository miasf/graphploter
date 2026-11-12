#!/usr/bin/env bash
# 指定したコミットごとに、計算エンジンをビルドして tools/visual/cases.tsv の式のメッシュを書き出す。
#
#   tools/visual/dump_revs.sh <出力先> <コミット>...
#
# コミットごとに <出力先>/<短いハッシュ>/ に 名前.bin と meta.tsv ができる。
# 古いコミットには core/examples/dump_meshes.rs が無いので、今の版をコピーして使う（git worktree の中で。元の作業ツリーは変えない）。
# 「WORK」と書くと、今の作業ツリー（コミットしていない変更を含む）を使う。
set -euo pipefail
root=$(git rev-parse --show-toplevel)
out=$(realpath -m "$1"); shift
mkdir -p "$out"
cases="$root/tools/visual/cases.tsv"
example="$root/core/examples/dump_meshes.rs"
trees="$out/.trees"
mkdir -p "$trees"
for rev in "$@"; do
  if [ "$rev" = WORK ]; then
    name=WORK
    core="$root/core"
  else
    name=$(git rev-parse --short "$rev")
    if [ -f "$out/$name/meta.tsv" ]; then echo "== $name: 書き出し済み"; continue; fi
    tree="$trees/$name"
    [ -d "$tree" ] || git worktree add --detach "$tree" "$rev" >/dev/null
    core="$tree/core"
    mkdir -p "$core/examples"
    cp "$example" "$core/examples/dump_meshes.rs"
  fi
  echo "== $name: ビルド"
  # ビルドの成果物は 1 か所にまとめる（依存のビルドを使い回す）
  CARGO_TARGET_DIR="$out/.target" cargo build --quiet --release --example dump_meshes --manifest-path "$core/Cargo.toml"
  echo "== $name: 書き出し"
  "$out/.target/release/examples/dump_meshes" "$cases" "$out/$name" 2>/dev/null
  git -C "$root" log -1 --format='%h %s' "${rev/WORK/HEAD}" > "$out/$name/rev.txt"
  [ "$rev" = WORK ] && echo "WORK（作業中の変更）" > "$out/$name/rev.txt"
done
