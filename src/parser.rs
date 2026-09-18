use crate::{Expr, Stmt, Token, Value, Pattern};

pub struct Parser { t: Vec<Token>, p: usize }
impl Parser {
    pub fn new(t:Vec<Token>)->Self{Self{t,p:0}}
    fn peek(&self)->&Token{&self.t[self.p]}
    fn take(&mut self)->Token{let x=self.t[self.p].clone();self.p+=1;x}
    fn eat(&mut self,x:&Token)->bool{if self.peek()==x{self.p+=1;true}else{false}}
    fn block(&mut self)->Result<Vec<Stmt>,String>{if !self.eat(&Token::LBrace){return Err("expected {".into())}let mut v=vec![];while *self.peek()!=Token::RBrace&&*self.peek()!=Token::Eof{v.push(self.stmt()?);self.eat(&Token::Semi);}if !self.eat(&Token::RBrace){return Err("expected }".into())}Ok(v)}
    pub fn program(&mut self)->Result<Vec<Stmt>,String>{let mut v=vec![];while *self.peek()!=Token::Eof{v.push(self.stmt()?);self.eat(&Token::Semi);}Ok(v)}
    fn type_name(&mut self)->Result<crate::types::Type,String>{
        match self.take(){
            Token::Ident(n)=>{
                let base=match n.as_str(){
                    "i32"=>crate::types::Type::I32,"i64"=>crate::types::Type::I64,
                    "f32"=>crate::types::Type::F32,"f64"=>crate::types::Type::F64,
                    "bool"=>crate::types::Type::Bool,"string"=>crate::types::Type::String,
                    "void"=>crate::types::Type::Void,"any"=>crate::types::Type::Any,
                    _=>crate::types::Type::Struct(n.clone())
                };
                let mut ty = base;
                if self.eat(&Token::Lt){
                    let mut args=Vec::new();
                    if !self.eat(&Token::Gt){
                        loop{
                            args.push(self.type_name()?);
                            if self.eat(&Token::Gt){break}
                            if !self.eat(&Token::Comma){return Err("expected , in generic type".into())}
                        }
                    }
                    let name=match ty { crate::types::Type::Struct(x)=>x, _=>ty.name() };
                    ty = crate::types::Type::Generic(name,args);
                }
                while self.eat(&Token::LBracket) {
                    if !self.eat(&Token::RBracket) { return Err("expected ] in array type".into()); }
                    ty = crate::types::Type::Array(Box::new(ty));
                }
                Ok(ty)
            },
            Token::LBracket=>{let t=self.type_name()?;if !self.eat(&Token::RBracket){return Err("expected ] in array type".into())}Ok(crate::types::Type::Array(Box::new(t)))},
            t=>Err(format!("expected type, got {:?}",t))
        }
    }
    fn mark_type_params(t: crate::types::Type, params: &[String]) -> crate::types::Type {
        match t {
            crate::types::Type::Struct(n) if params.iter().any(|p| p==&n) => crate::types::Type::TypeParam(n),
            crate::types::Type::Array(inner) => crate::types::Type::Array(Box::new(Self::mark_type_params(*inner, params))),
            crate::types::Type::Generic(n,args) => crate::types::Type::Generic(n,args.into_iter().map(|x|Self::mark_type_params(x,params)).collect()),
            other => other
        }
    }

