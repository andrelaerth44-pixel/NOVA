use crate::Token;

pub fn lex(src: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            ' ' | '\t' | '\r' | '\n' => i += 1,
            '#' => { while i < c.len() && c[i] != '\n' { i += 1; } }
            '0'..='9' => {
                let s = i; while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') { i += 1; }
                out.push(Token::Num(src[s..i].parse().map_err(|_| "invalid number")?));
            }
            '"' => {
                i += 1; let mut s = String::new();
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' && i + 1 < c.len() {
                        i += 1; s.push(match c[i] { 'n'=>'\n','r'=>'\r','t'=>'\t','"'=>'"','\\'=>'\\',x=>x });
                    } else { s.push(c[i]); }
                    i += 1;
                }
                if i == c.len() { return Err("unterminated string".into()); }
                i += 1; out.push(Token::Str(s));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let s = i; while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i]=='_') { i += 1; }
                let w = &src[s..i];
                out.push(match w {
                    "if"=>Token::If, "else"=>Token::Else, "while"=>Token::While,
                    "fn"=>Token::Fn, "return"=>Token::Return, "true"=>Token::True,
                    "false"=>Token::False, "let"=>Token::Let, "print"=>Token::Print, "in"=>Token::In, "for"=>Token::For, "import"=>Token::Import, "match"=>Token::Match,
                    "and"=>Token::And, "or"=>Token::Or, _=>Token::Ident(w.into())
                });
            }
            '+' => { out.push(Token::Plus); i+=1; }
            '-' => { out.push(Token::Minus); i+=1; }
            '*' => { out.push(Token::Star); i+=1; }
            '/' => { out.push(Token::Slash); i+=1; }
            '%' => { out.push(Token::Percent); i+=1; }
            '(' => { out.push(Token::LParen); i+=1; }
            ')' => { out.push(Token::RParen); i+=1; }
            '{' => { out.push(Token::LBrace); i+=1; }
            '}' => { out.push(Token::RBrace); i+=1; }
            '[' => { out.push(Token::LBracket); i+=1; }
            ']' => { out.push(Token::RBracket); i+=1; }
            ',' => { out.push(Token::Comma); i+=1; }
            ';' => { out.push(Token::Semi); i+=1; }
            ':' => { out.push(Token::Colon); i+=1; }
            '-' if i+1<c.len() && c[i+1]=='>' => { out.push(Token::Arrow); i+=2; }
            '.' => { out.push(Token::Dot); i+=1; }
            '!' => { if i+1<c.len() && c[i+1]=='=' {out.push(Token::Ne);i+=2} else {out.push(Token::Bang);i+=1} }
            '=' => { if i+1<c.len() && c[i+1]=='=' {out.push(Token::EqEq);i+=2} else {out.push(Token::Eq);i+=1} }
            '<' => { if i+1<c.len() && c[i+1]=='=' {out.push(Token::Le);i+=2} else {out.push(Token::Lt);i+=1} }
            '>' => { if i+1<c.len() && c[i+1]=='=' {out.push(Token::Ge);i+=2} else {out.push(Token::Gt);i+=1} }
            _ => return Err(format!("unexpected character {}", c[i])),
        }
    }
    out.push(Token::Eof); Ok(out)
}

