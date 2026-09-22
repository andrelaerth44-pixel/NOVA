    use super::*;
    #[test]
    fn parses_adjacent_nested_conditionals() {
        let source = r#"
            fn f(k) {
                if k == \"let\" {
                    value = 1
                    return value
                }
                if k == \"assign\" {
                    value = 2
                    return value
                }
            }
        "#;
        Parser::new(crate::lex(source).expect("lex")).program().expect("nested conditionals should parse");
    }
}