use std::iter::Peekable;

use embedded_i18n::translate;

use crate::{
    lexer::Lexer,
    token::{ConstantToken, FunctionToken, Token},
};

#[derive(Debug, Clone)]
pub enum Expression {
    Number(f64),
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    UnaryOp {
        op: UnaryOperator,
        expr: Box<Expression>,
    },
    FunctionCall {
        function: FunctionToken,
        arg: Box<Expression>,
    },
}

#[derive(Debug, Clone)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Modulo,
    Percent,
}

#[derive(Debug, Clone)]
pub enum UnaryOperator {
    Plus,
    Minus,
}

#[derive(Debug)]
pub struct Parser<'a> {
    lexer: Peekable<Lexer<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Result<Self, String> {
        let lexer = Lexer::new(input).peekable();

        Ok(Self { lexer })
    }

    fn current_token(&mut self) -> &Token<'_> {
        self.lexer.peek().unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) {
        self.lexer.next();
    }

    fn expect_token(&mut self, expected: Token) -> Result<(), String> {
        if self.current_token().get_discriminant() == expected.get_discriminant() {
            self.advance();
            Ok(())
        } else {
            Err(format!(
                translate!("Expected {:?}, found {:?}"),
                expected,
                self.current_token()
            ))
        }
    }

    pub fn parse(&mut self) -> Result<Expression, String> {
        self.parse_expression()
    }

    // Expression = Term (('+' | '-') Term)*
    fn parse_expression(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_term()?;

        while matches!(self.current_token(), Token::Plus | Token::Minus) {
            let op = match self.current_token() {
                Token::Plus => BinaryOperator::Add,
                Token::Minus => BinaryOperator::Subtract,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_term()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    // Term = Factor (('*' | '/' | 'mod' | '%') Factor)*
    fn parse_term(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_factor()?;

        loop {
            let op = match self.current_token() {
                Token::Multiply => BinaryOperator::Multiply,
                Token::Divide => BinaryOperator::Divide,
                Token::Identifier(s) if *s == "mod" => BinaryOperator::Modulo,
                Token::Identifier(s) if *s == "percent" => BinaryOperator::Percent,
                _ => break,
            };

            self.advance();
            let right = self.parse_factor()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    // Factor = Power ('^' Power)*
    fn parse_factor(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_power()?;

        while matches!(self.current_token(), Token::Power) {
            self.advance();
            let right = self.parse_power()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Power,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    // Power = ('+' | '-')? Primary
    fn parse_power(&mut self) -> Result<Expression, String> {
        match self.current_token() {
            Token::Plus => {
                self.advance();
                let expr = self.parse_primary()?;
                Ok(Expression::UnaryOp {
                    op: UnaryOperator::Plus,
                    expr: Box::new(expr),
                })
            }
            Token::Minus => {
                self.advance();
                let expr = self.parse_primary()?;
                Ok(Expression::UnaryOp {
                    op: UnaryOperator::Minus,
                    expr: Box::new(expr),
                })
            }
            _ => self.parse_primary(),
        }
    }

    // Primary = Number | '(' Expression ')' | Function '(' Expression ')' | Identifier
    fn parse_primary(&mut self) -> Result<Expression, String> {
        let primary = match self.current_token().clone() {
            Token::Number(value) => {
                self.advance();
                Expression::Number(value)
            }
            Token::LeftParenthesis => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect_token(Token::RightParenthesis)?;
                expr
            }
            Token::Function(name) => {
                self.advance();
                self.expect_token(Token::LeftParenthesis)?;
                let arg = self.parse_expression()?;
                self.expect_token(Token::RightParenthesis)?;
                Expression::FunctionCall {
                    function: name,
                    arg: Box::new(arg),
                }
            }
            Token::Constant(constant) => {
                self.advance();
                Expression::Number(match constant {
                    ConstantToken::Pi => std::f64::consts::PI,
                    ConstantToken::E => std::f64::consts::E,
                })
            }
            _ => {
                return Err(format!(
                    translate!("Unexpected token: {:?}"),
                    self.current_token()
                ));
            }
        };

        Ok(primary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_number() {
        let mut parser = Parser::new("42").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_addition() {
        let mut parser = Parser::new("2 + 3").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_subtraction() {
        let mut parser = Parser::new("5 - 2").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiplication() {
        let mut parser = Parser::new("4 * 3").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_division() {
        let mut parser = Parser::new("10 / 2").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_power() {
        let mut parser = Parser::new("2 ^ 3").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_modulo() {
        let mut parser = Parser::new("10 mod 3").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_percent() {
        let mut parser = Parser::new("50 percent 200").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parentheses() {
        let mut parser = Parser::new("(2 + 3) * 4").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_nested_parentheses() {
        let mut parser = Parser::new("((2 + 3) * (4 - 1))").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_unary_plus() {
        let mut parser = Parser::new("+5").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_unary_minus() {
        let mut parser = Parser::new("-5").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_complex_expression() {
        let mut parser = Parser::new("2 + 3 * 4 - 5 / 2").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_pi_constant() {
        let mut parser = Parser::new("pi").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
        if let Ok(Expression::Number(val)) = result {
            assert!((val - std::f64::consts::PI).abs() < 1e-10);
        }
    }

    #[test]
    fn test_pi_uppercase() {
        let mut parser = Parser::new("PI").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_e_constant() {
        let mut parser = Parser::new("e").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
        if let Ok(Expression::Number(val)) = result {
            assert!((val - std::f64::consts::E).abs() < 1e-10);
        }
    }

    #[test]
    fn test_e_uppercase() {
        let mut parser = Parser::new("E").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_call() {
        let mut parser = Parser::new("sin(0)").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_with_expression() {
        let mut parser = Parser::new("cos(2 + 3)").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_nested_functions() {
        let mut parser = Parser::new("sin(cos(0))").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_with_constant() {
        let mut parser = Parser::new("sin(pi)").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_unknown_identifier() {
        let mut parser = Parser::new("unknown").unwrap();
        let result = parser.parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_mismatched_parentheses() {
        let mut parser = Parser::new("(2 + 3").unwrap();
        let result = parser.parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_parentheses() {
        let mut parser = Parser::new("()").unwrap();
        let result = parser.parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_operations() {
        let mut parser = Parser::new("1 + 2 - 3 + 4").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_decimal_numbers() {
        let mut parser = Parser::new("3.14 + 2.71").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_negative_number_in_expression() {
        let mut parser = Parser::new("5 + -3").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_power_with_parentheses() {
        let mut parser = Parser::new("(2 + 3) ^ 2").unwrap();
        let result = parser.parse();
        assert!(result.is_ok());
    }
}
