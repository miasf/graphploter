//! 厳密値の物差し（計画 `docs/plan-exact-core.md` の EX0）。
//!
//! `core/tests/exact_cases/*.tsv` の例題を全部、厳密値の道にかけ、分野ごとに
//! 「正しい形」「値は正しいが形が違う」「見つからない」「推定」「照らし合わせで捨てた」「間違い」を数えて出す。
//! 落ちるのは、間違い（期待と違う値を出した）が 1 つでもあるときと、印が `必須` の行が正しい形で出ないとき。
//!
//! 表は `tools/exact/cases.py` が SymPy で答えを確かめて作る（手で直さない）。
//! あわせて、今の f64 の道（`eval_constant`）の値も表の近似値と比べ、違う行を出す（表の入力の書き方の間違いを見つけるため。落ちはしない）

use graph_core::equation::eval_constant;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mark {
    Required,
    Target,
    NotExist,
}

struct Case {
    input: String,
    latex: String,
    approx: Option<(f64, f64)>,
    /// 方程式の行の解の値（連立方程式は点の座標を順に）
    points: Option<Vec<f64>>,
    area: String,
    mark: Mark,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Verdict {
    Exact,
    WrongForm,
    NotFound,
    Guess,
    Rejected,
    Wrong,
}

const VERDICTS: [(Verdict, &str); 6] = [
    (Verdict::Exact, "正しい形"),
    (Verdict::WrongForm, "形が違う"),
    (Verdict::NotFound, "見つからない"),
    (Verdict::Guess, "推定"),
    (Verdict::Rejected, "捨てた"),
    (Verdict::Wrong, "間違い"),
];

/// 厳密値の道の答え（判定に使う形）
enum Answer {
    Found { latex: String, approx: Option<(f64, f64)>, engine: String, points: Option<Vec<f64>> },
    Guess,
    NotFound,
    Rejected,
    NotExist,
}

/// 厳密値の道
fn evaluate(input: &str) -> Answer {
    use graph_core::exact::Outcome;
    let params = graph_core::eval::Params::new();
    let _d = graph_core::expand::install(graph_core::expand::Defs::parse("", &params));
    let r = graph_core::exact::evaluate(input, &params);
    match r.outcome {
        Outcome::Found(t) => {
            // 方程式の行: 解（組は座標を順に）の値
            let points = r.equation.then(|| {
                let items = match &t {
                    graph_core::exact::term::Term::List(v) => v.clone(),
                    _ => Vec::new(),
                };
                items
                    .iter()
                    .flat_map(|p| match p {
                        graph_core::exact::term::Term::Tuple(c) => c.clone(),
                        other => vec![other.clone()],
                    })
                    .map(|c| graph_core::exact::ball::value(&c, 64).map(|b| b.to_f64().0).unwrap_or(f64::NAN))
                    .collect()
            });
            Answer::Found { latex: r.latex, approx: r.approx, engine: r.engine, points }
        }
        Outcome::Guess(_) => Answer::Guess,
        Outcome::NotFound(_) => Answer::NotFound,
        Outcome::Rejected(_) => Answer::Rejected,
        Outcome::NotExist(_) => Answer::NotExist,
    }
}

fn parse_approx(s: &str) -> Option<(f64, f64)> {
    if s == "-" || s.is_empty() {
        return None;
    }
    let mut it = s.split(';');
    let re: f64 = it.next()?.parse().ok()?;
    let im: f64 = it.next().map(|t| t.parse().unwrap()).unwrap_or(0.0);
    Some((re, im))
}

fn load() -> Vec<Case> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/exact_cases");
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "tsv")).collect();
    files.sort();
    let mut out = Vec::new();
    for f in files {
        for line in std::fs::read_to_string(&f).unwrap().lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            assert_eq!(cols.len(), 6, "{}: 列の数が 6 でない: {line}", f.display());
            let mark = match cols[5] {
                "必須" => Mark::Required,
                "目標" => Mark::Target,
                "存在しない" => Mark::NotExist,
                m => panic!("{}: 知らない印 {m}", f.display()),
            };
            let equation = cols[3] == "方程式";
            let points = equation.then(|| if cols[2] == "-" { Vec::new() } else { cols[2].split(' ').map(|v| v.parse().unwrap()).collect() });
            let approx = if equation { None } else { parse_approx(cols[2]) };
            out.push(Case { input: cols[0].into(), latex: cols[1].into(), approx, points, area: cols[3].into(), mark });
        }
    }
    out
}

