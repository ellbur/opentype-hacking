
use std::{io::{self}, path::Path, process::{Command, ExitStatus}, str::from_utf8, fs};

use lazy_static::lazy_static;
use regex::Regex;
use tempfile::{Builder, NamedTempFile};

use crate::{substitutions2::*, glyphs::AugGlyph};
use crate::fea_parsing as p;

pub fn apply_using_hbshape(slist: &SubstitutionList, text: &Vec<AugGlyph>) -> io::Result<Vec<AugGlyph>> {
  let encoded_text = &crate::glyphs::aug_encode(text);
  
  let fea_file = Builder::new().suffix(".fea").tempfile()?;
  let in_otf_path = Path::new("../t1-1.otf");
  let out_otf_file = Builder::new().suffix(".otf").tempfile()?;
  
  let text = format!("
@lc = [a b c d e f g h i j k l m n o p q r s t u v w x y z];
  
feature rlig {{
  {}
}} rlig;
", p::render_fea_feature_body(slist));
  
  fs::write(&fea_file, text)?;
  
  // fonttools feaLib -o t1-1-with-feats.otf features.fea t1-1.otf
  successful(Command::new("fonttools").args(["feaLib", "-o", p(&out_otf_file), p(&fea_file), p2(in_otf_path)]).status()?, "fonttools")?;
  
  // hb-shape t1-1-with-feats.otf 'you'
  let output = Command::new("hb-shape").args([p(&out_otf_file), encoded_text]).output()?;
  successful(output.status, "hb-shape")?;
  
  let output_text = from_utf8(&output.stdout).unwrap();
  let output_text = &strip_shaping_stuff(output_text);
  Ok(crate::glyphs::aug_decode(output_text))
}

lazy_static! {
  static ref STRIPPING_RE: Regex = Regex::new(r"^\[|\=\d+\+\d+\|?|\]$").unwrap();
}

fn strip_shaping_stuff(s: &str) -> String {
  // [y=0+400|u=1+1000]
  STRIPPING_RE.replace_all(s, "").into_owned()
}

#[cfg(test)]
mod stripping_tests {
  use super::*;
  #[test]
  fn test_stripping_1() {
    assert_eq!(strip_shaping_stuff("[y=0+400|u=1+1000]"), "yu".to_owned());
  }
}

fn p(f: &NamedTempFile) -> &str { f.path().to_str().unwrap() }
fn p2(f: &Path) -> &str { f.to_str().unwrap() }

fn successful(e: ExitStatus, name: &str) -> io::Result<()> {
  match e.code().ok_or(io::Error::new(io::ErrorKind::Other, format!("No return code {}", name)))? {
    0 => Ok(()),
    code => Err(io::Error::new(io::ErrorKind::Other, format!("Nonzero return code {} {}", name, code)))
  }
}

#[cfg(test)]
mod tests {
  use crate::substitutions2::{SubstitutionList, Lookup};
  use super::*;
  
  fn r(g: crate::glyphs::Glyph) -> AugGlyph { AugGlyph::Real(g) }
  fn rr(g: &[crate::glyphs::Glyph]) -> Vec<AugGlyph> { g.iter().map(|g| r(*g)).collect() }

  #[test]
  fn test_1() {
    assert_eq!(apply_using_hbshape(&SubstitutionList { lookups: vec![Lookup { substitutions: vec![] }] }, &vec![]).unwrap(), vec![]);
  }
  
  #[test]
  fn test_2() {
    use crate::glyphs::Glyph::*;
    assert_eq!(
      apply_using_hbshape(
        &crate::fea_parsing::parse_fea_feature_body("
          lookup l0 {
            sub a by b;
          } l0;
        ").unwrap(),
        &rr(&[A])
      ).unwrap(),
      rr(&[B])
    );
  }
  
  #[test]
  fn test_3() {
    use crate::glyphs::Glyph::*;
    assert_eq!(
      apply_using_hbshape(
        &crate::fea_parsing::parse_fea_feature_body("
          lookup l0 {
            sub a' b by c;
            sub c b' by d;
          } l0;
        ").unwrap(),
        &rr(&[A, B])
      ).unwrap(),
      rr(&[C, D])
    );
  }
  
  #[test]
  fn test_4() {
    use crate::glyphs::Glyph::*;
    assert_eq!(
      apply_using_hbshape(
        &crate::fea_parsing::parse_fea_feature_body("
          lookup l0 {
            sub a  b' by c;
            sub a' c  by d;
          } l0;
        ").unwrap(),
        &rr(&[A, B])
      ).unwrap(),
      rr(&[A, C])
    );
  }
  
  #[test]
  fn test_5() {
    use crate::glyphs::Glyph::*;
    assert_eq!(
      apply_using_hbshape(
        &crate::fea_parsing::parse_fea_feature_body("
          lookup l0 {
            sub a' by c d;
            sub d' by e;
          } l0;
        ").unwrap(),
        &rr(&[A, B])
      ).unwrap(),
      rr(&[C, D, B])
    );
  }
}

