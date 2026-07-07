#[test]
fn probe() {
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;
    let bad = "CREATE TABLE t ( a BIGINT NOT NULL, created_bigint NOT NULL );";
    match Parser::parse_sql(&PostgreSqlDialect {}, bad) {
        Ok(stmts) => println!("PROBE-OK len={} :: {:?}", stmts.len(), stmts),
        Err(e) => println!("PROBE-ERR: {:?}", e),
    }
}
