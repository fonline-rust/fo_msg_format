mod lexer;

use std::collections::btree_map::BTreeMap;

#[derive(Debug, PartialEq)]
pub struct MsgDictionary {
    index_to_line: BTreeMap<(u32, u32), MsgLine>,
}

#[derive(Debug, PartialEq)]
pub enum MsgLine {
    String(Box<str>),
    Bytes(Box<[u8]>),
}
impl MsgLine {
    fn string(&self) -> Option<&str> {
        match self {
            MsgLine::String(string) => Some(string),
            MsgLine::Bytes(_) => None,
        }
    }

    fn bytes(&self) -> &[u8] {
        match self {
            MsgLine::String(string) => string.as_bytes(),
            MsgLine::Bytes(bytes) => bytes,
        }
    }
}

impl MsgDictionary {
    fn new() -> Self {
        Self {
            index_to_line: BTreeMap::new(),
        }
    }

    pub fn get_first_string(&self, index: u32) -> Option<&str> {
        self.index_to_line
            .get(&(index, 0))
            .and_then(MsgLine::string)
    }

    pub fn get_first_bytes(&self, index: u32) -> Option<&[u8]> {
        self.index_to_line.get(&(index, 0)).map(MsgLine::bytes)
    }

    pub fn get_all_strings(&self, index: u32) -> impl Iterator<Item = (u32, &str)> {
        self.index_to_line
            .range((index, 0)..(index, u32::MAX))
            .filter_map(|(&(_index, sub_index), value)| Some((sub_index, value.string()?)))
    }

    pub fn insert(&mut self, index: u32, value: MsgLine) {
        let sub_index = self
            .index_to_line
            .range((index, 0)..(index, u32::MAX))
            .last()
            .map(|((_index, sub_index), _value)| sub_index + 1)
            .unwrap_or(0);
        let old = self.index_to_line.insert((index, sub_index), value);
        assert_eq!(old, None);
    }

    pub fn iter_first_strings(&self) -> impl Iterator<Item = (u32, &str)> {
        self.index_to_line
            .iter()
            .filter_map(|(&(index, sub_index), value)| {
                if sub_index == 0 {
                    Some((index, value.string()?))
                } else {
                    None
                }
            })
    }
}

#[derive(Debug, PartialEq)]
struct Msg<I> {
    lines: Vec<Line<I>>,
}

#[derive(Debug, PartialEq)]
enum Line<I> {
    Entry(Entry<I>),
    Break,
    Comment(I),
}

#[derive(Debug, PartialEq)]
struct Entry<I> {
    index: u32,
    secondary: I,
    value: I,
    comment: Option<I>,
}

pub fn parse_msg(input: &[u8]) -> Result<MsgDictionary, String> {
    parse_msg_ext(input, |bytes| match std::str::from_utf8(bytes) {
        Ok(str) => MsgLine::String(str.into()),
        Err(_) => MsgLine::Bytes(bytes.into()),
    })
}

pub fn parse_msg_ext(
    input: &[u8],
    line_converter: impl Fn(&[u8]) -> MsgLine,
) -> Result<MsgDictionary, String> {
    let msg = lexer::tokenize_msg(input, true)?;
    let mut dict = MsgDictionary::new();
    for line in msg.lines {
        match line {
            Line::Entry(entry) => {
                if !entry.secondary.is_empty() {
                    panic!("Non-empty secondary key! {:?}", entry);
                }
                dict.insert(entry.index, line_converter(entry.value))
            }
            Line::Break | Line::Comment(_) => {
                //ignore line breaks and comments
            }
        }
    }
    Ok(dict)
}

#[cfg(any(test, feature = "cp1251"))]
pub fn parse_cp1251_file<P: AsRef<std::path::Path>>(path: P) -> Result<MsgDictionary, String> {
    let bytes = std::fs::read(path).map_err(|err| format!("IoError: {}", err))?;

    //println!("{:?}", cow.as_ref());
    parse_msg_ext(&bytes, |bytes| {
        use encoding_rs::*;
        let (cow, _encoding_used, had_errors) = WINDOWS_1251.decode(bytes);
        if had_errors {
            MsgLine::String(cow.into())
        } else {
            MsgLine::Bytes(bytes.into())
        }
    })
}

pub fn parse_file<P: AsRef<std::path::Path>>(path: P) -> Result<MsgDictionary, String> {
    let bytes = std::fs::read(path).map_err(|err| format!("IoError: {}", err))?;
    parse_msg(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample() {
        const SAMPLE: &[u8] = b"\
            # Transit Name, (pid + 1) * 10 + 8 pm added\n\
            \n\
            # Map 0, Global, base 10\n\
            {10}{}{Global map}\n\
            {15}{}{20car}\n\
            {15}{}{23world}\n\
            {15}{}{03 - A Way To Anywhere.ogg}\
        ";
        let dict = parse_msg(SAMPLE).unwrap();
        let correct = mock_dict(&[
            ((10, 0), "Global map"),
            ((15, 0), "20car"),
            ((15, 1), "23world"),
            ((15, 2), "03 - A Way To Anywhere.ogg"),
        ]);
        assert_eq!(dict, correct);
    }

    fn mock_dict(data: &[((u32, u32), &str)]) -> MsgDictionary {
        let mut dict = MsgDictionary::new();
        for &((index, sub_index), value) in data {
            dict.index_to_line
                .insert((index, sub_index), MsgLine::String(value.into()));
        }
        dict
    }

    fn file_id<P: AsRef<std::path::Path>>(file: P) -> Option<String> {
        let file = file.as_ref();
        Some(format!(
            "{}/{}",
            file.parent()?.file_name()?.to_str()?,
            file.file_name()?.to_str()?,
        ))
    }

    #[test]
    fn parse_all_forp_msg_files() {
        let mut vec = vec![];
        for dir in &["../../../FO4RP/text/engl"] {
            //, "../../../FO4RP/text/russ"] {
            for file in std::fs::read_dir(dir).unwrap() {
                let path = file.unwrap().path();
                if let Some(ext) = path.extension() {
                    if &ext.to_str().unwrap().to_uppercase() == "MSG" {
                        //if path.file_name().unwrap() == "FOGM.MSG" {
                        let path_str = path.to_str().unwrap();
                        let res = parse_cp1251_file(&path).expect(path_str);
                        vec.push((file_id(path).unwrap(), res));
                    }
                }
            }
        }
        /*for (file, dict) in vec {
            for ((index, sub_index), value) in &dict.index_to_string {
                println!("[{}][{}][{}] \"{}\"", file, index, sub_index, value);
            }
        }*/
    }
}
