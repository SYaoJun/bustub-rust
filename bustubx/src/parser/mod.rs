use crate::error::BustubxResult;
use sqlparser::{ast::Statement, dialect::PostgreSqlDialect, parser::Parser};

pub fn parse_sql(sql: &str) -> BustubxResult<Vec<Statement>> {
    let stmts = Parser::parse_sql(&PostgreSqlDialect {}, sql)?;
    Ok(stmts)
}

#[cfg(test)]
mod tests {

    #[test]
    pub fn test_parser() {
        // 测试一下sqlparser-rs是否能正常解析SQL语句
        let sql = "select * from t1";
        let stmts = super::parse_sql(sql).unwrap();
        println!("{:#?}", stmts[0]);
    }
    #[test]
    pub fn test_multiple_sql() {
        let sql = "SELECT * FROM users; INSERT INTO users (id, name) VALUES (1, 'Alice');";
        let ast = super::parse_sql(sql);

        match ast {
            Ok(statements) => {
                println!("Number of statements: {}", statements.len());
                assert_eq!(2, statements.len());
                for statement in statements {
                    println!("{:?}", statement);
                }
            }
            Err(e) => println!("Error parsing SQL: {:?}", e),
        }
    }
}
