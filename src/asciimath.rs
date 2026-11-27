//! ```` ```asciimath ```` blocks: [AsciiMath](https://asciimath.org)
//! (`sum_(i=1)^n i^2 = (n(n+1)(2n+1))/6`) turned into TeX here, then
//! typeset by KaTeX like a ```` ```math ```` block. Each line of the block is
//! an equation.
//!
//! The grammar is AsciiMath's: E ::= I E | I/I E, I ::= S_S^S | S_S | S^S
//! | S, S ::= symbol | (E) | unary S | binary S S, with the brackets around
//! a fraction's, script's or function's argument dropped, and matrices
//! written `[[a,b],[c,d]]`.

use crate::render::escape_text;

/// Turn the block into a display-math block for math-init.js.
pub fn render(code: &str, sourcepos: &str) -> String {
    let lines: Vec<String> = code
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(to_tex)
        .collect();
    let tex = match lines.len() {
        0 => String::new(),
        1 => lines[0].clone(),
        _ => format!(
            "\\begin{{gathered}}{}\\end{{gathered}}",
            lines.join(" \\\\ ")
        ),
    };
    format!(
        "<pre data-math-style=\"display\" lang=\"asciimath\"{sourcepos}><code>{}</code></pre>\n",
        escape_text(&tex)
    )
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// A plain symbol.
    Const,
    /// Takes one argument: `\sqrt{…}`; `{}` is replaced by it.
    Unary,
    /// Takes two: `\frac{…}{…}`; `{0}`/`{1}` are replaced.
    Binary,
    Left,
    Right,
    /// `|`: a left or right bracket depending on where it is.
    Bar,
    /// Big operators whose scripts go under and over (`sum`, `lim`).
    UnderOver,
}

type Sym = (&'static str, &'static str, Kind);

use Kind::*;