/// LaTeX を比べる形にする: `\left`・`\right`・`\,` を消し、空白を消す（`\pi i` のように命令の後の文字の前の空白だけ 1 つ残す）
fn normalize(s: &str) -> String {
    let s = s.replace("\\left", "").replace("\\right", "").replace("\\,", " ").replace("\\;", " ");
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    let mut in_command = false;
    let mut pending_space = false;
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && in_command && c.is_ascii_alphabetic() {
            out.push(' ');
        }
        pending_space = false;
        if c == '\\' {
            in_command = true;
            out.push(c);
            // 1 文字の命令（`\\`・`\{`）
            if let Some(&n) = chars.peek() {
                if !n.is_ascii_alphabetic() {
                    out.push(n);
                    chars.next();
                    in_command = false;
                }
            }
            continue;
        }
        if !c.is_ascii_alphabetic() {
            in_command = false;
        }
        out.push(c);
    }
    out
}

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let m = (b.0 * b.0 + b.1 * b.1).sqrt().max(1.0);
    d <= 1e-12 * m
}

fn judge(c: &Case, a: &Answer) -> Verdict {
    match a {
        Answer::NotFound => Verdict::NotFound,
        Answer::Guess => Verdict::Guess,
        Answer::Rejected => Verdict::Rejected,
        Answer::NotExist => {
            if c.mark == Mark::NotExist {
                Verdict::Exact
            } else {
                Verdict::Wrong
            }
        }
        Answer::Found { latex, approx, points, .. } => {
            if c.mark == Mark::NotExist {
                return Verdict::Wrong;
            }
            // 方程式: 解の値の並びが表と同じ（一般解・解なしは表の並びが空）
            if let (Some(want), Some(got)) = (&c.points, points) {
                if want.len() != got.len() || want.iter().zip(got).any(|(a, b)| !close((*b, 0.0), (*a, 0.0))) {
                    return Verdict::Wrong;
                }
            }
            if let (Some(want), Some(got)) = (c.approx, *approx) {
                if !close(got, want) {
                    return Verdict::Wrong;
                }
            }
            if normalize(latex) == normalize(&c.latex) {
                Verdict::Exact
            } else {
                Verdict::WrongForm
            }
        }
    }
}

#[test]
fn normalize_keeps_commands_apart() {
    assert_eq!(normalize(r"\pi i"), r"\pi i");
    assert_eq!(normalize(r"\ln 2 - 1"), r"\ln2-1");
    assert_eq!(normalize(r"\left( x \right)"), "(x)");
    assert_eq!(normalize(r"\frac{\sqrt{3}}{2}"), r"\frac{\sqrt{3}}{2}");
    assert_eq!(normalize(r"\sin x"), r"\sin x");
    assert_eq!(normalize(r"a \\ b"), r"a\\b");
}

