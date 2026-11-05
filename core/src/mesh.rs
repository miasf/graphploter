//! 曲面のメッシュ（三角形の集まり）を作る。

use crate::eval::{as_real, Program};
use num_complex::Complex64 as C;

/// 表示範囲（直方体）
#[derive(Debug, Clone, Copy)]
pub struct Domain {
    pub x: (f64, f64),
    pub y: (f64, f64),
    pub z: (f64, f64),
}

/// 描画用のメッシュデータ。Three.js の BufferGeometry にそのまま渡せる形。
#[derive(Debug, Default)]
pub struct Mesh {
    /// 頂点座標 [x0, y0, z0, x1, y1, z1, ...]
    pub positions: Vec<f32>,
    /// 頂点の法線（陰影の計算に使う）
    pub normals: Vec<f32>,
    /// 三角形の頂点番号 [a0, b0, c0, a1, b1, c1, ...]
    pub indices: Vec<u32>,
}

/// 陽関数の曲面 z = f(x, y) を、`n × n` マスの格子で作る。
///
/// `prog` の変数は ["x", "y"] の順であること。
/// 値が定義されない点（NaN・無限大）や、表示範囲から大きく外れた点は省く。
pub fn explicit_surface(prog: &Program, dom: Domain, n: usize) -> Mesh {
    let n = n.max(2);
    let m = n + 1; // 1辺の頂点数

    // 格子点の座標を用意
    let mut xs = Vec::with_capacity(m * m);
    let mut ys = Vec::with_capacity(m * m);
    for j in 0..m {
        let y = lerp(dom.y, j as f64 / n as f64);
        for i in 0..m {
            xs.push(lerp(dom.x, i as f64 / n as f64));
            ys.push(y);
        }
    }

    // まとめて計算
    let mut vals = vec![C::new(0.0, 0.0); m * m];
    prog.eval_batch(&[&xs, &ys], &mut vals);

    // 表示範囲の上下に、範囲の高さ1つ分の余裕を持たせる。
    // はみ出た部分は描画側（Three.js のクリッピング）で切り落とす。
    let h = dom.z.1 - dom.z.0;
    let (zlo, zhi) = (dom.z.0 - h, dom.z.1 + h);

    // 使える頂点だけに番号を振る
    let mut index_of = vec![u32::MAX; m * m];
    let mut mesh = Mesh::default();
    for k in 0..m * m {
        let z = as_real(vals[k]);
        if z.is_finite() && z >= zlo && z <= zhi {
            index_of[k] = (mesh.positions.len() / 3) as u32;
            mesh.positions.extend_from_slice(&[xs[k] as f32, ys[k] as f32, z as f32]);
        }
    }

    // 各マスを2つの三角形に分ける。3頂点とも使えるものだけ残す。
    for j in 0..n {
        for i in 0..n {
            let a = index_of[j * m + i];
            let b = index_of[j * m + i + 1];
            let c = index_of[(j + 1) * m + i];
            let d = index_of[(j + 1) * m + i + 1];
            for tri in [[a, b, d], [a, d, c]] {
                if tri.iter().all(|&v| v != u32::MAX) {
                    mesh.indices.extend_from_slice(&tri);
                }
            }
        }
    }

    compute_normals(&mut mesh);
    mesh
}

fn lerp((a, b): (f64, f64), t: f64) -> f64 {
    a + (b - a) * t
}

/// 周りの三角形の向き（面積で重み付け）を平均して、頂点の法線を求める。
fn compute_normals(mesh: &mut Mesh) {
    let p = &mesh.positions;
    let mut nrm = vec![0.0f64; p.len()];
    for t in mesh.indices.chunks_exact(3) {
        let v = |k: u32| {
            let k = k as usize * 3;
            [p[k] as f64, p[k + 1] as f64, p[k + 2] as f64]
        };
        let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        // 外積 e1 × e2（長さが三角形の面積の2倍なので、そのまま足すと面積の重み付けになる）
        let cr = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        for &k in t {
            let k = k as usize * 3;
            for d in 0..3 {
                nrm[k + d] += cr[d];
            }
        }
    }
    mesh.normals = nrm
        .chunks_exact(3)
        .flat_map(|n| {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let s = if len > 0.0 { 1.0 / len } else { 0.0 };
            [(n[0] * s) as f32, (n[1] * s) as f32, (n[2] * s) as f32]
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn dom() -> Domain {
        Domain { x: (-1.0, 1.0), y: (-1.0, 1.0), z: (-1.0, 1.0) }
    }

    #[test]
    fn plane() {
        let p = Program::compile(&parse("0").unwrap(), &["x", "y"]).unwrap();
        let m = explicit_surface(&p, dom(), 4);
        assert_eq!(m.positions.len(), 25 * 3);
        assert_eq!(m.indices.len(), 4 * 4 * 2 * 3);
        // 平面 z = 0 の法線は (0, 0, 1)
        assert!((m.normals[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn undefined_region_is_removed() {
        // x < 0 では sqrt(x) が実数にならないので、そこは三角形が無くなる
        let p = Program::compile(&parse("sqrt(x)").unwrap(), &["x", "y"]).unwrap();
        let full = explicit_surface(&Program::compile(&parse("0").unwrap(), &["x", "y"]).unwrap(), dom(), 10);
        let half = explicit_surface(&p, dom(), 10);
        assert!(half.indices.len() < full.indices.len());
        assert!(!half.indices.is_empty());
    }
}
