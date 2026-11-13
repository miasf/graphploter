//! 見た目の比較（tools/visual）用: 式の一覧からメッシュを作り、ファイルに書き出す。
//!
//! ```text
//! cargo run --release --example dump_meshes -- <式の一覧.tsv> <出力先のフォルダ>
//! ```
//!
//! 式の一覧は 1 行 1 式で「名前<TAB>壁(0/1)<TAB>式」。# で始まる行と空行は飛ばす。
//! 壁の欄には、カンマで区切って設定を足せる: `0,xy=14.4,xz=10,flags=1,ires=40,res=75`
//! （xy・xz・yz は回転角（度）、flags は plot の flags（1: 再生・ドラッグ中の軽い計算）、ires・res は細かさ）。
//! 出力は 名前.bin（頂点の数 u32・番号の数 u32・頂点座標 f32 × 3・三角形の頂点番号 u32、すべてリトルエンディアン）と、
//! 全部の式の結果をまとめた meta.tsv（名前・三角形の数・時間 ms・説明 or エラー・破片の数・裏返りの割合 %）。
//! 破片は、頂点（同じ位置はまとめる）でつながった成分のうち三角形が 12 個以下のもの。裏返りは、同じ向きで 2 つ以上の
//! 三角形に使われている辺の割合（隣どうしで表裏が逆の所と、面が枝分かれする所で増える）。
//! 表示範囲と細かさはアプリの既定（x, y ∈ [−10, 10]、z ∈ [−6, 6]、細かさ 150、陰関数 80）。
//! 環境変数 VISUAL_ONLY を付けると、名前にそれを含む式だけ。
//!
//! 古いコミットと比べるときもこのファイルをそのまま入れてビルドするので、`plot` と `MeshResult` の
//! 古くからある口（positions・indices・kind）だけを使う。

use std::io::Write;
use std::time::Instant;

const DOMAIN: [f64; 6] = [-10.0, 10.0, -10.0, 10.0, -6.0, 6.0];
const RES: u32 = 150;
const IRES: u32 = 80;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (list, out) = match args.as_slice() {
        [_, a, b] => (a.clone(), std::path::PathBuf::from(b)),
        _ => panic!("使い方: dump_meshes <式の一覧.tsv> <出力先>"),
    };
    let only = std::env::var("VISUAL_ONLY").ok();
    std::fs::create_dir_all(&out).unwrap();
    let mut meta = std::fs::File::create(out.join("meta.tsv")).unwrap();
    for line in std::fs::read_to_string(&list).unwrap().lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.splitn(3, '\t').collect();
        let [name, walls, src] = cols[..] else { panic!("行の形が違う: {line}") };
        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }
        let mut opts = walls.split(',');
        let walls = opts.next().unwrap_or("0");
        let (mut rot, mut flags, mut res, mut ires) = ([0.0f64; 10], 0u32, RES, IRES);
        for o in opts {
            let (k, v) = o.split_once('=').unwrap_or_else(|| panic!("設定の形が違う: {o}"));
            let num: f64 = v.parse().unwrap_or_else(|_| panic!("設定の値が違う: {o}"));
            match k {
                // 回転角の並びは (x,y), (x,z), (x,w), (x,w2), (y,z), …
                "xy" => rot[0] = num.to_radians(),
                "xz" => rot[1] = num.to_radians(),
                "yz" => rot[4] = num.to_radians(),
                "flags" => flags = num as u32,
                "res" => res = num as u32,
                "ires" => ires = num as u32,
                _ => panic!("知らない設定: {o}"),
            }
        }
        let t = Instant::now();
        let r = graph_core::plot(
            src,
            &DOMAIN,
            res,
            ires,
            "",
            &[],
            "",
            &[],
            "",
            3,
            &[0, 1, 2],
            &[0.0; 3],
            &[f64::NAN; 6],
            "",
            &[],
            16,
            &rot,
            &[0.0; 5],
            0.0,
            0,
            walls == "1",
            false,
            1,
            false,
            flags,
        );
        let ms = t.elapsed().as_secs_f64() * 1e3;
        match r {
            Ok(r) => {
                let (p, ix) = (r.positions(), r.indices());
                // 表示範囲の外の三角形（回した陰関数は、表示範囲より広く作って描画側で切り落とす）は数えない。
                // 3 つの頂点がどれも、同じ面の外側にある三角形を捨てる
                let out_side = |k: u32, a: usize, hi: bool| {
                    let v = p[k as usize * 3 + a] as f64;
                    let m = (DOMAIN[2 * a + 1] - DOMAIN[2 * a]) * 1e-3;
                    if hi { v > DOMAIN[2 * a + 1] + m } else { v < DOMAIN[2 * a] - m }
                };
                let ix: Vec<u32> = ix
                    .chunks_exact(3)
                    .filter(|t| !(0..3).any(|a| [false, true].iter().any(|&h| t.iter().all(|&k| out_side(k, a, h)))))
                    .flatten()
                    .copied()
                    .collect();
                let file = std::fs::File::create(out.join(format!("{name}.bin"))).unwrap();
                let mut f = std::io::BufWriter::new(file);
                f.write_all(&(p.len() as u32 / 3).to_le_bytes()).unwrap();
                f.write_all(&(ix.len() as u32).to_le_bytes()).unwrap();
                for v in &p {
                    f.write_all(&v.to_le_bytes()).unwrap();
                }
                for v in &ix {
                    f.write_all(&v.to_le_bytes()).unwrap();
                }
                let (fragments, flipped) = topology(&p, &ix);
                writeln!(meta, "{name}\t{}\t{ms:.0}\t{}\t{fragments}\t{flipped:.2}", ix.len() / 3, r.kind()).unwrap();
            }
            Err(_) => writeln!(meta, "{name}\t0\t{ms:.0}\tエラー").unwrap(),
        }
        eprintln!("{name}: {ms:.0} ms");
    }
}

