//! Linguagem de expressões dos plugins: só aritmética, sem efeitos.
//!
//! Gramática: números, variáveis, `+ - * / ^`, parênteses, comparações `< > <= >=`
//! (valem 1 ou 0) e as funções `sin cos tan asin acos atan atan2 exp ln log10 sqrt abs
//! min max clamp if sign pow`. A avaliação não tem laços nem acesso a nada além das
//! variáveis passadas: o custo por avaliação é limitado pelo tamanho da árvore, e a
//! árvore é limitada a [`MAX_NODES`] nós.

use std::collections::BTreeMap;

/// Tamanho máximo de uma expressão (nós da árvore).
pub const MAX_NODES: usize = 2_000;

/// Expressão compilada.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// Número.
    Num(f64),
    /// Variável (índice na tabela de nomes).
    Var(String),
    /// Operação binária.
    Bin(char, Box<Expr>, Box<Expr>),
    /// Comparação ('<', '>', 'l' = <=, 'g' = >=).
    Cmp(char, Box<Expr>, Box<Expr>),
    /// Negação.
    Neg(Box<Expr>),
    /// Função.
    Call(String, Vec<Expr>),
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    nodes: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn node(&mut self) -> Result<(), String> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            Err(format!("expressão grande demais (mais de {MAX_NODES} nós)"))
        } else {
            Ok(())
        }
    }
    fn cmp(&mut self) -> Result<Expr, String> {
        let a = self.sum()?;
        match self.peek() {
            Some(c @ (b'<' | b'>')) => {
                self.i += 1;
                let op = if self.s.get(self.i) == Some(&b'=') {
                    self.i += 1;
                    if c == b'<' {
                        'l'
                    } else {
                        'g'
                    }
                } else {
                    c as char
                };
                self.node()?;
                Ok(Expr::Cmp(op, Box::new(a), Box::new(self.sum()?)))
            }
            _ => Ok(a),
        }
    }
    fn sum(&mut self) -> Result<Expr, String> {
        let mut a = self.prod()?;
        while let Some(c @ (b'+' | b'-')) = self.peek() {
            self.i += 1;
            self.node()?;
            a = Expr::Bin(c as char, Box::new(a), Box::new(self.prod()?));
        }
        Ok(a)
    }
    fn prod(&mut self) -> Result<Expr, String> {
        let mut a = self.unary()?;
        while let Some(c @ (b'*' | b'/')) = self.peek() {
            self.i += 1;
            self.node()?;
            a = Expr::Bin(c as char, Box::new(a), Box::new(self.unary()?));
        }
        Ok(a)
    }
    fn unary(&mut self) -> Result<Expr, String> {
        if self.peek() == Some(b'-') {
            self.i += 1;
            self.node()?;
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.peek() == Some(b'+') {
            self.i += 1;
        }
        self.pow()
    }
    fn pow(&mut self) -> Result<Expr, String> {
        let a = self.atom()?;
        if self.peek() == Some(b'^') {
            self.i += 1;
            self.node()?;
            return Ok(Expr::Bin('^', Box::new(a), Box::new(self.unary()?)));
        }
        Ok(a)
    }
    fn atom(&mut self) -> Result<Expr, String> {
        self.node()?;
        match self.peek() {
            Some(b'(') => {
                self.i += 1;
                let e = self.cmp()?;
                if self.peek() != Some(b')') {
                    return Err(format!("falta ')' na posição {}", self.i));
                }
                self.i += 1;
                Ok(e)
            }
            Some(c) if c.is_ascii_digit() || c == b'.' => {
                let st = self.i;
                while self.i < self.s.len()
                    && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.')
                {
                    self.i += 1;
                }
                if self.i < self.s.len() && (self.s[self.i] == b'e' || self.s[self.i] == b'E') {
                    self.i += 1;
                    if self.i < self.s.len() && (self.s[self.i] == b'-' || self.s[self.i] == b'+') {
                        self.i += 1;
                    }
                    while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                        self.i += 1;
                    }
                }
                let txt = std::str::from_utf8(&self.s[st..self.i]).unwrap_or("");
                txt.parse::<f64>()
                    .map(Expr::Num)
                    .map_err(|_| format!("número inválido: {txt}"))
            }
            Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                let st = self.i;
                while self.i < self.s.len()
                    && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_')
                {
                    self.i += 1;
                }
                let name = std::str::from_utf8(&self.s[st..self.i])
                    .unwrap_or("")
                    .to_owned();
                if self.peek() == Some(b'(') {
                    self.i += 1;
                    let mut args = Vec::new();
                    if self.peek() != Some(b')') {
                        loop {
                            args.push(self.cmp()?);
                            match self.peek() {
                                Some(b',') => self.i += 1,
                                Some(b')') => break,
                                _ => return Err(format!("argumentos de {name} malformados")),
                            }
                        }
                    }
                    self.i += 1;
                    let arity = match name.as_str() {
                        "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "exp" | "ln"
                        | "log10" | "sqrt" | "abs" | "sign" => 1,
                        "atan2" | "min" | "max" | "pow" => 2,
                        "clamp" | "if" => 3,
                        _ => return Err(format!("função desconhecida: {name}")),
                    };
                    if args.len() != arity {
                        return Err(format!("{name} pede {arity} argumento(s)"));
                    }
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            other => Err(format!(
                "símbolo inesperado {:?} na posição {}",
                other.map(|c| c as char),
                self.i
            )),
        }
    }
}