#[test]
fn table() {
    let cases = load();
    assert!(cases.len() >= 400, "例題が {} 問しかない", cases.len());
    let mut counts: BTreeMap<String, BTreeMap<Verdict, usize>> = BTreeMap::new();
    let mut failures = Vec::new();
    let mut f64_differs = Vec::new();
    let start = std::time::Instant::now();
    let mut times = Vec::new();
    let mut verdicts = Vec::new();
    for c in &cases {
        let t0 = std::time::Instant::now();
        let a = evaluate(&c.input);
        times.push(t0.elapsed().as_secs_f64());
        let v = judge(c, &a);
        verdicts.push(v);
        if v == Verdict::WrongForm && std::env::var("EXACT_SHOW").is_ok() {
            if let Answer::Found { latex, .. } = &a {
                println!("形が違う: {} → {} （表: {}）", c.input, latex, c.latex);
            }
        }
        *counts.entry(c.area.clone()).or_default().entry(v).or_default() += 1;
        if v == Verdict::Wrong {
            failures.push(format!("間違い: {}", c.input));
        }
        // 「行に追加」のエンジンの文字列が、同じ値に戻る（EX4）
        if let Answer::Found { engine, approx: Some(want), .. } = &a {
            let params = graph_core::eval::Params::new();
            match std::panic::catch_unwind(|| graph_core::equation::eval_constant_complex(engine, &params)) {
                Ok(Ok(z)) if close((z.re, z.im), *want) || ((z.re - want.0).abs() + (z.im - want.1).abs()) <= 1e-9 * want.0.abs().max(want.1.abs()).max(1.0) => {}
                other => failures.push(format!("エンジンの文字列が戻らない: {} → {engine}（{other:?}）", c.input)),
            }
        }
        if c.mark == Mark::Required && v != Verdict::Exact {
            failures.push(format!("必須の行が {v:?}: {}", c.input));
        }
        // 今の f64 の道と比べる（実数の値の行だけ）
        if let Some((re, 0.0)) = c.approx {
            match std::panic::catch_unwind(|| eval_constant(&c.input)) {
                Ok(Ok(v)) if (v - re).abs() <= 1e-6 * re.abs().max(1.0) => {}
                Ok(Ok(v)) => f64_differs.push(format!("{}: 今の道 {v}、表 {re}", c.input)),
                Ok(Err(e)) => f64_differs.push(format!("{}: 今の道は誤り「{e}」", c.input)),
                Err(_) => f64_differs.push(format!("{}: 今の道でパニック", c.input)),
            }
        }
    }
    // 正しい形で出た行の入力を書き出す（tools/exact/required.txt を作り直すとき: EXACT_DUMP=パス）
    if let Ok(path) = std::env::var("EXACT_DUMP") {
        let list: Vec<&str> = cases.iter().zip(&verdicts).filter(|(_, v)| **v == Verdict::Exact).map(|(c, _)| c.input.as_str()).collect();
        std::fs::write(path, list.join("\n") + "\n").unwrap();
    }
    println!("分野        | {}", VERDICTS.iter().map(|v| v.1).collect::<Vec<_>>().join(" | "));
    let mut total: BTreeMap<Verdict, usize> = BTreeMap::new();
    for (area, m) in &counts {
        let row: Vec<String> = VERDICTS.iter().map(|(v, _)| m.get(v).copied().unwrap_or(0).to_string()).collect();
        let n: usize = m.values().sum();
        let exact = m.get(&Verdict::Exact).copied().unwrap_or(0);
        println!("{area:<8}({n:>3}) | {} | 正しい形 {:.0}%", row.join(" | "), 100.0 * exact as f64 / n as f64);
        for (v, k) in m {
            *total.entry(*v).or_default() += k;
        }
    }
    let n = cases.len();
    let exact = total.get(&Verdict::Exact).copied().unwrap_or(0);
    println!("合計 {n} 問、正しい形 {exact}（{:.0}%）", 100.0 * exact as f64 / n as f64);
    times.sort_by(f64::total_cmp);
    println!("時間: 中央値 {:.3} ms、最大 {:.3} ms、合計 {:.2} s", times[n / 2] * 1e3, times[n - 1] * 1e3, start.elapsed().as_secs_f64());
    println!("今の f64 の道と表の近似値が違う行（{} 行）:", f64_differs.len());
    for d in &f64_differs {
        println!("  {d}");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
