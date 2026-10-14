use crate::parser::{BinaryOperator, Expression, UnaryOperator};
use crate::token::FunctionToken;
use embedded_i18n::translate;

pub struct Evaluator;

impl Evaluator {
    pub fn evaluate(expression: &Expression) -> Result<f64, String> {
        match expression {
            Expression::Number(value) => Ok(*value),
            Expression::BinaryOp { left, op, right } => {
                let left_val = Self::evaluate(left)?;
                let right_val = Self::evaluate(right)?;

                match op {
                    BinaryOperator::Add => Ok(left_val + right_val),
                    BinaryOperator::Subtract => Ok(left_val - right_val),
                    BinaryOperator::Multiply => Ok(left_val * right_val),
                    BinaryOperator::Divide => {
                        if right_val == 0.0 {
                            Err(translate!("Division by zero").to_string())
                        } else {
                            Ok(left_val / right_val)
                        }
                    }
                    BinaryOperator::Power => Ok(left_val.powf(right_val)),
                    BinaryOperator::Modulo => {
                        if right_val == 0.0 {
                            Err(translate!("Modulo by zero").to_string())
                        } else {
                            Ok(left_val % right_val)
                        }
                    }
                    BinaryOperator::Percent => {
                        // Percent operation: left% = left/100
                        Ok(left_val / 100.0)
                    }
                }
            }
            Expression::UnaryOp { op, expr } => {
                let val = Self::evaluate(expr)?;

                match op {
                    UnaryOperator::Plus => Ok(val),
                    UnaryOperator::Minus => Ok(-val),
                }
            }
            Expression::FunctionCall { function, arg } => {
                let arg_val = Self::evaluate(arg)?;

                match function {
                    FunctionToken::SquareRoot => {
                        if arg_val < 0.0 {
                            Err(translate!("Square root of negative number").to_string())
                        } else {
                            Ok(arg_val.sqrt())
                        }
                    }
                    FunctionToken::Sine => Ok(arg_val.sin()),
                    FunctionToken::Cosine => Ok(arg_val.cos()),
                    FunctionToken::Tangent => Ok(arg_val.tan()),
                    FunctionToken::Sind => Ok((arg_val * std::f64::consts::PI / 180.0).sin()), // degrees
                    FunctionToken::Cosd => Ok((arg_val * std::f64::consts::PI / 180.0).cos()), // degrees
                    FunctionToken::Tand => Ok((arg_val * std::f64::consts::PI / 180.0).tan()), // degrees
                    FunctionToken::HyperbolicSine => Ok(arg_val.sinh()),
                    FunctionToken::HyperbolicCosine => Ok(arg_val.cosh()),
                    FunctionToken::HyperbolicTangent => Ok(arg_val.tanh()),
                    FunctionToken::Logarithm => {
                        if arg_val <= 0.0 {
                            Err(translate!("Logarithm of non-positive number").to_string())
                        } else {
                            Ok(arg_val.log10())
                        }
                    }
                    FunctionToken::NaturalLogarithm => {
                        if arg_val <= 0.0 {
                            Err(translate!("Natural logarithm of non-positive number").to_string())
                        } else {
                            Ok(arg_val.ln())
                        }
                    }
                    FunctionToken::Exponential => Ok(arg_val.exp()),
                    FunctionToken::AbsoluteValue => Ok(arg_val.abs()),
                    FunctionToken::Sqr => Ok(arg_val * arg_val),
                    FunctionToken::Cube => Ok(arg_val * arg_val * arg_val),
                    FunctionToken::Power10 => Ok(10.0_f64.powf(arg_val)),
                    FunctionToken::Factorial => {
                        if arg_val < 0.0 || arg_val.fract() != 0.0 {
                            Err(translate!("Factorial requires non-negative integer").to_string())
                        } else if arg_val > 170.0 {
                            Err(translate!("Factorial too large").to_string())
                        } else {
                            let n = arg_val as u64;
                            let mut result = 1.0;
                            for i in 1..=n {
                                result *= i as f64;
                            }
                            Ok(result)
                        }
                    }
                    FunctionToken::Inverse => {
                        if arg_val == 0.0 {
                            Err(translate!("Division by zero").to_string())
                        } else {
                            Ok(1.0 / arg_val)
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evaluate_expression(input: &str) -> Result<f64, String> {
        let mut parser = crate::parser::Parser::new(input)?;
        let expression = parser.parse()?;
        Evaluator::evaluate(&expression)
    }

    #[test]
    fn test_basic_arithmetic() {
        assert_eq!(evaluate_expression("2 + 3").unwrap(), 5.0);
        assert_eq!(evaluate_expression("10 - 4").unwrap(), 6.0);
        assert_eq!(evaluate_expression("6 * 7").unwrap(), 42.0);
        assert_eq!(evaluate_expression("15 / 3").unwrap(), 5.0);
    }

    #[test]
    fn test_parentheses() {
        assert_eq!(evaluate_expression("(2 + 3) * 4").unwrap(), 20.0);
        assert_eq!(evaluate_expression("2 + (3 * 4)").unwrap(), 14.0);
    }

    #[test]
    fn test_power() {
        assert_eq!(evaluate_expression("2 ^ 3").unwrap(), 8.0);
        assert_eq!(evaluate_expression("10 ^ 2").unwrap(), 100.0);
    }

    #[test]
    fn test_functions() {
        assert_eq!(evaluate_expression("sqrt(16)").unwrap(), 4.0);
        assert_eq!(evaluate_expression("abs(-5)").unwrap(), 5.0);
    }

    #[test]
    fn test_complex_expression() {
        // Test the target expression: ((5 / 8) + sqrt(18) x 10^5 ) / 23
        let result = evaluate_expression("((5 / 8) + sqrt(18) * 10^5) / 23").unwrap();

        // Calculate expected result step by step
        let part1 = 5.0 / 8.0; // 0.625
        let part2 = 18.0_f64.sqrt() * (10.0_f64.powf(5.0)); // sqrt(18) * 100000
        let numerator = part1 + part2;
        let expected = numerator / 23.0;

        assert!((result - expected).abs() < 1e-10);
    }

    #[test]
    fn test_new_functions() {
        // Test hyperbolic functions
        assert!((evaluate_expression("sinh(0)").unwrap() - 0.0).abs() < 1e-10);
        assert!((evaluate_expression("cosh(0)").unwrap() - 1.0).abs() < 1e-10);
        assert!((evaluate_expression("tanh(0)").unwrap() - 0.0).abs() < 1e-10);

        // Test square and cube
        assert_eq!(evaluate_expression("sqr(5)").unwrap(), 25.0);
        assert_eq!(evaluate_expression("cube(3)").unwrap(), 27.0);

        // Test factorial
        assert_eq!(evaluate_expression("fact(5)").unwrap(), 120.0);
        assert_eq!(evaluate_expression("fact(0)").unwrap(), 1.0);

        // Test power of 10
        assert_eq!(evaluate_expression("pow10(2)").unwrap(), 100.0);
        assert_eq!(evaluate_expression("pow10(0)").unwrap(), 1.0);

        // Test inverse
        assert_eq!(evaluate_expression("inv(2)").unwrap(), 0.5);
        assert_eq!(evaluate_expression("inv(4)").unwrap(), 0.25);
        assert!((evaluate_expression("inv(3)").unwrap() - (1.0 / 3.0)).abs() < 1e-10);

        // Test absolute value
        assert_eq!(evaluate_expression("abs(-5)").unwrap(), 5.0);
        assert_eq!(evaluate_expression("abs(3)").unwrap(), 3.0);
        assert_eq!(evaluate_expression("abs(0)").unwrap(), 0.0);

        // Test modulo
        assert_eq!(evaluate_expression("10 mod 3").unwrap(), 1.0);
        assert_eq!(evaluate_expression("7 mod 2").unwrap(), 1.0);
        assert_eq!(evaluate_expression("8 mod 4").unwrap(), 0.0);

        // Test percent
        assert_eq!(evaluate_expression("50 percent 1").unwrap(), 0.5);
        assert_eq!(evaluate_expression("25 percent 1").unwrap(), 0.25);

        // Test degree trigonometric functions
        assert!((evaluate_expression("sind(90)").unwrap() - 1.0).abs() < 1e-10);
        assert!((evaluate_expression("cosd(0)").unwrap() - 1.0).abs() < 1e-10);
        assert!((evaluate_expression("tand(45)").unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_error_cases() {
        // Test factorial with negative number
        assert!(evaluate_expression("fact(-1)").is_err());

        // Test factorial with decimal
        assert!(evaluate_expression("fact(3.5)").is_err());

        // Test factorial with large number
        assert!(evaluate_expression("fact(200)").is_err());

        // Test inverse with zero
        assert!(evaluate_expression("inv(0)").is_err());

        // Test modulo with zero
        assert!(evaluate_expression("5 mod 0").is_err());
    }
}