/// AsciiMath input, TeX, kind. Longest match wins when tokenising.
const SYMBOLS: &[Sym] = &[
    // Greek
    ("alpha", "\\alpha", Const),
    ("beta", "\\beta", Const),
    ("gamma", "\\gamma", Const),
    ("Gamma", "\\Gamma", Const),
    ("delta", "\\delta", Const),
    ("Delta", "\\Delta", Const),
    ("epsilon", "\\epsilon", Const),
    ("varepsilon", "\\varepsilon", Const),
    ("zeta", "\\zeta", Const),
    ("eta", "\\eta", Const),
    ("theta", "\\theta", Const),
    ("Theta", "\\Theta", Const),
    ("vartheta", "\\vartheta", Const),
    ("iota", "\\iota", Const),
    ("kappa", "\\kappa", Const),
    ("lambda", "\\lambda", Const),
    ("Lambda", "\\Lambda", Const),
    ("mu", "\\mu", Const),
    ("nu", "\\nu", Const),
    ("xi", "\\xi", Const),
    ("Xi", "\\Xi", Const),
    ("pi", "\\pi", Const),
    ("Pi", "\\Pi", Const),
    ("rho", "\\rho", Const),
    ("sigma", "\\sigma", Const),
    ("Sigma", "\\Sigma", Const),
    ("tau", "\\tau", Const),
    ("upsilon", "\\upsilon", Const),
    ("phi", "\\phi", Const),
    ("Phi", "\\Phi", Const),
    ("varphi", "\\varphi", Const),
    ("chi", "\\chi", Const),
    ("psi", "\\psi", Const),
    ("Psi", "\\Psi", Const),
    ("omega", "\\omega", Const),
    ("Omega", "\\Omega", Const),
    // Operators
    ("*", "\\cdot", Const),
    ("**", "\\ast", Const),
    ("***", "\\star", Const),
    ("//", "/", Const),
    ("\\\\", "\\backslash", Const),
    ("setminus", "\\setminus", Const),
    ("xx", "\\times", Const),
    ("|><", "\\ltimes", Const),
    ("><|", "\\rtimes", Const),
    ("|><|", "\\bowtie", Const),
    ("-:", "\\div", Const),
    ("divide", "\\div", Const),
    ("@", "\\circ", Const),
    ("o+", "\\oplus", Const),
    ("ox", "\\otimes", Const),
    ("o.", "\\odot", Const),
    ("sum", "\\sum", UnderOver),
    ("prod", "\\prod", UnderOver),
    ("^^", "\\wedge", Const),
    ("^^^", "\\bigwedge", UnderOver),
    ("vv", "\\vee", Const),
    ("vvv", "\\bigvee", UnderOver),
    ("nn", "\\cap", Const),
    ("nnn", "\\bigcap", UnderOver),
    ("uu", "\\cup", Const),
    ("uuu", "\\bigcup", UnderOver),
    // Relations
    ("!=", "\\ne", Const),
    ("<=", "\\le", Const),
    ("lt=", "\\le", Const),
    (">=", "\\ge", Const),
    ("geq", "\\ge", Const),
    ("lt", "<", Const),
    ("gt", ">", Const),
    ("-<", "\\prec", Const),
    (">-", "\\succ", Const),
    ("-<=", "\\preceq", Const),
    (">-=", "\\succeq", Const),
    ("in", "\\in", Const),
    ("!in", "\\notin", Const),
    ("sub", "\\subset", Const),
    ("sup", "\\supset", Const),
    ("sube", "\\subseteq", Const),
    ("supe", "\\supseteq", Const),
    ("-=", "\\equiv", Const),
    ("~=", "\\cong", Const),
    ("~~", "\\approx", Const),
    ("~", "\\sim", Const),
    ("prop", "\\propto", Const),
    // Logic
    ("and", "\\text{ and }", Const),
    ("or", "\\text{ or }", Const),
    ("not", "\\neg", Const),
    ("=>", "\\implies", Const),
    ("if", "\\text{ if }", Const),
    ("<=>", "\\iff", Const),
    ("AA", "\\forall", Const),
    ("EE", "\\exists", Const),
    ("_|_", "\\bot", Const),
    ("TT", "\\top", Const),
    ("|--", "\\vdash", Const),
    ("|==", "\\models", Const),
    // Brackets
    ("(", "(", Left),
    (")", ")", Right),
    ("[", "[", Left),
    ("]", "]", Right),
    ("{", "\\{", Left),
    ("}", "\\}", Right),
    ("(:", "\\langle", Left),
    (":)", "\\rangle", Right),
    ("<<", "\\langle", Left),
    (">>", "\\rangle", Right),
    ("{:", ".", Left),
    (":}", ".", Right),
    ("|", "|", Bar),
    // Miscellaneous
    ("int", "\\int", Const),
    ("oint", "\\oint", Const),
    ("del", "\\partial", Const),
    ("grad", "\\nabla", Const),
    ("+-", "\\pm", Const),
    ("-+", "\\mp", Const),
    ("O/", "\\emptyset", Const),
    ("oo", "\\infty", Const),
    ("aleph", "\\aleph", Const),
    (":.", "\\therefore", Const),
    (":'", "\\because", Const),
    ("|...|", "|\\ldots|", Const),
    ("|cdots|", "|\\cdots|", Const),
    ("...", "\\ldots", Const),
    ("cdots", "\\cdots", Const),
    ("vdots", "\\vdots", Const),
    ("ddots", "\\ddots", Const),
    ("|~", "\\lceil", Const),
    ("~|", "\\rceil", Const),
    ("|__", "\\lfloor", Const),
    ("__|", "\\rfloor", Const),
    ("/_", "\\angle", Const),
    ("/_\\", "\\triangle", Const),
    ("frown", "\\frown", Const),
    ("diamond", "\\diamond", Const),
    ("square", "\\square", Const),
    ("quad", "\\quad", Const),
    ("qquad", "\\qquad", Const),
    ("CC", "\\mathbb{C}", Const),
    ("NN", "\\mathbb{N}", Const),
    ("QQ", "\\mathbb{Q}", Const),
    ("RR", "\\mathbb{R}", Const),
    ("ZZ", "\\mathbb{Z}", Const),
    // Functions
    ("sin", "\\sin", Const),
    ("cos", "\\cos", Const),
    ("tan", "\\tan", Const),
    ("sec", "\\sec", Const),
    ("csc", "\\csc", Const),
    ("cot", "\\cot", Const),
    ("arcsin", "\\arcsin", Const),
    ("arccos", "\\arccos", Const),
    ("arctan", "\\arctan", Const),
    ("sinh", "\\sinh", Const),
    ("cosh", "\\cosh", Const),
    ("tanh", "\\tanh", Const),
    ("sech", "\\operatorname{sech}", Const),
    ("csch", "\\operatorname{csch}", Const),
    ("coth", "\\coth", Const),
    ("exp", "\\exp", Const),
    ("log", "\\log", Const),
    ("ln", "\\ln", Const),
    ("det", "\\det", Const),
    ("dim", "\\dim", Const),
    ("mod", "\\operatorname{mod}", Const),
    ("gcd", "\\gcd", Const),
    ("lcm", "\\operatorname{lcm}", Const),
    ("lub", "\\operatorname{lub}", Const),
    ("glb", "\\operatorname{glb}", Const),
    ("min", "\\min", UnderOver),
    ("max", "\\max", UnderOver),
    ("lim", "\\lim", UnderOver),
    ("Lim", "\\operatorname*{Lim}", UnderOver),
    // Arrows
    ("uarr", "\\uparrow", Const),
    ("darr", "\\downarrow", Const),
    ("rarr", "\\rightarrow", Const),
    ("->", "\\to", Const),
    (">->", "\\rightarrowtail", Const),
    ("->>", "\\twoheadrightarrow", Const),
    (">->>", "\\twoheadrightarrowtail", Const),
    ("|->", "\\mapsto", Const),
    ("larr", "\\leftarrow", Const),
    ("harr", "\\leftrightarrow", Const),
    ("rArr", "\\Rightarrow", Const),
    ("lArr", "\\Leftarrow", Const),
    ("hArr", "\\Leftrightarrow", Const),
    // Accents and fonts (one argument)
    ("sqrt", "\\sqrt{}", Unary),
    ("abs", "\\left|{}\\right|", Unary),
    ("norm", "\\left\\|{}\\right\\|", Unary),
    ("floor", "\\left\\lfloor{}\\right\\rfloor", Unary),
    ("ceil", "\\left\\lceil{}\\right\\rceil", Unary),
    ("hat", "\\hat{}", Unary),
    ("bar", "\\overline{}", Unary),
    ("overline", "\\overline{}", Unary),
    ("ul", "\\underline{}", Unary),
    ("underline", "\\underline{}", Unary),
    ("vec", "\\vec{}", Unary),
    ("tilde", "\\tilde{}", Unary),
    ("dot", "\\dot{}", Unary),
    ("ddot", "\\ddot{}", Unary),
    ("obrace", "\\overbrace{}", Unary),
    ("ubrace", "\\underbrace{}", Unary),
    ("cancel", "\\cancel{}", Unary),
    ("bb", "\\mathbf{}", Unary),
    ("bbb", "\\mathbb{}", Unary),
    ("cc", "\\mathcal{}", Unary),
    ("tt", "\\mathtt{}", Unary),
    ("fr", "\\mathfrak{}", Unary),
    ("sf", "\\mathsf{}", Unary),
    ("text", "\\text{}", Unary),
    // Two arguments
    ("frac", "\\frac{0}{1}", Binary),
    ("root", "\\sqrt[{0}]{1}", Binary),
    ("stackrel", "\\stackrel{0}{1}", Binary),
    ("overset", "\\overset{0}{1}", Binary),
    ("underset", "\\underset{0}{1}", Binary),
];

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Sym(Sym),
    /// A number, a letter or another character, as TeX.
    Plain(String),
    /// "quoted text".
    Text(String),
    Sub,
    Sup,
    Slash,
    Comma,
}

