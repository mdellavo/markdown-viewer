//! Chess boards: ```` ```fen ```` shows a position, ```` ```pgn ```` the
//! final position of a game (moves are played out from the start, or from
//! its `[FEN]` tag), with the players, the result and the last move
//! marked. Drawn as SVG with the Unicode chess pieces.

use std::fmt::Write;

use crate::diagrams::{figure, invalid};
use crate::render::escape_text;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Color {
    White,
    Black,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    King,
    Queen,
    Rook,
    Bishop,
    Knight,
    Pawn,
}

type Piece = (Color, Kind);

/// Squares are 0..64: a1 = 0, h1 = 7, a8 = 56.
#[derive(Clone, Debug)]
struct Position {
    board: [Option<Piece>; 64],
    turn: Color,
    /// White king side, white queen side, black king side, black queen side.
    castling: [bool; 4],
    en_passant: Option<usize>,
    fullmove: u32,
}

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn kind_of(c: char) -> Option<Kind> {
    Some(match c.to_ascii_lowercase() {
        'k' => Kind::King,
        'q' => Kind::Queen,
        'r' => Kind::Rook,
        'b' => Kind::Bishop,
        'n' => Kind::Knight,
        'p' => Kind::Pawn,
        _ => return None,
    })
}

fn square(name: &str) -> Option<usize> {
    let b = name.as_bytes();
    if b.len() != 2 || !(b'a'..=b'h').contains(&b[0]) || !(b'1'..=b'8').contains(&b[1]) {
        return None;
    }
    Some((b[1] - b'1') as usize * 8 + (b[0] - b'a') as usize)
}

fn other(c: Color) -> Color {
    match c {
        Color::White => Color::Black,
        Color::Black => Color::White,
    }
}

impl Position {
    fn from_fen(fen: &str) -> Result<Self, String> {
        let mut parts = fen.split_whitespace();
        let placement = parts.next().ok_or("an empty FEN")?;
        let mut board = [None; 64];
        let ranks: Vec<&str> = placement.split('/').collect();
        if ranks.len() != 8 {
            return Err("a FEN board has 8 ranks separated by \"/\"".into());
        }
        for (r, rank) in ranks.iter().enumerate() {
            let row = 7 - r;
            let mut file = 0usize;
            for c in rank.chars() {
                if let Some(d) = c.to_digit(10) {
                    file += d as usize;
                } else {
                    let kind = kind_of(c).ok_or_else(|| format!("{c:?} isn't a piece"))?;
                    let color = if c.is_ascii_uppercase() {
                        Color::White
                    } else {
                        Color::Black
                    };
                    if file >= 8 {
                        return Err(format!("rank {} has more than 8 squares", 8 - r));
                    }
                    board[row * 8 + file] = Some((color, kind));
                    file += 1;
                }
                if file > 8 {
                    return Err(format!("rank {} has more than 8 squares", 8 - r));
                }
            }
            if file != 8 {
                return Err(format!("rank {} has {file} squares, not 8", 8 - r));
            }
        }
        let turn = match parts.next() {
            Some("b") => Color::Black,
            _ => Color::White,
        };
        let rights = parts.next().unwrap_or("-");
        let castling = ['K', 'Q', 'k', 'q'].map(|c| rights.contains(c));
        let en_passant = parts.next().and_then(square);
        let fullmove = parts.nth(1).and_then(|n| n.parse().ok()).unwrap_or(1);
        Ok(Self {
            board,
            turn,
            castling,
            en_passant,
            fullmove,
        })
    }

    fn king(&self, color: Color) -> Option<usize> {
        (0..64).find(|&s| self.board[s] == Some((color, Kind::King)))
    }

    /// Is `sq` attacked by a piece of `by`?
    fn attacked(&self, sq: usize, by: Color) -> bool {
        (0..64).any(|from| match self.board[from] {
            Some((c, k)) if c == by => self.reaches(from, sq, c, k, true),
            _ => false,
        })
    }

