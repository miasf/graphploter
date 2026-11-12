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
//! 全部の式の結果をまとめた meta.tsv（名前・三角形の数・時間 ms・説明 or エラー）。
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
                writeln!(meta, "{name}\t{}\t{ms:.0}\t{}", ix.len() / 3, r.kind()).unwrap();
            }
            Err(_) => writeln!(meta, "{name}\t0\t{ms:.0}\tエラー").unwrap(),
        }
        eprintln!("{name}: {ms:.0} ms");
    }
}
