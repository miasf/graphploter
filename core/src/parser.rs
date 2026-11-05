//! 数式の文字列を構文木 (AST) に変換する。
//!
//! Desmos のように書きやすくするため、次の書き方を許す:
//! - 暗黙の掛け算: `2x`, `xy`, `2sin(x)`, `(x+1)(x-1)`
//! - 括弧なしの関数呼び出し: `sin x`, `sin^2 x`  (= (sin x)^2)
//! - 定数: `pi` / `π`, `e`, `i`（虚数単位）

use crate::functions::Func;
use std::fmt;

/// 構文木のノード
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    /// 名前付き定数 (pi, e, i)
    Const(Constant),
    Var(String),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Call(Func, Vec<Expr>),
    /// 組 (a, b, c)。媒介変数表示の点を表す
    Tuple(Vec<Expr>),
}

impl Expr {
    /// 式の中で使われている変数名か調べる
    pub fn uses_var(&self, name: &str) -> bool {
        match self {
            Expr::Var(v) => v == name,
            Expr::Num(_) | Expr::Const(_) => false,
            Expr::Neg(a) => a.uses_var(name),
            Expr::Bin(_, a, b) => a.uses_var(name) || b.uses_var(name),
            Expr::Call(_, args) | Expr::Tuple(args) => args.iter().any(|a| a.uses_var(name)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Constant {
    Pi,
    E,
    I,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

/// 解析エラー（`pos` は文字単位の位置）
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub pos: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}（{}文字目）", self.message, self.pos + 1)
    }
}

fn err<T>(message: impl Into<String>, pos: usize) -> Result<T, ParseError> {
    Err(ParseError {
        message: message.into(),
        pos,
    })
}

// ---------------------------------------------------------------------------
// 字句解析（文字列 → トークン列）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Var(String),
    Const(Constant),
    Func(Func),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Bang,
    LParen,
    RParen,
    Comma,
    Eof,
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    pos: usize,
}

/// 識別子として認識する名前（関数と定数）。長いものを優先して照合する。
fn named_tokens() -> Vec<(&'static str, Tok)> {
    let mut v: Vec<(&'static str, Tok)> = Func::ALL
        .iter()
        .flat_map(|f| f.names().iter().map(move |n| (*n, Tok::Func(*f))))
        .collect();
    v.push(("pi", Tok::Const(Constant::Pi)));
    v.push(("π", Tok::Const(Constant::Pi)));
    v.push(("e", Tok::Const(Constant::E)));
    v.push(("i", Tok::Const(Constant::I)));
    // 長い名前から順に試す（"sinh" を "sin" + "h" と誤認しないため）
    v.sort_by_key(|t| std::cmp::Reverse(t.0.chars().count()));
    v
}

fn tokenize(src: &str) -> Result<Vec<Token>, ParseError> {
    let chars: Vec<char> = src.chars().collect();
    let names = named_tokens();
    let mut out = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;

        // 数値
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            let n: f64 = match s.parse() {
                Ok(n) => n,
                Err(_) => return err(format!("数値 \"{s}\" を読めません"), start),
            };
            out.push(Token { tok: Tok::Num(n), pos: start });
            continue;
        }

        // 英字の並び: 関数名・定数を最長一致で取り出し、残りは1文字ずつ変数にする
        // 例) "xsinx" → x, sin, x
        if c.is_alphabetic() {
            let mut end = i;
            while end < chars.len() && chars[end].is_alphabetic() {
                end += 1;
            }
            while i < end {
                let rest: String = chars[i..end].iter().collect();
                if let Some((name, tok)) = names.iter().find(|(n, _)| rest.starts_with(n)) {
                    out.push(Token { tok: tok.clone(), pos: i });
                    i += name.chars().count();
                } else {
                    out.push(Token { tok: Tok::Var(chars[i].to_string()), pos: i });
                    i += 1;
                }
            }
            continue;
        }

        let tok = match c {
            '+' => Tok::Plus,
            '-' | '−' => Tok::Minus,
            '*' | '·' | '×' => Tok::Star,
            '/' | '÷' => Tok::Slash,
            '^' => Tok::Caret,
            '!' => Tok::Bang,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            ',' => Tok::Comma,
            _ => return err(format!("使えない文字 '{c}' があります"), i),
        };
        out.push(Token { tok, pos: i });
        i += 1;
    }
    out.push(Token { tok: Tok::Eof, pos: chars.len() });
    Ok(out)
}

// ---------------------------------------------------------------------------
// 構文解析（トークン列 → 構文木）
//
// 優先順位（弱い順）:
//   expr  := term (('+' | '-') term)*
//   term  := unary (('*' | '/' | 暗黙の掛け算) unary)*
//   unary := ('-' | '+') unary | power
//   power := primary '!'* ('^' unary)?     ← 右結合: 2^3^2 = 2^(3^2)、x! は階乗
//   primary := 数値 | 変数 | 定数 | 関数呼び出し | '(' expr ')'
// ---------------------------------------------------------------------------