    /// Can the piece on `from` move to (or, with `capture_only`, attack)
    /// `to`, ignoring checks? Pawn pushes and en passant included.
    fn reaches(
        &self,
        from: usize,
        to: usize,
        color: Color,
        kind: Kind,
        capture_only: bool,
    ) -> bool {
        if from == to {
            return false;
        }
        let (fx, fy) = ((from % 8) as i32, (from / 8) as i32);
        let (tx, ty) = ((to % 8) as i32, (to / 8) as i32);
        let (dx, dy) = (tx - fx, ty - fy);
        let target = self.board[to];
        if target.is_some_and(|(c, _)| c == color) {
            return false;
        }
        let clear = |sx: i32, sy: i32| {
            let (mut x, mut y) = (fx + sx, fy + sy);
            while (x, y) != (tx, ty) {
                if self.board[(y * 8 + x) as usize].is_some() {
                    return false;
                }
                x += sx;
                y += sy;
            }
            true
        };
        match kind {
            Kind::King => dx.abs() <= 1 && dy.abs() <= 1,
            Kind::Knight => (dx.abs(), dy.abs()) == (1, 2) || (dx.abs(), dy.abs()) == (2, 1),
            Kind::Rook => (dx == 0 || dy == 0) && clear(dx.signum(), dy.signum()),
            Kind::Bishop => dx.abs() == dy.abs() && clear(dx.signum(), dy.signum()),
            Kind::Queen => {
                (dx == 0 || dy == 0 || dx.abs() == dy.abs()) && clear(dx.signum(), dy.signum())
            }
            Kind::Pawn => {
                let dir = if color == Color::White { 1 } else { -1 };
                let start = if color == Color::White { 1 } else { 6 };
                if dx.abs() == 1 && dy == dir {
                    return capture_only || target.is_some() || self.en_passant == Some(to);
                }
                if capture_only || dx != 0 || target.is_some() {
                    return false;
                }
                dy == dir
                    || (dy == 2 * dir
                        && fy == start
                        && self.board[(fy + dir) as usize * 8 + fx as usize].is_none())
            }
        }
    }

    /// Play a move without checking it; returns the new position.
    fn play(&self, from: usize, to: usize, promote: Option<Kind>) -> Self {
        let mut p = self.clone();
        let piece = p.board[from].take();
        // En passant takes the pawn beside, not on, the square moved to.
        if let Some((color, Kind::Pawn)) = piece
            && Some(to) == self.en_passant
            && self.board[to].is_none()
            && from % 8 != to % 8
        {
            let captured = if color == Color::White {
                to - 8
            } else {
                to + 8
            };
            p.board[captured] = None;
        }
        if let Some((_, Kind::King)) = piece {
            // Castling moves the rook too.
            if to as i32 - from as i32 == 2 {
                p.board[from + 1] = p.board[from + 3].take();
            } else if from as i32 - to as i32 == 2 {
                p.board[from - 1] = p.board[from - 4].take();
            }
        }
        p.board[to] = match (piece, promote) {
            (Some((c, Kind::Pawn)), Some(k)) => Some((c, k)),
            (Some((c, Kind::Pawn)), None) if to / 8 == 0 || to / 8 == 7 => Some((c, Kind::Queen)),
            _ => piece,
        };
        p.en_passant = match piece {
            Some((_, Kind::Pawn)) if (to as i32 - from as i32).abs() == 16 => Some((from + to) / 2),
            _ => None,
        };
        for (sq, rights) in [
            (4, [0, 1]),
            (60, [2, 3]),
            (7, [0, 0]),
            (0, [1, 1]),
            (63, [2, 2]),
            (56, [3, 3]),
        ] {
            if from == sq || to == sq {
                for r in rights {
                    p.castling[r] = false;
                }
            }
        }
        if self.turn == Color::Black {
            p.fullmove += 1;
        }
        p.turn = other(self.turn);
        p
    }

