//! Helper for preparing SQL statements.

use crate::*;
pub use std::fmt::Write;

pub trait SqlWriter: Write {
    fn push_param(&mut self, value: Value, query_builder: &dyn QueryBuilder);
    fn result(self) -> String;
}

pub struct SqlStringWriter {
    pub(crate) string: String,
}

impl SqlStringWriter {
    pub fn new() -> Self {
        Self {
            string: String::with_capacity(256),
        }
    }
}

impl Write for SqlStringWriter {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        write!(self.string, "{}", s)
    }
}

impl SqlWriter for SqlStringWriter {
    fn push_param(&mut self, value: Value, query_builder: &dyn QueryBuilder) {
        self.string.push_str(&query_builder.value_to_string(&value));
    }

    fn result(self) -> String {
        self.string
    }
}

pub struct SqlWriterObj {
    counter: usize,
    placeholder: String,
    numbered: bool,
    string: String,
    values: Vec<Value>,
}

impl SqlWriterObj {
    pub fn new(placeholder: &str, numbered: bool) -> Self {
        Self {
            counter: 0,
            placeholder: placeholder.to_owned(),
            numbered,
            string: String::with_capacity(256),
            values: Vec::new(),
        }
    }

    pub fn into_parts(self) -> (String, Values) {
        (self.string, Values(self.values))
    }
}

impl Write for SqlWriterObj {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        write!(self.string, "{}", s)
    }
}

impl SqlWriter for SqlWriterObj {
    fn push_param(&mut self, value: Value, query_builder: &dyn QueryBuilder) {}

    fn result(self) -> String {
        self.string
    }
}

pub fn inject_parameters<I>(sql: &str, params: I, query_builder: &dyn QueryBuilder) -> String
where
    I: IntoIterator<Item = Value>,
{
    let params: Vec<Value> = params.into_iter().collect();
    let tokenizer = Tokenizer::new(sql);
    let tokens: Vec<Token> = tokenizer.iter().collect();
    let mut counter = 0;
    let mut output = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        match token {
            Token::Punctuation(mark) => {
                if (mark.as_ref(), false) == query_builder.placeholder() {
                    output.push(query_builder.value_to_string(&params[counter]));
                    counter += 1;
                    i += 1;
                    continue;
                } else if (mark.as_ref(), true) == query_builder.placeholder()
                    && i + 1 < tokens.len()
                {
                    if let Token::Unquoted(next) = &tokens[i + 1] {
                        if let Ok(num) = next.parse::<usize>() {
                            output.push(query_builder.value_to_string(&params[num - 1]));
                            i += 2;
                            continue;
                        }
                    }
                }
                output.push(mark.to_string())
            }
            _ => output.push(token.to_string()),
        }
        i += 1;
    }
    output.into_iter().collect()
}

#[cfg(test)]
#[cfg(feature = "backend-mysql")]
mod tests {
    use super::*;

    #[test]
    fn inject_parameters_1() {
        assert_eq!(
            inject_parameters("WHERE A = ?", vec!["B".into()], &MysqlQueryBuilder),
            "WHERE A = 'B'"
        );
    }

    #[test]
    fn inject_parameters_2() {
        assert_eq!(
            inject_parameters(
                "WHERE A = '?' AND B = ?",
                vec!["C".into()],
                &MysqlQueryBuilder
            ),
            "WHERE A = '?' AND B = 'C'"
        );
    }

    #[test]
    fn inject_parameters_3() {
        assert_eq!(
            inject_parameters(
                "WHERE A = ? AND C = ?",
                vec!["B".into(), "D".into()],
                &MysqlQueryBuilder
            ),
            "WHERE A = 'B' AND C = 'D'"
        );
    }

    #[test]
    fn inject_parameters_4() {
        assert_eq!(
            inject_parameters(
                "WHERE A = $1 AND C = $2",
                vec!["B".into(), "D".into()],
                &PostgresQueryBuilder
            ),
            "WHERE A = 'B' AND C = 'D'"
        );
    }

    #[test]
    fn inject_parameters_5() {
        assert_eq!(
            inject_parameters(
                "WHERE A = $2 AND C = $1",
                vec!["B".into(), "D".into()],
                &PostgresQueryBuilder
            ),
            "WHERE A = 'D' AND C = 'B'"
        );
    }

    #[test]
    fn inject_parameters_6() {
        assert_eq!(
            inject_parameters("WHERE A = $1", vec!["B'C".into()], &PostgresQueryBuilder),
            "WHERE A = E'B\\'C'"
        );
    }

    #[test]
    fn inject_parameters_7() {
        assert_eq!(
            inject_parameters(
                "?",
                vec![vec![0xABu8, 0xCD, 0xEF].into()],
                &MysqlQueryBuilder
            ),
            "x'ABCDEF'"
        );
    }
}
