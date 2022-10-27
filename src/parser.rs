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
    pub pipelines: Vec<Pipeline>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RunIf {
    Always,
    /// Run the command if the previous command returned 0.
    Success,
    /// Run the command if the previous command returned non-zero value.
    Failure,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Pipeline {
    pub run_if: RunIf,
    pub commands: Vec<Command>, // Separated by `|'.
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Command {
    SimpleCommand { argv: Vec<Word> },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LiteralChar {
    Normal(char),
    Escaped(char),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Word(pub Vec<Span>);

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Span {
    Literal(String),
    // Internally used by the parser.
    LiteralChars(Vec<LiteralChar>),
}

pub fn parse(script: &str) -> Result<Ast, ParseError> {
    match ShellParser::parse(Rule::script, script) {
        Ok(mut pairs) => {
            let terms = visit_compound_list(pairs.next().unwrap());
            debug!(?terms);

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
            let pipelines = visit_and_or_list(and_or_list, RunIf::Always);
            terms.push(Term { code, pipelines });
        }

        if let Some(rest) = rest {
            terms.extend(visit_compound_list(rest));
        }
    }

    terms
}

fn visit_and_or_list(pair: Pair<Rule>, run_if: RunIf) -> Vec<Pipeline> {
    let mut terms = Vec::new();
    let mut inner = pair.into_inner();
    if let Some(pipeline) = inner.next() {
        let commands = visit_pipeline(pipeline);
        terms.push(Pipeline { commands, run_if });

        let next_run_if = inner
            .next()
            .map(|sep| match sep.as_span().as_str() {
                "||" => RunIf::Failure,
                "&&" => RunIf::Success,
                _ => RunIf::Always,
            })
            .unwrap_or(RunIf::Always);

        if let Some(rest) = inner.next() {
            terms.extend(visit_and_or_list(rest, next_run_if));
        }
    }

    terms
}

fn visit_pipeline(pair: Pair<Rule>) -> Vec<Command> {
    let mut commands = Vec::new();
    let inner = pair.into_inner();
    for command in inner {
        commands.push(visit_command(command));
    }

    commands
}

fn visit_command(pair: Pair<Rule>) -> Command {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::simple_command => visit_simple_command(inner),
        // TODO: support other rules
        _ => unimplemented!("rule {:?}", inner.as_rule()),
    }
}

fn visit_simple_command(pair: Pair<Rule>) -> Command {
    assert_eq!(pair.as_rule(), Rule::simple_command);

    let mut argv = Vec::new();

    let mut inner = pair.into_inner();
    debug!(?inner);
    let _assignments_pairs = inner.next().unwrap().into_inner();
    let argv0 = inner.next().unwrap().into_inner().next().unwrap();
    let args = inner.next().unwrap().into_inner();

    argv.push(visit_word(argv0));
    for word_or_redirect in args {
        match word_or_redirect.as_rule() {
            Rule::word => argv.push(visit_word(word_or_redirect)),
            Rule::redirect => (),
            _ => unreachable!(),
        }
    }

    Command::SimpleCommand { argv }
}

fn visit_word(pair: Pair<Rule>) -> Word {
    visit_escaped_word(pair, false)
}

fn visit_escape_sequences(pair: Pair<Rule>, escaped_chars: Option<&str>) -> String {
    let mut s = String::new();
    let mut escaped = false;
    for ch in pair.as_str().chars() {
        if escaped {
            escaped = false;
            if let Some(escaped_chars) = escaped_chars {
                if !escaped_chars.contains(ch) {
                    s.push('\\');
                }
            }
            s.push(ch);
        } else if ch == '\\' {
            escaped = true;
        } else {
            s.push(ch);
        }
    }

    s
}

fn visit_escaped_word(pair: Pair<Rule>, literal_chars: bool) -> Word {
    assert_eq!(pair.as_rule(), Rule::word);

    let mut spans = Vec::new();
    for span in pair.into_inner() {
        match span.as_rule() {
            Rule::literal_span if literal_chars => {
                let mut chars = Vec::new();
                for ch in span.into_inner() {
                    match ch.as_rule() {
                        Rule::escaped_char => {
                            let lit_ch = ch.as_str().chars().nth(1).unwrap();
                            chars.push(LiteralChar::Escaped(lit_ch))
                        }
                        Rule::unescaped_char => {
                            let lit_ch = ch.as_str().chars().next().unwrap();
                            chars.push(LiteralChar::Normal(lit_ch))
                        }
                        _ => unreachable!(),
                    }
                }
                spans.push(Span::LiteralChars(chars));
            }
            Rule::literal_span if !literal_chars => {
                spans.push(Span::Literal(visit_escape_sequences(span, None)));
            }
            _ => {
                debug!(?span);
                unimplemented!("span {:?}", span.as_rule());
            }
        }
    }

    debug!("spans: {:?}", spans);
    Word(spans)
}

#[cfg(test)]
mod test {
    use super::{parse, Ast, Command, Pipeline, RunIf, Span, Term, Word};

    macro_rules! literal_word_vec {
        ($($x:expr), *) => {
            vec![$( Word(vec![Span::Literal($x.to_string())]), )*]
        };
    }

    #[test]
    pub fn test_simple_commands() {
        assert_eq!(
            parse("ls -G /tmp\n"),
            Ok(Ast {
                terms: vec![Term {
                    code: "ls -G /tmp".into(),
                    pipelines: vec![Pipeline {
                        run_if: RunIf::Always,
                        commands: vec![Command::SimpleCommand {
                            argv: literal_word_vec!["ls", "-G", "/tmp"]
                        }],
                    }],
                }],
            })
        );
    }
}