    /// Play a move written in SAN (`Nf3`, `exd5`, `O-O`, `e8=Q+`).
    /// Returns the new position and the squares moved from and to.
    fn play_san(&self, san: &str) -> Result<(Self, usize, usize), String> {
        let m = san.trim_end_matches(['+', '#', '!', '?']);
        let color = self.turn;
        let home = if color == Color::White { 4 } else { 60 };
        if matches!(m, "O-O" | "0-0" | "O-O-O" | "0-0-0") {
            let long = m.len() == 5;
            let to = if long { home - 2 } else { home + 2 };
            let right = match (color, long) {
                (Color::White, false) => 0,
                (Color::White, true) => 1,
                (Color::Black, false) => 2,
                (Color::Black, true) => 3,
            };
            let path: Vec<usize> = if long {
                vec![home - 1, home - 2, home - 3]
            } else {
                vec![home + 1, home + 2]
            };
            let ok = self.castling[right]
                && self.board[home] == Some((color, Kind::King))
                && path.iter().all(|&s| self.board[s].is_none())
                && ![home, path[0], path[1]]
                    .iter()
                    .any(|&s| self.attacked(s, other(color)));
            return if ok {
                Ok((self.play(home, to, None), home, to))
            } else {
                Err(format!("{san} isn't possible here"))
            };
        }
        let (body, promote) = match m.split_once('=') {
            Some((b, p)) => (b, Some(p)),
            None => match m.chars().last() {
                Some(c @ ('Q' | 'R' | 'B' | 'N'))
                    if m.len() > 2 && m.as_bytes()[0].is_ascii_lowercase() =>
                {
                    (&m[..m.len() - 1], Some(&m[m.len() - c.len_utf8()..]))
                }
                _ => (m, None),
            },
        };
        let promote = match promote {
            Some(p) => Some(
                p.chars()
                    .next()
                    .and_then(kind_of)
                    .filter(|k| !matches!(k, Kind::King | Kind::Pawn))
                    .ok_or_else(|| format!("{san}: bad promotion"))?,
            ),
            None => None,
        };
        let first = body.chars().next().ok_or("an empty move")?;
        let (kind, rest) = if first.is_ascii_uppercase() {
            (
                kind_of(first).ok_or_else(|| format!("{san} isn't a move"))?,
                &body[1..],
            )
        } else {
            (Kind::Pawn, body)
        };
        let rest = rest.replace(['x', ':', '-'], "");
        if rest.len() < 2 {
            return Err(format!("{san} isn't a move"));
        }
        let to = square(&rest[rest.len() - 2..]).ok_or_else(|| format!("{san} isn't a move"))?;
        let hint = &rest[..rest.len() - 2];
        let file = hint
            .chars()
            .find(|c| ('a'..='h').contains(c))
            .map(|c| c as usize - 'a' as usize);
        let rank = hint
            .chars()
            .find(|c| ('1'..='8').contains(c))
            .map(|c| c as usize - '1' as usize);
        let candidates: Vec<usize> = (0..64)
            .filter(|&from| self.board[from] == Some((color, kind)))
            .filter(|&from| {
                file.is_none_or(|f| from % 8 == f) && rank.is_none_or(|r| from / 8 == r)
            })
            .filter(|&from| self.reaches(from, to, color, kind, false))
            .filter(|&from| {
                let next = self.play(from, to, promote);
                next.king(color)
                    .is_none_or(|k| !next.attacked(k, other(color)))
            })
            .collect();
        match candidates[..] {
            [from] => Ok((self.play(from, to, promote), from, to)),
            [] => Err(format!("{san} isn't possible here")),
            _ => Err(format!("{san} could be more than one piece")),
        }
    }
}

/// ```fen: a position.
pub fn fen(code: &str, sourcepos: &str) -> String {
    match Position::from_fen(code.trim()) {
        Ok(p) => {
            let to_move = match p.turn {
                Color::White => "White to move",
                Color::Black => "Black to move",
            };
            figure("chess", sourcepos, &board(&p, None, to_move))
        }
        Err(e) => invalid("FEN", "fen", code, sourcepos, &e),
    }
}

/// ```pgn: a game's final position.
pub fn pgn(code: &str, sourcepos: &str) -> String {
    match play_pgn(code) {
        Ok((p, last, caption)) => figure("chess", sourcepos, &board(&p, last, &caption)),
        Err(e) => invalid("PGN", "pgn", code, sourcepos, &e),
    }
}

type Played = (Position, Option<(usize, usize)>, String);

