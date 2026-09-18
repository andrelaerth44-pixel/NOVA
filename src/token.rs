#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Num(f64), Str(String), Ident(String),
    Plus, Minus, Star, Slash, Percent,
    Eq, EqEq, Ne, Lt, Le, Gt, Ge,
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,
    Comma, Semi, Dot, Bang, And, Or,
    If, Else, While, Fn, Return, True, False, Let,
    Print, In, For, Import, Match, Eof,
}

