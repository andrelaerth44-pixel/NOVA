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
                let target=self.expr()?;
                if self.eat(&Token::Eq) {
                    let value=self.expr()?;
                    return match target {
                        Expr::Var(name) => Ok(Stmt::Assign(name,value)),
                        Expr::Field(_, _) | Expr::Index(_, _) => Ok(Stmt::AssignTarget(target,value)),
                        _ => Err("assignment target must be a variable, field, or index".into()),
                    };
                }
                Ok(Stmt::Expr(target))
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