fn play_pgn(code: &str) -> Result<Played, String> {
    let mut tags = Vec::new();
    let mut moves = String::new();
    for line in code.lines() {
        let t = line.trim();
        if let Some(tag) = t.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
            if let Some((name, value)) = tag.split_once(' ') {
                tags.push((name.to_string(), value.trim().trim_matches('"').to_string()));
            }
        } else if !t.starts_with('%') {
            // `;` comments run to the end of the line.
            moves.push_str(t.split(';').next().unwrap_or(""));
            moves.push(' ');
        }
    }
    let tag = |n: &str| tags.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str());
    let mut pos = Position::from_fen(tag("FEN").unwrap_or(START))?;
    // Comments `{…}` and variations `(…)` are skipped.
    let mut clean = String::new();
    let (mut brace, mut paren) = (false, 0);
    for c in moves.chars() {
        match c {
            '{' => brace = true,
            '}' => brace = false,
            '(' if !brace => paren += 1,
            ')' if !brace => paren -= 1,
            _ if brace || paren > 0 => {}
            _ => clean.push(c),
        }
    }
    let mut last = None;
    let mut last_san = String::new();
    let mut result = tag("Result").filter(|r| *r != "*").map(String::from);
    for token in clean.split_whitespace() {
        if matches!(token, "1-0" | "0-1" | "1/2-1/2" | "½-½" | "*") {
            if token != "*" {
                result = Some(token.to_string());
            }
            break;
        }
        // Move numbers may be joined to the move: "1.e4", "12...Nf6".
        let token = token.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '…');
        if token.is_empty() || token.starts_with('$') {
            continue;
        }
        let number = format!(
            "{}{}",
            pos.fullmove,
            if pos.turn == Color::White { "." } else { "…" }
        );
        let (next, from, to) = pos
            .play_san(token)
            .map_err(|e| format!("move {number} {e}"))?;
        pos = next;
        last = Some((from, to));
        last_san = format!("{number} {token}");
    }
    let players = match (tag("White"), tag("Black")) {
        (Some(w), Some(b)) if w != "?" && b != "?" => format!("{w} – {b}"),
        _ => String::new(),
    };
    let mut caption: Vec<String> = Vec::new();
    if !players.is_empty() {
        caption.push(players);
    }
    if let Some(r) = result {
        caption.push(r);
    }
    if !last_san.is_empty() {
        caption.push(format!("after {last_san}"));
    }
    Ok((pos, last, caption.join(", ")))
}

const SQ: f64 = 40.0;
const EDGE: f64 = 16.0;

fn glyph(kind: Kind) -> char {
    // The filled ("black") shapes for both sides; white pieces are filled
    // white with a dark outline, so both read on any square.
    match kind {
        Kind::King => '♚',
        Kind::Queen => '♛',
        Kind::Rook => '♜',
        Kind::Bishop => '♝',
        Kind::Knight => '♞',
        Kind::Pawn => '♟',
    }
}

fn name(p: Piece) -> String {
    let c = match p.0 {
        Color::White => "white",
        Color::Black => "black",
    };
    let k = match p.1 {
        Kind::King => "king",
        Kind::Queen => "queen",
        Kind::Rook => "rook",
        Kind::Bishop => "bishop",
        Kind::Knight => "knight",
        Kind::Pawn => "pawn",
    };
    format!("{c} {k}")
}

