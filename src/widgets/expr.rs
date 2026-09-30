//! Small, safe arithmetic-expression parser for the `interactive` widget.
//!
//! Expressions are parsed server-side into a JSON AST that the client evaluates
//! directly — the client never receives source text it must `eval`.

use serde_json::{json, Value};

use crate::widgets::WidgetError;

const FUNCTIONS: [&str; 9] = ["exp", "log", "sqrt", "abs", "min", "max", "pow", "sin", "cos"];
const CONSTANTS: [&str; 1] = ["pi"];

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Var(String),
    Bin(String, Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    Call(String, Vec<Expr>),
}

impl Expr {
    pub fn to_ast(&self) -> Value {
        match self {
            Expr::Num(n) => json!(["n", n]),
            Expr::Var(name) => json!(["v", name]),
            Expr::Bin(op, l, r) => json!([op, l.to_ast(), r.to_ast()]),
            Expr::Neg(e) => json!(["neg", e.to_ast()]),
            Expr::Call(name, args) => {
                let a: Vec<Value> = args.iter().map(|e| e.to_ast()).collect();
                json!(["f", name, a])
            }
        }
    }
}

struct Lexer<'a> {
    chars: Vec<char>,
    pos: usize,
    src: &'a str,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
    Eof,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Lexer<'a> {
        Lexer {
            chars: src.chars().collect(),
            pos: 0,
            src,
        }
    }

    fn err(&self, msg: &str) -> WidgetError {
        WidgetError::Invalid(format!("expression '{}': {}", self.src, msg))
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn next(&mut self) -> Result<Tok, WidgetError> {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.bump();
        }
        let Some(c) = self.bump() else {
            return Ok(Tok::Eof);
        };
        match c {
            '+' => Ok(Tok::Plus),
            '-' => Ok(Tok::Minus),
            '*' => Ok(Tok::Star),
            '/' => Ok(Tok::Slash),
            '^' => Ok(Tok::Caret),
            '(' => Ok(Tok::LParen),
            ')' => Ok(Tok::RParen),
            ',' => Ok(Tok::Comma),
            c if c.is_ascii_digit() || c == '.' => {
                let mut s = String::from(c);
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '_' {
                        if (c == 'e' || c == 'E')
                            && !self
                                .chars
                                .get(self.pos + 1)
                                .map(|n| n.is_ascii_digit() || *n == '+' || *n == '-')
                                .unwrap_or(false)
                        {
                            break;
                        }
                        s.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
                s = s.replace('_', "");
                s.parse::<f64>()
                    .map(Tok::Num)
                    .map_err(|_| self.err(&format!("bad number literal '{s}'")))
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut s = String::from(c);
                while let Some(c) = self.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        s.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
                Ok(Tok::Ident(s))
            }
            other => Err(self.err(&format!("unexpected character '{other}'"))),
        }
    }
}

