
use serde::{Serialize, Deserialize};
use std::io;
use std::fs::File;
use crate::glyphs::Glyph;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadlexEntry {
  pub latin: String,
  pub shaw: String,
  pub ipa: String,
  pub freq: u32,
}

pub fn read_readlex_top5000() -> io::Result<Vec<ReadlexEntry>> {
  serde_json::from_reader(
    io::BufReader::new(
      File::open("res/readlex-entries-top5000.json")?
    )
  ).map_err(|e| e.into())
}

pub fn shaw_char_to_glyphs(sh: char) -> Option<Vec<Glyph>> {
  use Glyph::*;
  Some(match sh {
    '𐑑' => vec![T],
    '𐑔' => vec![Th],
    '𐑩' => vec![Schwa],
    '𐑴' => vec![O],
    '𐑟' => vec![Z],
    '𐑪' => vec![Ah],
    '𐑥' => vec![M],
    '𐑙' => vec![Ng],
    '𐑳' => vec![Uh],
    '𐑐' => vec![P],
    '𐑚' => vec![B],
    '𐑓' => vec![F],
    '𐑝' => vec![V],
    '𐑯' => vec![N],
    '𐑛' => vec![D],
    '𐑤' => vec![L],
    '𐑶' => vec![Oi],
    '𐑦' => vec![Ih],
    '𐑲' => vec![I],
    '𐑕' => vec![S],
    '𐑒' => vec![K],
    '𐑿' => vec![Yu],
    '𐑷' => vec![Aw],
    '𐑼' => vec![Er],
    '𐑖' => vec![Sh],
    '𐑠' => vec![Jh],
    '𐑗' => vec![Ch],
    '𐑡' => vec![J],
    '𐑘' => vec![Y],
    '𐑢' => vec![W],
    '𐑣' => vec![H],
    '𐑮' => vec![R],
    '𐑰' => vec![Ee],
    '𐑧' => vec![Eh],
    '𐑱' => vec![Ei],
    '𐑨' => vec![Ae],
    '𐑫' => vec![Eu],
    '𐑵' => vec![U],
    '𐑬' => vec![Ow],
    '𐑸' => vec![Ah, R],
    '𐑺' => vec![Ei, R],
    '𐑻' => vec![Eh, R],
    '𐑽' => vec![Ee, R],
    '𐑹' => vec![Aw, R],
    '𐑾' => vec![Ee, Eh],
    _ => None?
  })
}

pub fn shaw_word_to_glyphs(sh: &str) -> Vec<Glyph> {
  let mut res = vec![];

  for c in sh.chars() {
    if let Some(gs) = shaw_char_to_glyphs(c) {
      res.extend(gs);
    }
  }

  res
}

pub fn shaw_word_to_glyphs_with_fixes(sh: &str, latin: &str) -> Vec<Glyph> {
  let mut res = vec![];

  for c in sh.chars() {
    if let Some(gs) = shaw_char_to_glyphs(c) {
      res.extend(gs);
    }
  }

  let n = res.len();
  if n > 0 && res[n - 1] == Glyph::Ih {
    res[n - 1] = Glyph::Ee;
  }

  if n > 0 && res[0] == Glyph::Ih {
    let latin_glyphs = crate::glyphs::decode(latin);
    if latin_glyphs.len() > 0 && latin_glyphs[0] == Glyph::E {
      res[0] = Glyph::Schwa;
    }
  }

  res
}

pub fn fix_final_ih(gs: &Vec<Glyph>) -> Vec<Glyph> {
  let mut res = gs.clone();
  let n = res.len();
  if n > 0 && res[n - 1] == Glyph::Ih {
    res[n - 1] = Glyph::Ee;
  }
  res
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_shaw_word_to_glyphs() {
    use Glyph::*;
    assert_eq!(shaw_word_to_glyphs("𐑒𐑸"), vec![K, Ah, R]);
    assert_eq!(shaw_word_to_glyphs("𐑒𐑸-𐑒𐑸"), vec![K, Ah, R, K, Ah, R]);
  }
}

