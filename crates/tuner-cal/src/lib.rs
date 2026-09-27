use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Conversion {
    source: String,
    expression: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConversionError {
    Parse { position: usize, message: String },
    UnknownIdentifier { position: usize, identifier: String },
    NonFiniteInput,
    DivisionByZero,
    NonFiniteResult,
    NotInvertible,
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { position, message } => {
                write!(f, "conversion parse error at {position}: {message}")
            }
            Self::UnknownIdentifier {
                position,
                identifier,
            } => write!(
                f,
                "unknown conversion identifier {identifier:?} at {position}"
            ),
            Self::NonFiniteInput => write!(f, "conversion input must be finite"),
            Self::DivisionByZero => write!(f, "conversion attempted division by zero"),
            Self::NonFiniteResult => write!(f, "conversion result is not finite"),
            Self::NotInvertible => write!(f, "conversion is not invertible"),
        }
    }
}

impl std::error::Error for ConversionError {}

impl Conversion {
    pub fn parse(source: &str) -> Result<Self, ConversionError> {
        let mut parser = Parser::new(source);
        let expression = parser.parse_expression()?;
        parser.skip_whitespace();
        if let Some(character) = parser.current() {
            return Err(parser.parse_error(format!("unexpected character {character:?}")));
        }
        Ok(Self {
            source: source.to_string(),
            expression,
        })
    }

    pub fn evaluate(&self, raw: f64) -> Result<f64, ConversionError> {
        if !raw.is_finite() {
            return Err(ConversionError::NonFiniteInput);
        }
        let value = self.expression.evaluate(raw)?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(ConversionError::NonFiniteResult)
        }
    }

    pub fn invert(&self, engineering: f64) -> Result<f64, ConversionError> {
        if !engineering.is_finite() {
            return Err(ConversionError::NonFiniteInput);
        }
        let (numerator, denominator) = self.expression.linear_fraction()?;
        if numerator.slope == 0.0 && denominator.slope == 0.0 {
            if denominator.intercept == 0.0 {
                return Err(ConversionError::DivisionByZero);
            }
            return Err(ConversionError::NotInvertible);
        }
        let raw = if denominator.slope == 0.0 {
            if numerator.slope == 0.0 {
                return Err(ConversionError::NotInvertible);
            }
            (engineering * denominator.intercept - numerator.intercept) / numerator.slope
        } else {
            let inverse_denominator = engineering * denominator.slope - numerator.slope;
            if inverse_denominator == 0.0 {
                return Err(ConversionError::NotInvertible);
            }
            (numerator.intercept - engineering * denominator.intercept) / inverse_denominator
        };
        if !raw.is_finite() {
            return Err(ConversionError::NonFiniteResult);
        }
        if denominator.evaluate(raw) == 0.0 {
            return Err(ConversionError::DivisionByZero);
        }
        Ok(raw)
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Number(f64),
    Variable,
    Add(Box<Self>, Box<Self>),
    Sub(Box<Self>, Box<Self>),
    Mul(Box<Self>, Box<Self>),
    Div(Box<Self>, Box<Self>),
    Neg(Box<Self>),
}

impl Expr {
    fn evaluate(&self, raw: f64) -> Result<f64, ConversionError> {
        let value = match self {
            Self::Number(value) => *value,
            Self::Variable => raw,
            Self::Add(left, right) => left.evaluate(raw)? + right.evaluate(raw)?,
            Self::Sub(left, right) => left.evaluate(raw)? - right.evaluate(raw)?,
            Self::Mul(left, right) => left.evaluate(raw)? * right.evaluate(raw)?,
            Self::Div(left, right) => {
                let numerator = left.evaluate(raw)?;
                let denominator = right.evaluate(raw)?;
                if denominator == 0.0 {
                    return Err(ConversionError::DivisionByZero);
                }
                numerator / denominator
            }
            Self::Neg(value) => -value.evaluate(raw)?,
        };
        if value.is_finite() {
            Ok(value)
        } else {
            Err(ConversionError::NonFiniteResult)
        }
    }

    fn linear_fraction(&self) -> Result<(Affine, Affine), ConversionError> {
        match self {
            Self::Div(left, right) => {
                let numerator = left.to_affine()?;
                let denominator = right.to_affine()?;
                if denominator.slope == 0.0 && denominator.intercept == 0.0 {
                    return Err(ConversionError::DivisionByZero);
                }
                Ok((numerator, denominator))
            }
            _ => Ok((self.to_affine()?, Affine::constant(1.0))),
        }
    }