struct Parser<'a> {
    lex: Lexer<'a>,
    cur: Tok,
    params: &'a [String],
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, params: &'a [String]) -> Result<Parser<'a>, WidgetError> {
        let mut lex = Lexer::new(src);
        let cur = lex.next()?;
        Ok(Parser { lex, cur, params })
    }

    fn err(&self, msg: &str) -> WidgetError {
        self.lex.err(msg)
    }

    fn advance(&mut self) -> Result<(), WidgetError> {
        self.cur = self.lex.next()?;
        Ok(())
    }

    fn ident(&self, name: &str) -> Result<Expr, WidgetError> {
        if name == "x" {
            return Ok(Expr::Var("x".into()));
        }
        if CONSTANTS.contains(&name) {
            return Ok(Expr::Num(std::f64::consts::PI));
        }
        if FUNCTIONS.contains(&name) {
            return Err(self.err(&format!("function '{name}' used as a variable")));
        }
        if self.params.iter().any(|p| p == name) {
            return Ok(Expr::Var(name.into()));
        }
        Err(self.err(&format!(
            "unknown identifier '{name}' (allowed: x, pi, exp, log, sqrt, abs, min, max, pow, sin, cos, or a declared parameter)"
        )))
    }

    fn primary(&mut self) -> Result<Expr, WidgetError> {
        let tok = self.cur.clone();
        match tok {
            Tok::Num(n) => {
                self.advance()?;
                Ok(Expr::Num(n))
            }
            Tok::Ident(name) => {
                self.advance()?;
                if FUNCTIONS.contains(&name.as_str()) {
                    if self.cur != Tok::LParen {
                        return Err(self.err(&format!("function '{name}' must be called with '('")));
                    }
                    self.advance()?;
                    let mut args = vec![self.expr()?];
                    while self.cur == Tok::Comma {
                        self.advance()?;
                        args.push(self.expr()?);
                    }
                    if self.cur != Tok::RParen {
                        return Err(self.err("expected ')' after function arguments"));
                    }
                    self.advance()?;
                    let arity = match name.as_str() {
                        "exp" | "log" | "sqrt" | "abs" | "sin" | "cos" => 1,
                        "min" | "max" | "pow" => 2,
                        _ => unreachable!(),
                    };
                    if args.len() != arity {
                        return Err(self.err(&format!(
                            "function '{name}' takes {arity} argument(s), got {}",
                            args.len()
                        )));
                    }
                    Ok(Expr::Call(name, args))
                } else {
                    self.ident(&name)
                }
            }
            Tok::LParen => {
                self.advance()?;
                let e = self.expr()?;
                if self.cur != Tok::RParen {
                    return Err(self.err("expected ')'"));
                }
                self.advance()?;
                Ok(e)
            }
            Tok::Minus => {
                self.advance()?;
                Ok(Expr::Neg(Box::new(self.unary()?)))
            }
            Tok::Plus => {
                self.advance()?;
                self.unary()
            }
            Tok::Eof => Err(self.err("unexpected end of expression")),
            _ => Err(self.err("expected a number, identifier, '(', or unary '-'")),
        }
    }

    fn unary(&mut self) -> Result<Expr, WidgetError> {
        if self.cur == Tok::Minus {
            self.advance()?;
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.cur == Tok::Plus {
            self.advance()?;
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Result<Expr, WidgetError> {
        let base = self.primary()?;
        if self.cur == Tok::Caret {
            self.advance()?;
            let exp = self.unary()?;
            return Ok(Expr::Bin("^".into(), Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn term(&mut self) -> Result<Expr, WidgetError> {
        let mut left = self.unary()?;
        loop {
            let op = match self.cur {
                Tok::Star => "*",
                Tok::Slash => "/",
                _ => break,
            };
            self.advance()?;
            let right = self.unary()?;
            left = Expr::Bin(op.into(), Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn expr(&mut self) -> Result<Expr, WidgetError> {
        let mut left = self.term()?;
        loop {
            let op = match self.cur {
                Tok::Plus => "+",
                Tok::Minus => "-",
                _ => break,
            };
            self.advance()?;
            let right = self.term()?;
            left = Expr::Bin(op.into(), Box::new(left), Box::new(right));
        }
        Ok(left)
    }
}

fn validate_param_names(params: &[String], src: &str) -> Result<(), WidgetError> {
    let invalid = |name: &str| {
        !name.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false)
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    for p in params {
        if p == "x" || CONSTANTS.contains(&p.as_str()) || FUNCTIONS.contains(&p.as_str()) {
            return Err(WidgetError::Invalid(format!(
                "expression '{src}': parameter name '{p}' is reserved"
            )));
        }
        if invalid(p) {
            return Err(WidgetError::Invalid(format!(
                "expression '{src}': parameter name '{p}' must be an identifier"
            )));
        }
    }
    let mut seen = std::collections::HashSet::new();
    for p in params {
        if !seen.insert(p) {
            return Err(WidgetError::Invalid(format!(
                "expression '{src}': duplicate parameter '{p}'"
            )));
        }
    }
    Ok(())
}

/// Parse `src` over variable `x` and `params` into a JSON AST.
/// Errors when any input remains unconsumed.
pub fn parse(src: &str, params: &[String]) -> Result<Value, WidgetError> {
    validate_param_names(params, src)?;
    let mut p = Parser::new(src, params)?;
    let e = p.expr()?;
    if p.cur != Tok::Eof {
        return Err(p.lex.err("unexpected trailing input"));
    }
    Ok(e.to_ast())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_arithmetic_to_ast() {
        let ast = parse("B + a*x^2", &names(&["B", "a"])).unwrap();
        assert_eq!(
            ast,
            json!(["+", ["v", "B"], ["*", ["v", "a"], ["^", ["v", "x"], ["n", 2.0]]]])
        );
    }

    #[test]
    fn parses_precedence_and_unary() {
        let ast = parse("-2*x + 1/3", &[]).unwrap();
        assert_eq!(
            ast,
            json!(["+", ["*", ["neg", ["n", 2.0]], ["v", "x"]], ["/", ["n", 1.0], ["n", 3.0]]])
        );
    }

    #[test]
    fn parses_calls() {
        let ast = parse("sqrt(B^2 + 4) * exp(-x)", &names(&["B"])).unwrap();
        assert!(matches!(ast, Value::Array(_)));
        let flat = serde_json::to_string(&ast).unwrap();
        assert!(flat.contains("\"sqrt\"") && flat.contains("\"exp\""));
    }

    #[test]
    fn rejects_unknown_identifiers() {
        assert!(parse("y + 1", &[]).is_err());
        assert!(parse("tan(x)", &[]).is_err());
    }

    #[test]
    fn supports_sin_cos_and_pi() {
        let ast = parse("sin(2 * pi * x) + cos(pi)", &[]).unwrap();
        assert!(serde_json::to_string(&ast).unwrap().contains("sin"));
        assert!(parse("x", &names(&["pi"])).is_err());
    }

    #[test]
    fn rejects_reserved_and_bad_param_names() {
        assert!(parse("x", &names(&["x"])).is_err());
        assert!(parse("x", &names(&["exp"])).is_err());
        assert!(parse("x", &names(&["9bad"])).is_err());
        assert!(parse("x", &names(&["a", "a"])).is_err());
    }

    #[test]
    fn rejects_bad_syntax_and_trailing_input() {
        assert!(parse("x +", &[]).is_err());
        assert!(parse("(x", &[]).is_err());
        assert!(parse("x 2", &[]).is_err());
        assert!(parse("", &[]).is_err());
        assert!(parse("exp(x", &[]).is_err());
        assert!(parse("min(x)", &[]).is_err()); // arity
        assert!(parse("log(x)", &[]).is_ok());
        assert!(parse("pow(x, 2)", &[]).is_ok());
    }
}