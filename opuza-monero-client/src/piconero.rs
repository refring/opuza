use serde::Serialize;
use {
  regex::Regex,
  serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
  },
  std::fmt::{self, Display, Formatter},
};

#[derive(PartialEq, Debug, Clone, Copy, Serialize)]
pub struct Piconero(u64);

impl Piconero {
  pub const ONE_XMR: Piconero = Piconero(1_000_000_000_000);

  pub(crate) fn value(self) -> u64 {
    self.0
  }

  pub fn new(value: u64) -> Self {
    Self(value)
  }

  pub fn as_xmr(self) -> f64 {
    const BASE: f64 = 1000000000000.0;

    let xmr: f64 = self.0 as f64 / BASE;

    xmr
  }
}

impl<'de> Deserialize<'de> for Piconero {
  fn deserialize<D>(deserializer: D) -> Result<Piconero, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_str(PiconeroVisitor)
  }
}

struct PiconeroVisitor;

impl<'de> Visitor<'de> for PiconeroVisitor {
  type Value = Piconero;

  fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
    formatter.write_str("a string, e.g. \"1 XMR\"")
  }

  fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    let regex = Regex::new(r"^(\d+(?:\.\d+)?) XMR$").expect("regex is valid");
    let captures = regex.captures(value).ok_or_else(|| {
      de::Error::invalid_value(
        de::Unexpected::Str(value),
        &"integer number of XMR, including unit, e.g. \"1000 XMR\"",
      )
    })?;
    let value = captures[1].parse::<f64>().unwrap();
    Ok(Piconero((value * (1000000000000_u64 as f64)) as u64))
  }
}

impl Display for Piconero {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    use num_format::{Locale, ToFormattedString};

    const BASE: u64 = 1000000000000;

    write!(f, "{}", (self.0 / BASE).to_formatted_string(&Locale::en))?;

    let piconero = self.0 % BASE;

    if piconero > 0 {
      write!(
        f,
        ".{}",
        ((piconero as f64) / (BASE as f64))
          .to_string()
          .strip_prefix("0.")
          .expect("float string always starts with `0.`")
      )?;
    }

    write!(f, " XMR")?;

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  fn invalid_value(input: &str) {
    let expected = format!(
            "invalid value: string \"{}\", expected integer number of XMR, including unit, e.g. \"1000 XMR\" at line 1 column 1",
            serde_yaml::from_str::<String>(input).unwrap(),
        );
    assert_eq!(
      serde_yaml::from_str::<Piconero>(input)
        .unwrap_err()
        .to_string(),
      expected
    );
  }

  #[test]
  fn leading_space() {
    invalid_value("\" 1 XMR\"");
  }

  #[test]
  fn trailing_space() {
    invalid_value("\"1 XMR \"");
  }

  #[test]
  fn wrong_unit() {
    invalid_value("1 mXMR");
  }

  #[test]
  fn missing_unit() {
    invalid_value("\"1\"");
  }

  #[test]
  fn number_type() {
    invalid_value("1");
  }

  // #[test]
  // fn decimal_point() {
  //     invalid_value("1.1 XMR");
  // }

  #[test]
  fn negative_number() {
    invalid_value("-1 XMR");
  }

  #[test]
  fn list_input() {
    assert_eq!(
      serde_yaml::from_str::<Piconero>("[1]")
        .unwrap_err()
        .to_string(),
      "invalid type: sequence, expected a string, e.g. \"1 XMR\" at line 1 column 1"
    );
  }

  #[test]
  fn display_singular() {
    assert_eq!(Piconero::new(1).to_string(), "0.000000000001 XMR");
  }

  #[test]
  fn display_plural() {
    assert_eq!(Piconero::new(0).to_string(), "0 XMR");
  }

  #[test]
  fn display_piconeros_no_trailing_zeros() {
    assert_eq!(Piconero::new(10).to_string(), "0.00000000001 XMR");
  }

  #[test]
  fn display_xmr_with_comma() {
    assert_eq!(
      Piconero::new(1_000_000_000_000_000).to_string(),
      "1,000 XMR"
    );
  }

  #[test]
  fn display_xmr_with_comma_and_decimal_separator() {
    assert_eq!(
      Piconero::new(1_000_123_000_000_000).to_string(),
      "1,000.123 XMR"
    );
  }

  #[test]
  fn from_string_decimal() {
    assert_eq!(
      serde_yaml::from_str::<Piconero>("1.1 XMR")
        .unwrap()
        .to_string(),
      "1.1 XMR"
    );
  }

  #[test]
  fn from_string_decimal_zero() {
    assert_eq!(
      serde_yaml::from_str::<Piconero>("0.231 XMR")
        .unwrap()
        .to_string(),
      "0.231 XMR"
    );
  }

  #[test]
  fn from_string_no_decimal() {
    assert_eq!(
      serde_yaml::from_str::<Piconero>("2 XMR")
        .unwrap()
        .to_string(),
      "2 XMR"
    );
  }
}