fn board(p: &Position, last: Option<(usize, usize)>, caption: &str) -> String {
    let size = EDGE + 8.0 * SQ;
    let mut svg = String::new();
    let _ = write!(
        svg,
        "<svg viewBox=\"0 0 {size} {size}\" width=\"{size}\" role=\"img\" aria-label=\"Chess board{}{}\" \
         xmlns=\"http://www.w3.org/2000/svg\">",
        if caption.is_empty() { "" } else { ", " },
        crate::render::escape_attr(caption)
    );
    for (sq, square) in p.board.iter().enumerate() {
        let (file, rank) = (sq % 8, sq / 8);
        let x = EDGE + file as f64 * SQ;
        let y = (7 - rank) as f64 * SQ;
        let shade = if (file + rank) % 2 == 0 {
            "dark"
        } else {
            "light"
        };
        let moved = last.is_some_and(|(a, b)| a == sq || b == sq);
        let _ = write!(
            svg,
            "<rect class=\"mdv-chess-{shade}{}\" x=\"{x}\" y=\"{y}\" width=\"{SQ}\" height=\"{SQ}\"/>",
            if moved { " mdv-chess-moved" } else { "" }
        );
        if let Some(piece) = *square {
            let class = match piece.0 {
                Color::White => "mdv-chess-white",
                Color::Black => "mdv-chess-black",
            };
            // U+FE0E asks for the text form, not an emoji.
            let _ = write!(
                svg,
                "<text class=\"{class}\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\"><title>{}</title>{}\u{FE0E}</text>",
                x + SQ / 2.0,
                y + SQ * 0.8,
                name(piece),
                glyph(piece.1)
            );
        }
    }
    for i in 0..8 {
        let _ = write!(
            svg,
            "<text class=\"mdv-chess-coord\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>\
             <text class=\"mdv-chess-coord\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
            EDGE / 2.0,
            (7 - i) as f64 * SQ + SQ / 2.0 + 4.0,
            i + 1,
            EDGE + i as f64 * SQ + SQ / 2.0,
            8.0 * SQ + EDGE - 4.0,
            (b'a' + i as u8) as char
        );
    }
    svg.push_str("</svg>");
    if !caption.is_empty() {
        let _ = write!(svg, "<figcaption>{}</figcaption>", escape_text(caption));
    }
    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fen_positions() {
        let html = fen(
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1\n",
            "",
        );
        assert!(
            html.starts_with("<figure class=\"mdv-diagram mdv-chess\"><svg"),
            "{html}"
        );
        assert_eq!(html.matches("<title>").count(), 32);
        assert!(html.contains("Black to move"));
        assert!(fen("8/8/8", "").contains("8 ranks"));
        assert!(fen("9/8/8/8/8/8/8/8", "").contains("rank 8 has"));
    }

    #[test]
    fn games_are_played_out() {
        // Scholar's mate, with a comment, a variation, a NAG and castling
        // rights in play.
        let game = "[White \"A\"]\n[Black \"B\"]\n[Result \"1-0\"]\n\n\
                    1. e4 e5 2. Qh5 {threat} Nc6 (2... g6) 3. Bc4 Nf6?? $4 4. Qxf7# 1-0\n";
        let (p, last, caption) = play_pgn(game).unwrap();
        assert_eq!(caption, "A – B, 1-0, after 4. Qxf7#");
        assert_eq!(
            p.board[square("f7").unwrap()],
            Some((Color::White, Kind::Queen))
        );
        assert_eq!(last, Some((square("h5").unwrap(), square("f7").unwrap())));

        // Castling, en passant, promotion and disambiguation.
        let (p, _, _) = play_pgn("1. e4 d5 2. e5 f5 3. exf6 Nh6 4. fxg7 Nc6 5. gxh8=Q Bg4 6. Nf3 Qd7 7. Be2 O-O-O 8. O-O").unwrap();
        assert_eq!(
            p.board[square("h8").unwrap()],
            Some((Color::White, Kind::Queen))
        );
        assert_eq!(
            p.board[square("g1").unwrap()],
            Some((Color::White, Kind::King))
        );
        assert_eq!(
            p.board[square("f1").unwrap()],
            Some((Color::White, Kind::Rook))
        );
        assert_eq!(
            p.board[square("c8").unwrap()],
            Some((Color::Black, Kind::King))
        );
        assert_eq!(p.board[square("f5").unwrap()], None, "taken en passant");
        let (p, _, _) = play_pgn("[FEN \"4k3/8/8/8/8/8/4K3/R6R w - - 0 1\"]\n1. Rad1").unwrap();
        assert!(p.board[square("d1").unwrap()].is_some());
        assert!(
            play_pgn("[FEN \"4k3/8/8/8/8/8/4K3/R6R w - - 0 1\"]\n1. Rd1")
                .unwrap_err()
                .contains("more than one")
        );
        // A pinned piece can't move.
        assert!(
            play_pgn("[FEN \"4k3/4r3/8/8/8/8/4N3/4K3 w - - 0 1\"]\n1. Nc3")
                .unwrap_err()
                .contains("move 1. Nc3 isn't possible")
        );
        assert!(pgn("1. e5", "").contains("Can't draw this PGN: move 1. e5 isn't possible here"));
    }
}