struct Parser {
    toks: Vec<Token>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.i].tok
    }
    fn pos(&self) -> usize {
        self.toks[self.i].pos
    }
    fn next(&mut self) -> Token {
        let t = self.toks[self.i].clone();
        if self.i < self.toks.len() - 1 {
            self.i += 1;
        }
        t
    }
    fn expect(&mut self, tok: Tok, what: &str) -> Result<(), ParseError> {
        if *self.peek() == tok {
            self.next();
            Ok(())
        } else {
            err(format!("{what} が必要です"), self.pos())
        }
    }

    fn expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.term()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => return Ok(lhs),
            };
            self.next();
            let rhs = self.term()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
    }

    fn term(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => {
                    self.next();
                    BinOp::Mul
                }
                Tok::Slash => {
                    self.next();
                    BinOp::Div
                }
                // 次に値が始まるなら暗黙の掛け算
                Tok::Num(_) | Tok::Var(_) | Tok::Const(_) | Tok::Func(_) | Tok::LParen => BinOp::Mul,
                _ => return Ok(lhs),
            };
            let rhs = self.unary()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Tok::Minus => {
                self.next();
                Ok(Expr::Neg(Box::new(self.unary()?)))
            }
            Tok::Plus => {
                self.next();
                self.unary()
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Result<Expr, ParseError> {
        let mut base = self.primary()?;
        // 階乗 x! = Γ(x + 1)。累乗より先に計算する（3!^2 = (3!)^2）
        while *self.peek() == Tok::Bang {
            self.next();
            let arg = Expr::Bin(BinOp::Add, Box::new(base), Box::new(Expr::Num(1.0)));
            base = Expr::Call(Func::Gamma, vec![arg]);
        }
        if *self.peek() == Tok::Caret {
            self.next();
            let exp = self.unary()?;
            return Ok(Expr::Bin(BinOp::Pow, Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let t = self.next();
        match t.tok {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Var(v) => Ok(Expr::Var(v)),
            Tok::Const(c) => Ok(Expr::Const(c)),
            Tok::LParen => {
                let e = self.expr()?;
                // カンマが続けば組 (a, b, c)
                if *self.peek() == Tok::Comma {
                    let mut items = vec![e];
                    while *self.peek() == Tok::Comma {
                        self.next();
                        items.push(self.expr()?);
                    }
                    self.expect(Tok::RParen, "閉じ括弧 ')'")?;
                    return Ok(Expr::Tuple(items));
                }
                self.expect(Tok::RParen, "閉じ括弧 ')'")?;
                Ok(e)
            }
            Tok::Func(f) => self.call(f, t.pos),
            Tok::Eof => err("式が途中で終わっています", t.pos),
            _ => err("ここに値が必要です", t.pos),
        }
    }

    /// 関数呼び出し。`sin(x)`, `sin x`, `sin^2(x)` の3通りを受け付ける。
    fn call(&mut self, f: Func, pos: usize) -> Result<Expr, ParseError> {
        // sin^2(x) → (sin(x))^2
        let mut power = None;
        if *self.peek() == Tok::Caret {
            self.next();
            power = Some(self.primary()?);
        }

        let args = if *self.peek() == Tok::LParen {
            self.next();
            let mut args = vec![self.expr()?];
            while *self.peek() == Tok::Comma {
                self.next();
                args.push(self.expr()?);
            }
            self.expect(Tok::RParen, "閉じ括弧 ')'")?;
            args
        } else {
            // 括弧なし: 直後の1項（累乗を含む）だけを引数にする。
            // 例) sin x^2 = sin(x^2)、 sin x y = sin(x)·y
            vec![self.power()?]
        };

        if args.len() != f.arity() {
            return err(
                format!("{} の引数は {} 個です（{} 個あります）", f.names()[0], f.arity(), args.len()),
                pos,
            );
        }
        let call = Expr::Call(f, args);
        Ok(match power {
            Some(p) => Expr::Bin(BinOp::Pow, Box::new(call), Box::new(p)),
            None => call,
        })
    }
}

/// 文字列を構文木に変換する
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, i: 0 };
    if *p.peek() == Tok::Eof {
        return err("式が空です", 0);
    }
    let e = p.expr()?;
    if *p.peek() != Tok::Eof {
        return err("ここで式が終わるはずです（余分な文字があります）", p.pos());
    }
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(s: &str) -> Box<Expr> {
        Box::new(Expr::Var(s.into()))
    }

    #[test]
    fn implicit_multiplication() {
        assert_eq!(
            parse("xy").unwrap(),
            Expr::Bin(BinOp::Mul, var("x"), var("y"))
        );
        assert_eq!(
            parse("2x").unwrap(),
            Expr::Bin(BinOp::Mul, Box::new(Expr::Num(2.0)), var("x"))
        );
    }

    #[test]
    fn neg_power_precedence() {
        // -x^2 = -(x^2)
        assert_eq!(
            parse("-x^2").unwrap(),
            Expr::Neg(Box::new(Expr::Bin(BinOp::Pow, var("x"), Box::new(Expr::Num(2.0)))))
        );
    }

    #[test]
    fn function_forms() {
        let a = parse("sin(x)").unwrap();
        assert_eq!(parse("sin x").unwrap(), a);
        assert_eq!(parse("sinx").unwrap(), a);
        assert!(matches!(parse("sinh(x)").unwrap(), Expr::Call(Func::Sinh, _)));
        assert!(matches!(parse("sin^2 x").unwrap(), Expr::Bin(BinOp::Pow, _, _)));
    }

    #[test]
    fn errors() {
        assert!(parse("").is_err());
        assert!(parse("(x+1").is_err());
        assert!(parse("x+").is_err());
        assert!(parse("x $ y").is_err());
    }
}