/// (破片の数, 裏返りの割合 %)
fn topology(p: &[f32], ix: &[u32]) -> (usize, f64) {
    use std::collections::HashMap;
    // 同じ位置の頂点をまとめる
    let mut ids: HashMap<[i64; 3], u32> = HashMap::new();
    let vid: Vec<u32> = p
        .chunks_exact(3)
        .map(|q| {
            let next = ids.len() as u32;
            *ids.entry([0, 1, 2].map(|a| (q[a] as f64 * 1e4).round() as i64)).or_insert(next)
        })
        .collect();
    let mut parent: Vec<u32> = (0..ids.len() as u32).collect();
    fn root(p: &mut [u32], mut x: u32) -> u32 {
        while p[x as usize] != x {
            p[x as usize] = p[p[x as usize] as usize];
            x = p[x as usize];
        }
        x
    }
    let mut edges: HashMap<(u32, u32), u32> = HashMap::new();
    let mut total = 0usize;
    for t in ix.chunks_exact(3) {
        let v = [vid[t[0] as usize], vid[t[1] as usize], vid[t[2] as usize]];
        let a = root(&mut parent, v[0]);
        for &k in &v[1..] {
            let b = root(&mut parent, k);
            parent[b as usize] = a;
        }
        for (a, b) in [(v[0], v[1]), (v[1], v[2]), (v[2], v[0])] {
            if a != b {
                *edges.entry((a, b)).or_default() += 1;
                total += 1;
            }
        }
    }
    let mut sizes: HashMap<u32, usize> = HashMap::new();
    for t in ix.chunks_exact(3) {
        *sizes.entry(root(&mut parent, vid[t[0] as usize])).or_default() += 1;
    }
    let fragments = sizes.values().filter(|&&s| s <= 12).count();
    let dup: usize = edges.values().filter(|&&c| c > 1).map(|&c| c as usize).sum();
    (fragments, 100.0 * dup as f64 / total.max(1) as f64)
}