    fn stmt(&mut self)->Result<Stmt,String>{
        if let Token::Ident(word) = self.peek() {
            if word == "struct" {
                // struct handled below

                self.take();
                let name = match self.take() { Token::Ident(x)=>x, _=>return Err("expected struct name".into()) };
                if !self.eat(&Token::LBrace) { return Err("expected { after struct name".into()); }
                let mut fields = Vec::new();
                while *self.peek()!=Token::RBrace && *self.peek()!=Token::Eof {
                    let field = match self.take() { Token::Ident(x)=>x, _=>return Err("expected struct field name".into()) };
                    if !self.eat(&Token::Colon) { return Err("expected : after struct field".into()); }
                    let ty = self.type_name()?;
                    fields.push((field, ty));
                    self.eat(&Token::Comma);
                    self.eat(&Token::Semi);
                }
                if !self.eat(&Token::RBrace) { return Err("expected } after struct".into()); }
                return Ok(Stmt::StructDecl(name, fields));
            }
            if word == "enum" {
                self.take();
                let name = match self.take() { Token::Ident(x)=>x, _=>return Err("expected enum name".into()) };
                if !self.eat(&Token::LBrace) { return Err("expected { after enum name".into()); }
                let mut variants = Vec::new();
                while *self.peek()!=Token::RBrace && *self.peek()!=Token::Eof {
                    let variant = match self.take() { Token::Ident(x)=>x, _=>return Err("expected enum variant name".into()) };
                    let payload = if self.eat(&Token::LParen) { let t=self.type_name()?; if !self.eat(&Token::RParen){return Err("expected ) after enum payload".into())} Some(t) } else { None };
                    variants.push((variant,payload)); self.eat(&Token::Comma); self.eat(&Token::Semi);
                }
                if !self.eat(&Token::RBrace) { return Err("expected } after enum".into()); }
                return Ok(Stmt::EnumDecl(name, variants));
            }
        }
        match self.peek() {
            Token::Let=>{self.take();let n=match self.take(){Token::Ident(x)=>x,_=>return Err("expected identifier".into())};let ty=if self.eat(&Token::Colon){Some(self.type_name()?)}else{None};if !self.eat(&Token::Eq){return Err("expected =".into())}Ok(Stmt::Let(n,ty,self.expr()?))},
            Token::Print=>{self.take();Ok(Stmt::Print(self.expr()?))},
            Token::Return=>{self.take();Ok(Stmt::Return(self.expr()?))},
            Token::If=>{self.take();let c=self.expr()?;let a=self.block()?;let b=if self.eat(&Token::Else){self.block()?}else{vec![]};Ok(Stmt::If(c,a,b))},
            Token::While=>{self.take();let c=self.expr()?;Ok(Stmt::While(c,self.block()?))},
            Token::For=>{self.take();let n=match self.take(){Token::Ident(x)=>x,_=>return Err("expected loop variable".into())};if !self.eat(&Token::In){return Err("expected in".into())}let it=self.expr()?;Ok(Stmt::For(n,it,self.block()?))},
            Token::Import=>{self.take();match self.take(){Token::Str(x)=>Ok(Stmt::Import(x)),_=>Err("import expects a string path".into())}},
            Token::Match=>{self.take();let value=self.expr()?;if !self.eat(&Token::LBrace){return Err("expected { after match".into())}let mut arms=vec![];let mut otherwise=vec![];while *self.peek()!=Token::RBrace&&*self.peek()!=Token::Eof{if *self.peek()==Token::Else{self.take();otherwise=self.block()?;self.eat(&Token::Comma);continue}let pat_expr=self.expr()?;let pat=Self::pattern_from_expr(pat_expr)?;if !self.eat(&Token::LBrace){return Err("expected { in match arm".into())}let mut body=vec![];while *self.peek()!=Token::RBrace&&*self.peek()!=Token::Eof{body.push(self.stmt()?);self.eat(&Token::Semi);}if !self.eat(&Token::RBrace){return Err("expected } in match arm".into())}arms.push((pat,body));self.eat(&Token::Comma);}if !self.eat(&Token::RBrace){return Err("expected } after match".into())}Ok(Stmt::Match(value,arms,otherwise))},
            Token::Fn=>{
                self.take();
                let n=match self.take(){Token::Ident(x)=>x,_=>return Err("expected function name".into())};
                let mut generics=Vec::new();
                if self.eat(&Token::Lt){
                    if !self.eat(&Token::Gt){loop{
                        generics.push(match self.take(){Token::Ident(x)=>x,_=>return Err("expected generic type parameter".into())});
                        if self.eat(&Token::Gt){break}
                        if !self.eat(&Token::Comma){return Err("expected , in generic parameter list".into())}
                    }}
                }
                if !self.eat(&Token::LParen){return Err("expected (".into())}
                let mut a=vec![];
                if !self.eat(&Token::RParen){loop{
                    let pn=match self.take(){Token::Ident(x)=>x,_=>return Err("expected parameter".into())};
                    let pt=if self.eat(&Token::Colon){self.type_name()?}else{crate::types::Type::Any};
                    a.push((pn,pt));
                    if self.eat(&Token::RParen){break}
                    if !self.eat(&Token::Comma){return Err("expected ,".into())}
                }}
                let ret=if self.eat(&Token::Arrow){self.type_name()?}else{crate::types::Type::Any};
                let a=a.into_iter().map(|(name,t)|(name,Self::mark_type_params(t,&generics))).collect();
                let ret=Self::mark_type_params(ret,&generics);
                Ok(Stmt::Fn(n,generics,a,ret,self.block()?))
            },
            Token::Ident(n)=>{
                let name=n.clone();
                if self.p+1<self.t.len() && self.t[self.p+1]==Token::Colon {
                    self.take();
                    self.take();
                    let ty=self.type_name()?;
                    if !self.eat(&Token::Eq){return Err("expected =".into())}
                    return Ok(Stmt::Let(name,Some(ty),self.expr()?));
                }
                if self.p+1<self.t.len() && self.t[self.p+1]==Token::Eq {
                    self.take();
                    self.take();
                    return Ok(Stmt::Assign(name,self.expr()?));
                }
                Ok(Stmt::Expr(self.expr()?))
            },
            _=>Ok(Stmt::Expr(self.expr()?))
        }
    }
    fn pattern_from_expr(e: Expr)->Result<Pattern,String>{
        match e {
            Expr::Var(n) if n=="_" => Ok(Pattern::Wildcard),
            Expr::Var(n) => Ok(Pattern::Enum{variant:n,binding:None}),
            Expr::Call(n,args) if args.len() == 1 => {
                let binding=match &args[0]{
                    Expr::Var(x) if x!="_"=>Some(x.clone()),
                    Expr::Var(x) if x=="_"=>None,
                    _=>return Err("enum pattern payload must be a binding or _".into())
                };
                Ok(Pattern::Enum{variant:n,binding})
            }
            Expr::Call(n,args) if args.is_empty() => Ok(Pattern::Enum{variant:n,binding:None}),
            Expr::Field(_,variant) => Ok(Pattern::Enum{variant,binding:None}),
            x => Ok(Pattern::Literal(x)),
        }
    }
    fn expr(&mut self)->Result<Expr,String>{self.or()}
    fn or(&mut self)->Result<Expr,String>{let mut x=self.and()?;while self.eat(&Token::Or){x=Expr::Binary(Box::new(x),Token::Or,Box::new(self.and()?));}Ok(x)}
    fn and(&mut self)->Result<Expr,String>{let mut x=self.eq()?;while self.eat(&Token::And){x=Expr::Binary(Box::new(x),Token::And,Box::new(self.eq()?));}Ok(x)}
    fn eq(&mut self)->Result<Expr,String>{let mut x=self.cmp()?;loop{let op=match self.peek(){Token::EqEq=>Token::EqEq,Token::Ne=>Token::Ne,_=>break};self.take();x=Expr::Binary(Box::new(x),op,Box::new(self.cmp()?));}Ok(x)}
    fn cmp(&mut self)->Result<Expr,String>{let mut x=self.term()?;loop{let op=match self.peek(){Token::Lt=>Token::Lt,Token::Le=>Token::Le,Token::Gt=>Token::Gt,Token::Ge=>Token::Ge,_=>break};self.take();x=Expr::Binary(Box::new(x),op,Box::new(self.term()?));}Ok(x)}
    fn term(&mut self)->Result<Expr,String>{let mut x=self.factor()?;loop{let op=match self.peek(){Token::Plus=>Token::Plus,Token::Minus=>Token::Minus,_=>break};self.take();x=Expr::Binary(Box::new(x),op,Box::new(self.factor()?));}Ok(x)}
    fn factor(&mut self)->Result<Expr,String>{let mut x=self.unary()?;loop{let op=match self.peek(){Token::Star=>Token::Star,Token::Slash=>Token::Slash,Token::Percent=>Token::Percent,_=>break};self.take();x=Expr::Binary(Box::new(x),op,Box::new(self.unary()?));}Ok(x)}
    fn unary(&mut self)->Result<Expr,String>{if self.eat(&Token::Minus){Ok(Expr::Unary(Token::Minus,Box::new(self.unary()?)))}else if self.eat(&Token::Bang){Ok(Expr::Unary(Token::Bang,Box::new(self.unary()?)))}else{self.primary()}}
    fn primary(&mut self)->Result<Expr,String>{
        let x=match self.take(){Token::Num(x)=>Expr::Val(Value::Num(x)),Token::Str(x)=>Expr::Val(Value::Str(x)),Token::True=>Expr::Val(Value::Bool(true)),Token::False=>Expr::Val(Value::Bool(false)),
            Token::Ident(n)=>{
                if self.eat(&Token::LParen){
                    let mut a=vec![];if !self.eat(&Token::RParen){loop{a.push(self.expr()?);if self.eat(&Token::RParen){break}if !self.eat(&Token::Comma){return Err("expected ,".into())}}}
                    Expr::Call(n,a)
                } else if n == "map" && self.eat(&Token::LBrace) {
                    let mut entries=vec![];
                    if !self.eat(&Token::RBrace){loop{
                        let key=self.expr()?;
                        if !self.eat(&Token::Colon){return Err("expected : in map literal".into())}
                        let value=self.expr()?;
                        entries.push((key,value));
                        if self.eat(&Token::RBrace){break}
                        if !self.eat(&Token::Comma){return Err("expected , in map literal".into())}
                    }}
                    Expr::Map(entries)
                } else if n == "set" && self.eat(&Token::LBrace) {
                    let mut values=vec![];
                    if !self.eat(&Token::RBrace){loop{
                        values.push(self.expr()?);
                        if self.eat(&Token::RBrace){break}
                        if !self.eat(&Token::Comma){return Err("expected , in set literal".into())}
                    }}
                    Expr::Set(values)
                } else if self.peek() == &Token::LBrace
                    && matches!(self.t.get(self.p + 1), Some(Token::Ident(_)))
                    && self.t.get(self.p + 2) == Some(&Token::Colon) {
                    self.take();
                    let mut fields=vec![];
                    if !self.eat(&Token::RBrace){loop{
                        let field=match self.take(){Token::Ident(x)=>x,_=>return Err("expected field name".into())};
                        if !self.eat(&Token::Colon){return Err("expected : in struct literal".into())}
                        fields.push((field,self.expr()?));
                        if self.eat(&Token::RBrace){break}
                        if !self.eat(&Token::Comma){return Err("expected , in struct literal".into())}
                    }}
                    Expr::StructInit(n,fields)
                } else {Expr::Var(n)}
            },
            Token::LBracket=>{let mut a=vec![];if !self.eat(&Token::RBracket){loop{a.push(self.expr()?);if self.eat(&Token::RBracket){break}if !self.eat(&Token::Comma){return Err("expected ,".into())}}}Expr::Array(a)},
            Token::LParen=>{let x=self.expr()?;if !self.eat(&Token::RParen){return Err("expected )".into())}x},
            Token::Fn=>{
                if !self.eat(&Token::LParen){return Err("expected ( after fn in closure".into())}
                let mut args=vec![];
                if !self.eat(&Token::RParen){loop{
                    let n=match self.take(){Token::Ident(x)=>x,_=>return Err("expected closure parameter".into())};
                    if self.eat(&Token::Colon){let _=self.type_name()?;}
                    args.push(n);
                    if self.eat(&Token::RParen){break}
                    if !self.eat(&Token::Comma){return Err("expected , in closure parameters".into())}
                }}
                let _ret=if self.eat(&Token::Arrow){Some(self.type_name()?)}else{None};
                Expr::Closure(args,self.block()?)
            },
            t=>return Err(format!("unexpected token {:?}",t))};
        let mut x = x;
        loop {
            if self.eat(&Token::Dot) {
                let field = match self.take() { Token::Ident(n)=>n, _=>return Err("expected field name after .".into()) };
                x = Expr::Field(Box::new(x), field);
            } else if self.eat(&Token::LBracket) {
                let index = self.expr()?;
                if !self.eat(&Token::RBracket) { return Err("expected ] after index".into()); }
                x = Expr::Index(Box::new(x), Box::new(index));
            } else if self.eat(&Token::LParen) {
                let mut args=vec![];
                if !self.eat(&Token::RParen) { loop { args.push(self.expr()?); if self.eat(&Token::RParen){break} if !self.eat(&Token::Comma){return Err("expected , in call".into())} } }
                x = match x { Expr::Var(n)=>Expr::Call(n,args), other=>Expr::CallValue(Box::new(other),args) };
            } else if self.eat(&Token::Question) {
                x = Expr::Try(Box::new(x));
            } else { break; }
        }
        Ok(x)
    }
}