/// TeX for characters that mean something there.
fn tex_char(c: char) -> String {
    match c {
        '#' | '$' | '%' | '&' | '_' | '{' | '}' => format!("\\{c}"),
        '\\' => "\\backslash".into(),
        '^' => "\\hat{}".into(),
        '~' => "\\sim".into(),
        _ => c.to_string(),
    }
}

fn tokenize(s: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    let b = s.as_bytes();
    while i < s.len() {
        let rest = &s[i..];
        let c = rest.chars().next().unwrap();
        if c.is_whitespace() {
            i += c.len_utf8();
            continue;
        }
        if c == '"' {
            let end = rest[1..].find('"').map_or(rest.len(), |e| e + 1);
            out.push(Token::Text(rest[1..end].to_string()));
            i += (end + 1).min(rest.len());
            continue;
        }
        // Longest symbol first.
        let best = SYMBOLS
            .iter()
            .filter(|(input, _, _)| rest.starts_with(input))
            .max_by_key(|(input, _, _)| input.len());
        // A word that's a symbol must not run on into letters ("sinx" is
        // fine; "integer" isn't "int" + "eger"), except single-letter-ish
        // operators and functions followed by a variable.
        if let Some(sym) = best {
            let after = rest[sym.0.len()..].chars().next();
            let word = sym.0.chars().all(|c| c.is_ascii_alphabetic());
            let runs_on = word && sym.0.len() > 1 && after.is_some_and(|a| a.is_ascii_alphabetic());
            let function = matches!(sym.2, Unary | Binary | UnderOver)
                || sym.1.starts_with("\\operatorname")
                || ["sin", "cos", "tan", "log", "ln", "exp"].contains(&sym.0);
            if !runs_on || function {
                match sym.0 {
                    "_" => out.push(Token::Sub),
                    _ => out.push(Token::Sym(*sym)),
                }
                i += sym.0.len();
                continue;
            }
        }
        match c {
            '_' => {
                out.push(Token::Sub);
                i += 1;
            }
            '^' => {
                out.push(Token::Sup);
                i += 1;
            }
            '/' => {
                out.push(Token::Slash);
                i += 1;
            }
            ',' => {
                out.push(Token::Comma);
                i += 1;
            }
            c if c.is_ascii_digit()
                || (c == '.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) =>
            {
                let len = rest
                    .char_indices()
                    .find(|&(k, ch)| {
                        !(ch.is_ascii_digit()
                            || (ch == '.'
                                && k > 0
                                && rest[k + 1..].starts_with(|d: char| d.is_ascii_digit())))
                    })
                    .map_or(rest.len(), |(k, _)| k);
                out.push(Token::Plain(rest[..len].to_string()));
                i += len;
            }
            c => {
                out.push(Token::Plain(tex_char(c)));
                i += c.len_utf8();
            }
        }
    }
    out
}

struct Parser {
    tokens: Vec<Token>,
    i: usize,
    /// Inside how many `|…|` groups: a `|` then closes one.
    bars: usize,
}

/// A parsed piece: its TeX, and the TeX without its outer brackets (for
/// use as an argument).
#[derive(Clone)]
struct Piece {
    tex: String,
    bare: String,
}

impl Piece {
    fn plain(tex: String) -> Self {
        Self {
            bare: tex.clone(),
            tex,
        }
    }
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.i)
    }

    /// S: a simple expression.
    fn simple(&mut self) -> Option<Piece> {
        let t = self.peek()?.clone();
        match t {
            Token::Sym((_, tex, kind)) => match kind {
                Left => {
                    self.i += 1;
                    if let Some(m) = self.matrix(tex) {
                        return Some(Piece::plain(m));
                    }
                    let inner = self.expression(true);
                    let right = match self.peek() {
                        Some(Token::Sym((_, r, Right | Bar))) => {
                            let r = *r;
                            self.i += 1;
                            r
                        }
                        _ => ".",
                    };
                    let tex = if tex == "(" && right == ")" || tex == "[" && right == "]" {
                        format!("\\left{tex}{inner}\\right{right}")
                    } else {
                        // A space ends a command such as \langle before the
                        // letters that follow.
                        format!("\\left{} {inner}\\right{}", bracket(tex), bracket(right))
                    };
                    Some(Piece { tex, bare: inner })
                }
                Right => None,
                Bar => {
                    self.i += 1;
                    // |x| as absolute value when a closing bar follows.
                    let save = self.i;
                    self.bars += 1;
                    let inner = self.expression(true);
                    self.bars -= 1;
                    if matches!(self.peek(), Some(Token::Sym((_, _, Bar)))) {
                        self.i += 1;
                        Some(Piece::plain(format!("\\left|{inner}\\right|")))
                    } else {
                        self.i = save;
                        Some(Piece::plain("|".into()))
                    }
                }
                Unary => {
                    self.i += 1;
                    let arg = self.simple().map_or(String::new(), |p| p.bare);
                    Some(Piece::plain(if tex == "\\text{}" {
                        format!("\\text{{{}}}", strip_tex_escapes(&arg))
                    } else {
                        tex.replacen("{}", &format!("{{{arg}}}"), 1)
                    }))
                }
                Binary => {
                    self.i += 1;
                    let a = self.simple().map_or(String::new(), |p| p.bare);
                    let b = self.simple().map_or(String::new(), |p| p.bare);
                    Some(Piece::plain(
                        tex.replacen("{0}", &format!("{{{a}}}"), 1).replacen(
                            "{1}",
                            &format!("{{{b}}}"),
                            1,
                        ),
                    ))
                }
                Const | UnderOver => {
                    self.i += 1;
                    Some(Piece::plain(tex.to_string()))
                }
            },
            Token::Plain(p) => {
                self.i += 1;
                Some(Piece::plain(p))
            }
            Token::Text(t) => {
                self.i += 1;
                Some(Piece::plain(format!("\\text{{{}}}", text_escape(&t))))
            }
            Token::Comma => {
                self.i += 1;
                Some(Piece::plain(",".into()))
            }
            Token::Sub | Token::Sup | Token::Slash => {
                self.i += 1;
                Some(Piece::plain(match t {
                    Token::Sub => "\\_".into(),
                    Token::Sup => "\\hat{}".into(),
                    _ => "/".into(),
                }))
            }
        }
    }

    /// I: S with an optional subscript and superscript.
    fn intermediate(&mut self) -> Option<Piece> {
        let base = self.simple()?;
        let big = base.tex.starts_with("\\sum")
            || base.tex.starts_with("\\prod")
            || base.tex.starts_with("\\lim")
            || base.tex.starts_with("\\big")
            || base.tex.starts_with("\\min")
            || base.tex.starts_with("\\max")
            || base.tex.starts_with("\\int")
            || base.tex.starts_with("\\oint");
        let mut tex = base.tex.clone();
        let mut scripted = false;
        if self.peek() == Some(&Token::Sub) {
            self.i += 1;
            let s = self.simple().map_or(String::new(), |p| p.bare);
            tex = format!("{{{tex}}}_{{{s}}}");
            if big {
                tex = format!("{}_{{{s}}}", base.tex);
            }
            scripted = true;
        }
        if self.peek() == Some(&Token::Sup) {
            self.i += 1;
            let s = self.simple().map_or(String::new(), |p| p.bare);
            if !scripted && !big {
                tex = format!("{{{tex}}}");
            }
            tex = format!("{tex}^{{{s}}}");
            scripted = true;
        }
        Some(if scripted { Piece::plain(tex) } else { base })
    }

    /// E: a sequence of I, with `/` making fractions. Stops at a right
    /// bracket (and, with `in_brackets`, leaves it for the caller).
    fn expression(&mut self, in_brackets: bool) -> String {
        let mut out = String::new();
        loop {
            match self.peek() {
                None => break,
                Some(Token::Sym((_, _, Right))) => {
                    if in_brackets {
                        break;
                    }
                    // A stray closing bracket: keep it as a symbol.
                    if let Some(Token::Sym((_, tex, _))) = self.peek().cloned() {
                        out.push_str(&bracket_symbol(tex));
                    }
                    self.i += 1;
                    continue;
                }
                Some(Token::Sym((_, _, Bar))) if in_brackets && self.bars > 0 => break,
                _ => {}
            }
            let Some(first) = self.intermediate() else {
                break;
            };
            if self.peek() == Some(&Token::Slash) {
                self.i += 1;
                let below = self.intermediate().map_or(String::new(), |p| p.bare);
                out.push_str(&format!("\\frac{{{}}}{{{below}}}", first.bare));
            } else {
                out.push_str(&first.tex);
            }
            out.push(' ');
        }
        out.trim_end().to_string()
    }

    /// `[[a,b],[c,d]]` (or with parentheses): rows in brackets, separated
    /// by commas, all the same length. Called after the outer left
    /// bracket; on anything else, puts the position back.
    fn matrix(&mut self, outer: &str) -> Option<String> {
        let start = self.i;
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut inner_bracket = None;
        loop {
            match self.peek() {
                Some(Token::Sym((_, l, Left))) if *l == "(" || *l == "[" => {
                    if inner_bracket.is_some_and(|b| b != *l) {
                        break;
                    }
                    inner_bracket = Some(*l);
                    self.i += 1;
                }
                _ => break,
            }
            let mut row = Vec::new();
            loop {
                let cell = self.cell();
                row.push(cell);
                match self.peek() {
                    Some(Token::Comma) => self.i += 1,
                    Some(Token::Sym((_, _, Right))) => {
                        self.i += 1;
                        break;
                    }
                    _ => {
                        self.i = start;
                        return None;
                    }
                }
            }
            rows.push(row);
            match self.peek() {
                Some(Token::Comma) => self.i += 1,
                Some(Token::Sym((_, _, Right))) => {
                    self.i += 1;
                    break;
                }
                _ => {
                    self.i = start;
                    return None;
                }
            }
        }
        let cols = rows.first().map_or(0, Vec::len);
        if rows.len() < 2 || cols < 2 && rows.len() < 2 || rows.iter().any(|r| r.len() != cols) {
            self.i = start;
            return None;
        }
        let env = match outer {
            "(" => "pmatrix",
            "[" => "bmatrix",
            "\\{" => "Bmatrix",
            "|" => "vmatrix",
            _ => "matrix",
        };
        let body: Vec<String> = rows.iter().map(|r| r.join(" & ")).collect();
        Some(format!(
            "\\begin{{{env}}}{}\\end{{{env}}}",
            body.join(" \\\\ ")
        ))
    }

    /// A matrix cell: an expression up to a comma or right bracket.
    fn cell(&mut self) -> String {
        let mut out = String::new();
        while let Some(t) = self.peek() {
            if matches!(t, Token::Comma | Token::Sym((_, _, Right))) {
                break;
            }
            let Some(p) = self.intermediate() else {
                break;
            };
            if self.peek() == Some(&Token::Slash) {
                self.i += 1;
                let below = self.intermediate().map_or(String::new(), |p| p.bare);
                out.push_str(&format!("\\frac{{{}}}{{{below}}}", p.bare));
            } else {
                out.push_str(&p.tex);
            }
            out.push(' ');
        }
        out.trim_end().to_string()
    }
}