impl Expr {
    /// Compila uma expressão.
    ///
    /// # Errors
    /// Mensagem com a posição do erro.
    pub fn parse(src: &str) -> Result<Self, String> {
        let mut p = Parser {
            s: src.as_bytes(),
            i: 0,
            nodes: 0,
        };
        let e = p.cmp()?;
        if p.peek().is_some() {
            return Err(format!("sobrou texto na posição {}", p.i));
        }
        Ok(e)
    }

    /// Variáveis usadas.
    pub fn vars(&self, out: &mut Vec<String>) {
        match self {
            Self::Num(_) => {}
            Self::Var(v) => {
                if !out.contains(v) {
                    out.push(v.clone());
                }
            }
            Self::Bin(_, a, b) | Self::Cmp(_, a, b) => {
                a.vars(out);
                b.vars(out);
            }
            Self::Neg(a) => a.vars(out),
            Self::Call(_, args) => args.iter().for_each(|a| a.vars(out)),
        }
    }

    /// Avalia com as variáveis dadas (variável ausente vale NaN).
    #[must_use]
    pub fn eval(&self, env: &BTreeMap<String, f64>) -> f64 {
        match self {
            Self::Num(x) => *x,
            Self::Var(v) => env.get(v).copied().unwrap_or(f64::NAN),
            Self::Neg(a) => -a.eval(env),
            Self::Bin(op, a, b) => {
                let (x, y) = (a.eval(env), b.eval(env));
                match op {
                    '+' => x + y,
                    '-' => x - y,
                    '*' => x * y,
                    '/' => x / y,
                    _ => x.powf(y),
                }
            }
            Self::Cmp(op, a, b) => {
                let (x, y) = (a.eval(env), b.eval(env));
                let r = match op {
                    '<' => x < y,
                    '>' => x > y,
                    'l' => x <= y,
                    _ => x >= y,
                };
                if r {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Call(f, a) => {
                let v = |i: usize| a[i].eval(env);
                match f.as_str() {
                    "sin" => v(0).sin(),
                    "cos" => v(0).cos(),
                    "tan" => v(0).tan(),
                    "asin" => v(0).asin(),
                    "acos" => v(0).acos(),
                    "atan" => v(0).atan(),
                    "exp" => v(0).exp(),
                    "ln" => v(0).ln(),
                    "log10" => v(0).log10(),
                    "sqrt" => v(0).sqrt(),
                    "abs" => v(0).abs(),
                    "sign" => v(0).signum(),
                    "atan2" => v(0).atan2(v(1)),
                    "min" => v(0).min(v(1)),
                    "max" => v(0).max(v(1)),
                    "pow" => v(0).powf(v(1)),
                    "clamp" => v(0).clamp(v(1).min(v(2)), v(2).max(v(1))),
                    _ => {
                        if v(0) != 0.0 {
                            v(1)
                        } else {
                            v(2)
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str, vars: &[(&str, f64)]) -> f64 {
        let env = vars.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect();
        Expr::parse(s).unwrap().eval(&env)
    }

    #[test]
    fn arithmetic_precedence_and_functions() {
        assert_eq!(ev("1 + 2 * 3", &[]), 7.0);
        assert_eq!(ev("-2^2", &[]), -4.0);
        assert_eq!(ev("2^3^2", &[]), 512.0);
        assert_eq!(ev("(1 + 2) * 3", &[]), 9.0);
        assert!(
            (ev("sqrt(2) * sin(pi/2)", &[("pi", core::f64::consts::PI)]) - 2.0_f64.sqrt()).abs()
                < 1e-15
        );
        assert_eq!(ev("if(x > 3, 10, 20)", &[("x", 4.0)]), 10.0);
        assert_eq!(ev("clamp(x, 0, 1)", &[("x", 7.0)]), 1.0);
        assert_eq!(
            ev(
                "min(F, P / max(v, 1))",
                &[("F", 500.0), ("P", 1000.0), ("v", 4.0)]
            ),
            250.0
        );
        assert_eq!(ev("1.5e3", &[]), 1500.0);
        assert!(ev("y", &[]).is_nan());
    }

    #[test]
    fn errors_and_size_limit() {
        assert!(Expr::parse("2 +").is_err());
        assert!(Expr::parse("foo(1)").unwrap_err().contains("desconhecida"));
        assert!(Expr::parse("min(1)").unwrap_err().contains("2 argumento"));
        assert!(Expr::parse("(1 + 2").is_err());
        let big = vec!["x"; MAX_NODES].join("+");
        assert!(Expr::parse(&big).unwrap_err().contains("grande demais"));
        let mut v = Vec::new();
        Expr::parse("a * b + sin(c) + a").unwrap().vars(&mut v);
        assert_eq!(v, ["a", "b", "c"]);
    }
}
