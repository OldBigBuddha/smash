use crate::parser::Span;
use crate::parser::Word;
use crate::shell::Shell;

use tracing::debug;

pub fn expand_words(shell: &mut Shell, words: &[Word]) -> anyhow::Result<Vec<String>> {
    debug!("expand_words: {:?}", words);
    let mut evaluated = Vec::new();
    for word in words {
        let mut ws = Vec::new();
        for w in expand_word_into_vec(shell, word, &shell.ifs())? {
            debug!("w: {:?}", w);
            ws.push(w);
        }

        evaluated.extend(ws);
    }

    debug!("expand_words: {:?}", evaluated);
    Ok(evaluated)
}

pub fn expand_word_into_vec(
    _shell: &mut Shell,
    word: &Word,
    ifs: &str,
) -> anyhow::Result<Vec<String>> {
    let mut words = Vec::new();
    let mut current_word = Vec::new();
    for span in word.spans() {
        let (frags, expand) = match span {
            Span::LiteralChars(..) => {
                unreachable!()
            }
            Span::Literal(s) => (vec![s.clone()], false),
        };

        let frags_len = frags.len();
        for frag in frags {
            if expand {
                if !current_word.is_empty() {
                    words.push(current_word.into_iter().collect::<String>());
                    current_word = Vec::new();
                }

                for word in frag.split(|c| ifs.contains(c)) {
                    words.push(word.to_string());
                }
            } else {
                current_word.push(frag);
            }

            if frags_len > 1 && !current_word.is_empty() {
                words.push(current_word.into_iter().collect::<String>());
                current_word = Vec::new();
            }
        }
    }

    if !current_word.is_empty() {
        words.push(current_word.into_iter().collect::<String>());
    }

    if words.is_empty() {
        Ok(vec![String::new()])
    } else {
        Ok(words)
    }
}