fn bracket(tex: &str) -> &str {
    match tex {
        "." => ".",
        t => t,
    }
}

fn bracket_symbol(tex: &str) -> String {
    match tex {
        "." => String::new(),
        t => t.to_string(),
    }
}

fn text_escape(s: &str) -> String {
    s.chars().map(tex_char).collect()
}

/// `text(abc)` arrives as TeX letters; undo `\` escapes made for them.
fn strip_tex_escapes(s: &str) -> String {
    s.replace(' ', "\\ ")
}

/// One AsciiMath expression as TeX.
pub fn to_tex(s: &str) -> String {
    let mut p = Parser {
        tokens: tokenize(s),
        i: 0,
        bars: 0,
    };
    p.expression(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions() {
        for (am, tex) in [
            ("x^2 + y_1", "{x}^{2} + {y}_{1}"),
            ("sum_(i=1)^n i^2", "\\sum_{i = 1}^{n} {i}^{2}"),
            ("(a+b)/2", "\\frac{a + b}{2}"),
            ("sqrt(x) + root(3)(y)", "\\sqrt{x} + \\sqrt[{3}]{y}"),
            ("a xx b != c", "a \\times b \\ne c"),
            ("int_0^1 f(x) dx", "\\int_{0}^{1} f \\left(x\\right) d x"),
            (
                "[[a,b],[c,d]]",
                "\\begin{bmatrix}a & b \\\\ c & d\\end{bmatrix}",
            ),
            ("abs(x) <= 1", "\\left|{x}\\right| \\le 1"),
            ("|x|", "\\left|x\\right|"),
            ("(|x|+1)", "\\left(\\left|x\\right| + 1\\right)"),
            ("\"if \" x > 0", "\\text{if } x > 0"),
            (
                "lim_(x->oo) 1/x = 0",
                "\\lim_{x \\to \\infty} \\frac{1}{x} = 0",
            ),
            ("sin x + alpha", "\\sin x + \\alpha"),
            ("100%", "100 \\%"),
            ("hat x vec v bb R", "\\hat{x} \\vec{v} \\mathbf{R}"),
            ("(:a, b:)", "\\left\\langle a , b\\right\\rangle"),
        ] {
            assert_eq!(to_tex(am), tex, "{am}");
        }
    }

    #[test]
    fn blocks_become_display_math() {
        let html = render("x^2\n\ny = 1\n", " data-sourcepos=\"1:1-4:3\"");
        assert_eq!(
            html,
            "<pre data-math-style=\"display\" lang=\"asciimath\" data-sourcepos=\"1:1-4:3\"><code>\
             \\begin{gathered}{x}^{2} \\\\ y = 1\\end{gathered}</code></pre>\n"
        );
        assert!(render("a<b", "").contains("a &lt; b"));
    }
}
