use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use tracing::debug;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParseError {
    Fatal(String),
    Empty,
}

#[derive(Parser)]
#[grammar = "shell.pest"]
struct ShellParser;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Ast {
    pub terms: Vec<Term>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Term {
    pub code: String,
}

pub fn parse(script: &str) -> Result<Ast, ParseError> {
    match ShellParser::parse(Rule::script, script) {
        Ok(mut pairs) => {
            debug!(?pairs);
            let terms = visit_compound_list(pairs.next().unwrap());

            if terms.is_empty() {
                Err(ParseError::Empty)
            } else {
                Ok(Ast { terms })
            }
        }
        Err(err) => Err(ParseError::Fatal(err.to_string())),
    }
}

fn visit_compound_list(pair: Pair<Rule>) -> Vec<Term> {
    let mut terms = Vec::new();
    let mut inner = pair.into_inner();
    if let Some(and_or_list) = inner.next() {
        let mut rest = None;
        while let Some(sep_or_rest) = inner.next() {
            debug!(?sep_or_rest);
            match sep_or_rest.as_rule() {
                Rule::compound_list => {
                    rest = Some(sep_or_rest);
                    break;
                }
                _ => (),
            }
        }

        if and_or_list.as_rule() == Rule::and_or_list {
            let code = and_or_list.as_str().to_owned().trim().to_owned();
            terms.push(Term { code });
        }

        if let Some(rest) = rest {
            terms.extend(visit_compound_list(rest));
        }
    }

    terms
}