    fn to_affine(&self) -> Result<Affine, ConversionError> {
        match self {
            Self::Number(value) => Ok(Affine::constant(*value)),
            Self::Variable => Ok(Affine {
                slope: 1.0,
                intercept: 0.0,
            }),
            Self::Add(left, right) => Ok(left.to_affine()?.add(right.to_affine()?)),
            Self::Sub(left, right) => Ok(left.to_affine()?.sub(right.to_affine()?)),
            Self::Neg(value) => Ok(value.to_affine()?.scale(-1.0)),
            Self::Mul(left, right) => {
                let left = left.to_affine()?;
                let right = right.to_affine()?;
                if left.slope != 0.0 && right.slope != 0.0 {
                    return Err(ConversionError::NotInvertible);
                }
                Ok(if left.slope == 0.0 {
                    right.scale(left.intercept)
                } else {
                    left.scale(right.intercept)
                })
            }
            Self::Div(left, right) => {
                let left = left.to_affine()?;
                let right = right.to_affine()?;
                if right.slope != 0.0 {
                    return Err(ConversionError::NotInvertible);
                }
                if right.intercept == 0.0 {
                    return Err(ConversionError::DivisionByZero);
                }
                Ok(left.scale(1.0 / right.intercept))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Affine {
    slope: f64,
    intercept: f64,
}

impl Affine {
    const fn constant(value: f64) -> Self {
        Self {
            slope: 0.0,
            intercept: value,
        }
    }

    fn add(self, other: Self) -> Self {
        Self {
            slope: self.slope + other.slope,
            intercept: self.intercept + other.intercept,
        }
    }

    fn sub(self, other: Self) -> Self {
        Self {
            slope: self.slope - other.slope,
            intercept: self.intercept - other.intercept,
        }
    }

    fn scale(self, factor: f64) -> Self {
        Self {
            slope: self.slope * factor,
            intercept: self.intercept * factor,
        }
    }

    fn evaluate(self, raw: f64) -> f64 {
        self.slope * raw + self.intercept
    }
}

struct Parser<'a> {
    source: &'a str,
    chars: Vec<char>,
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars().collect(),
            position: 0,
        }
    }

    fn parse_expression(&mut self) -> Result<Expr, ConversionError> {
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> Result<Expr, ConversionError> {
        let mut expression = self.parse_multiplicative()?;
        loop {
            self.skip_whitespace();
            let operator = match self.current() {
                Some('+') => Some('+'),
                Some('-') => Some('-'),
                _ => None,
            };
            let Some(operator) = operator else {
                break;
            };
            self.position += 1;
            let right = self.parse_multiplicative()?;
            expression = if operator == '+' {
                Expr::Add(Box::new(expression), Box::new(right))
            } else {
                Expr::Sub(Box::new(expression), Box::new(right))
            };
        }
        Ok(expression)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ConversionError> {
        let mut expression = self.parse_unary()?;
        loop {
            self.skip_whitespace();
            let operator = match self.current() {
                Some('*') => Some('*'),
                Some('/') => Some('/'),
                _ => None,
            };
            let Some(operator) = operator else {
                break;
            };
            self.position += 1;
            let right = self.parse_unary()?;
            expression = if operator == '*' {
                Expr::Mul(Box::new(expression), Box::new(right))
            } else {
                Expr::Div(Box::new(expression), Box::new(right))
            };
        }
        Ok(expression)
    }

    fn parse_unary(&mut self) -> Result<Expr, ConversionError> {
        self.skip_whitespace();
        match self.current() {
            Some('+') => {
                self.position += 1;
                self.parse_unary()
            }
            Some('-') => {
                self.position += 1;
                Ok(Expr::Neg(Box::new(self.parse_unary()?)))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ConversionError> {
        self.skip_whitespace();
        match self.current() {
            Some('(') => {
                self.position += 1;
                let expression = self.parse_expression()?;
                self.skip_whitespace();
                if self.current() != Some(')') {
                    return Err(self.parse_error("expected ')'"));
                }
                self.position += 1;
                Ok(expression)
            }
            Some(character) if character.is_ascii_digit() || character == '.' => {
                self.parse_number()
            }
            Some(character) if character.is_ascii_alphabetic() || character == '_' => {
                self.parse_identifier()
            }
            Some(character) => Err(self.parse_error(format!("unexpected character {character:?}"))),
            None => Err(self.parse_error("expected a number, X, or '('")),
        }
    }

    fn parse_number(&mut self) -> Result<Expr, ConversionError> {
        let start = self.position;
        let mut digits_before = 0usize;
        while self
            .current()
            .is_some_and(|character| character.is_ascii_digit())
        {
            digits_before += 1;
            self.position += 1;
        }
        let mut digits_after = 0usize;
        if self.current() == Some('.') {
            self.position += 1;
            while self
                .current()
                .is_some_and(|character| character.is_ascii_digit())
            {
                digits_after += 1;
                self.position += 1;
            }
        }
        if digits_before == 0 && digits_after == 0 {
            return Err(self.parse_error("number requires at least one digit"));
        }
        if self
            .current()
            .is_some_and(|character| character == 'e' || character == 'E')
        {
            self.position += 1;
            if self
                .current()
                .is_some_and(|character| character == '+' || character == '-')
            {
                self.position += 1;
            }
            let exponent_start = self.position;
            while self
                .current()
                .is_some_and(|character| character.is_ascii_digit())
            {
                self.position += 1;
            }
            if exponent_start == self.position {
                return Err(self.parse_error("scientific notation requires exponent digits"));
            }
        }
        let token: String = self.chars[start..self.position].iter().collect();
        let value = token
            .parse::<f64>()
            .map_err(|_| self.parse_error(format!("invalid number {token:?}")))?;
        if !value.is_finite() {
            return Err(self.parse_error("number must be finite"));
        }
        Ok(Expr::Number(value))
    }

    fn parse_identifier(&mut self) -> Result<Expr, ConversionError> {
        let start = self.position;
        self.position += 1;
        while self
            .current()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            self.position += 1;
        }
        let identifier: String = self.chars[start..self.position].iter().collect();
        if identifier == "X" {
            Ok(Expr::Variable)
        } else {
            Err(ConversionError::UnknownIdentifier {
                position: start,
                identifier,
            })
        }
    }

    fn skip_whitespace(&mut self) {
        while self
            .current()
            .is_some_and(|character| character.is_ascii_whitespace())
        {
            self.position += 1;
        }
    }

    fn current(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn parse_error(&self, message: impl Into<String>) -> ConversionError {
        let _ = self.source;
        ConversionError::Parse {
            position: self.position,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Conversion, ConversionError};

    #[test]
    fn identity_and_precedence_are_evaluable() {
        let conversion = Conversion::parse("2 * X + 1").unwrap();
        assert_eq!(conversion.evaluate(3.0).unwrap(), 7.0);
    }

    #[test]
    fn linear_fractional_conversion_round_trips() {
        let conversion = Conversion::parse("(2 * X + 4) / (3 * X + 5)").unwrap();
        let engineering = conversion.evaluate(2.0).unwrap();
        assert!((conversion.invert(engineering).unwrap() - 2.0).abs() < 1e-12);
    }

    #[test]
    fn source_is_preserved() {
        let conversion = Conversion::parse(" X / 2 ").unwrap();
        assert_eq!(conversion.source(), " X / 2 ");
    }

    #[test]
    fn division_by_zero_is_rejected_during_evaluation() {
        let conversion = Conversion::parse("X / (X - 2)").unwrap();
        assert_eq!(
            conversion.evaluate(2.0),
            Err(ConversionError::DivisionByZero)
        );
    }

    #[test]
    fn parses_unary_scientific_numbers_and_parentheses() {
        let conversion = Conversion::parse("-(1.5e1 + 2) / 2").unwrap();
        assert_eq!(conversion.evaluate(99.0).unwrap(), -8.5);
    }

    #[test]
    fn rejects_unknown_identifiers_and_malformed_parentheses() {
        assert!(matches!(
            Conversion::parse("Y + 1"),
            Err(ConversionError::UnknownIdentifier { .. })
        ));
        assert!(matches!(
            Conversion::parse("(X + 1"),
            Err(ConversionError::Parse { .. })
        ));
    }

    #[test]
    fn rejects_non_finite_input_and_result() {
        let conversion = Conversion::parse("X * 1e308").unwrap();
        assert_eq!(
            conversion.evaluate(f64::INFINITY),
            Err(ConversionError::NonFiniteInput)
        );
        assert_eq!(
            conversion.evaluate(2.0),
            Err(ConversionError::NonFiniteResult)
        );
    }

    #[test]
    fn affine_inverse_and_constant_inverse_are_distinct() {
        let affine = Conversion::parse("2 * X + 3").unwrap();
        assert_eq!(affine.invert(11.0).unwrap(), 4.0);

        let constant = Conversion::parse("7").unwrap();
        assert_eq!(constant.invert(7.0), Err(ConversionError::NotInvertible));
    }

    #[test]
    fn rejects_singular_and_non_linear_inverses() {
        let singular = Conversion::parse("X / (X - 1)").unwrap();
        assert_eq!(singular.invert(1.0), Err(ConversionError::NotInvertible));

        let nonlinear = Conversion::parse("X * X").unwrap();
        assert_eq!(nonlinear.invert(4.0), Err(ConversionError::NotInvertible));
    }
}
