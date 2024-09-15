use nom_prelude::{complete::*, nom::AsChar, *};

use super::{Entry, Line, Msg};

pub(crate) fn tokenize_msg<I: StringLikeInput>(
    input: I,
    exhaustive: bool,
) -> Result<Msg<I>, String> {
    let (rest, res) = input.err_to_string(msg(input))?;
    if !exhaustive || rest.input_len() == 0 {
        Ok(res)
    } else {
        let tail: String = rest
            .iter_elements()
            .take(20)
            .map(|ch| ch.as_char())
            .collect();
        Err(format!("Failed to exhaust input to the end: {tail}",))
    }
}

fn msg<I: StringLikeInput, E: ParseError<I>>(i: I) -> IResult<I, Msg<I>, E> {
    map(separated_list_first_unchecked(t_rn, line), |lines| Msg {
        lines,
    })(i)
}

fn line<I: StringLikeInput, E: ParseError<I>>(i: I) -> IResult<I, Line<I>, E> {
    alt((
        map(comment, Line::Comment),
        //map(char('#'), |_| Line::Comment("")),
        map(preceded(space0, entry_with_apply), Line::Entry),
        map(space0, |_| Line::Break),
    ))(i)
}

fn comment<I: StringLikeInput, E: ParseError<I>>(i: I) -> IResult<I, I, E> {
    let alt = alt((recognize(char('#')), recognize(tag("//"))));
    preceded(pair(space0, alt), optional_text)(i)
}

fn _entry_with_tuple<I: StringLikeInput, E: ParseError<I>>(i: I) -> IResult<I, Entry<I>, E> {
    map(
        tuple((
            curly_delimited(unsigned_number),
            curly_delimited(not_closing_curly),
            curly_delimited(not_closing_curly),
            opt(comment),
        )),
        |(index, secondary, value, comment)| Entry {
            index,
            secondary,
            value,
            comment,
        },
    )(i)
}

fn _entry_with_macro<I: StringLikeInput, E: ParseError<I>>(i: I) -> IResult<I, Entry<I>, E> {
    Ok(parse_struct!(
        i,
        Entry {
            index: curly_delimited(unsigned_number),
            secondary: curly_delimited(not_closing_curly),
            value: curly_delimited(not_closing_curly),
            comment: opt(comment),
        }
    ))
}

fn entry_with_apply<I: StringLikeInput, E: ParseError<I>>(mut i: I) -> IResult<I, Entry<I>, E> {
    let entry = Entry {
        index: apply(&mut i, curly_delimited(cut(unsigned_number)))?,
        secondary: cut_apply(&mut i, curly_delimited(not_closing_curly))?,
        value: cut_apply(&mut i, curly_delimited(not_closing_curly))?,
        comment: cut_apply(&mut i, opt(comment))?,
    };
    Ok((i, entry))
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use super::*;

    fn lex<I: Debug + StringLikeInput, T, F>(fun: F, input: I) -> T
    where
        F: FnOnce(I) -> IResult<I, T, nom::error::VerboseError<I>>,
    {
        fun(input).unwrap().1
    }

    fn with_all_entry_impls<I: Debug + StringLikeInput>(sample: I, correct: &Entry<I>) {
        assert_eq!(&lex(_entry_with_tuple, sample), correct);
        assert_eq!(&lex(_entry_with_macro, sample), correct);
        assert_eq!(&lex(entry_with_apply, sample), correct);
    }

    #[test]
    fn test_all_entry_impls() {
        const SAMPLE: &str = "{1}{foo}{bar}";
        const CORRECT: Entry<&str> = Entry {
            index: 1,
            secondary: "foo",
            value: "bar",
            comment: None,
        };
        with_all_entry_impls(SAMPLE, &CORRECT);
    }
    #[test]
    fn test_all_entry_impls_bytes() {
        const SAMPLE: &[u8] = b"{1}{foo}{bar}";
        const CORRECT: Entry<&[u8]> = Entry {
            index: 1,
            secondary: b"foo",
            value: b"bar",
            comment: None,
        };
        with_all_entry_impls(SAMPLE, &CORRECT);
    }

    fn new_entry<I: StringLikeInput>(index: u32, secondary: I, value: I) -> Entry<I> {
        Entry {
            index,
            secondary,
            value,
            comment: None,
        }
    }
    fn entry_line<I: StringLikeInput>(index: u32, secondary: I, value: I) -> Line<I> {
        Line::Entry(Entry {
            index,
            secondary,
            value,
            comment: None,
        })
    }

    impl<'a> Entry<&'a str> {
        fn as_bytes(&self) -> Entry<&'a [u8]> {
            Entry {
                index: self.index,
                secondary: self.secondary.as_bytes(),
                value: self.value.as_bytes(),
                comment: self.comment.map(|comment| comment.as_bytes()),
            }
        }
    }

    #[test]
    fn lex_entry() {
        let samples = &[
            (
                "{4294967295}{             zxc}{zxc              zxc}",
                new_entry(4294967295, "             zxc", "zxc              zxc"),
            ),
            ("{0}{}{}", new_entry(0, "", "")),
            ("{1}{\n}{\n}", new_entry(1, "\n", "\n")),
            (
                "{2}{\n foo \n   \n}{\n\n\n   bar}",
                new_entry(2, "\n foo \n   \n", "\n\n\n   bar"),
            ),
        ];
        for (sample, correct) in samples {
            with_all_entry_impls(*sample, correct);
        }
        for (sample, correct) in samples {
            with_all_entry_impls(sample.as_bytes(), &correct.as_bytes());
        }
    }

    #[test]
    fn lex_msg() {
        const SAMPLE: &str = "\
            \n\
            # Transit Name, (pid + 1) * 10 + 8 pm added\n\
            \n\
            # Map 0, Global, base 10\n\
            {10}{}{Global map}\n\
            {15}{}{20car}\n\
            {15}{}{23world}\n\
            {15}{}{03 - A Way To Anywhere.ogg}\
        ";
        let correct = Msg {
            lines: vec![
                Line::Break,
                Line::Comment("Transit Name, (pid + 1) * 10 + 8 pm added"),
                Line::Break,
                Line::Comment("Map 0, Global, base 10"),
                entry_line(10, "", "Global map"),
                entry_line(15, "", "20car"),
                entry_line(15, "", "23world"),
                entry_line(15, "", "03 - A Way To Anywhere.ogg"),
            ],
        };
        assert_eq!(lex(msg, SAMPLE), correct);
    }
}
