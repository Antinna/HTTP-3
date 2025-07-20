use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use tracing::{debug, info};

use crate::error::AppResult;

/// Currency configuration loaded from environment variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyConfig {
    pub code: String,
    pub symbol: String,
    pub name: String,
    pub decimal_places: u8,
    pub thousands_separator: String,
    pub decimal_separator: String,
    pub symbol_before: bool,
    pub rates: HashMap<String, Decimal>,
}

impl Default for CurrencyConfig {
    fn default() -> Self {
        Self {
            code: "INR".to_string(),
            symbol: "₹".to_string(),
            name: "Rupees".to_string(),
            decimal_places: 2,
            thousands_separator: ",".to_string(),
            decimal_separator: ".".to_string(),
            symbol_before: true,
            rates: HashMap::new(),
        }
    }
}

/// Currency helper service for formatting, conversion, and localization
pub struct CurrencyHelper {
    config: CurrencyConfig,
}

impl CurrencyHelper {
    /// Create new currency helper with provided configuration
    pub fn new(config: CurrencyConfig) -> Self {
        Self { config }
    }

    /// Create currency helper from environment variables
    pub fn from_env() -> AppResult<Self> {
        info!("Loading currency configuration from environment");
        
        let config = CurrencyConfig {
            code: env::var("APP_CURRENCY").unwrap_or_else(|_| "INR".to_string()),
            symbol: env::var("APP_CURRENCY_SYMBOL").unwrap_or_else(|_| "₹".to_string()),
            name: env::var("APP_CURRENCY_NAME").unwrap_or_else(|_| "Rupees".to_string()),
            decimal_places: env::var("APP_CURRENCY_DECIMAL_PLACES")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .unwrap_or(2),
            thousands_separator: env::var("APP_CURRENCY_THOUSANDS_SEP")
                .unwrap_or_else(|_| ",".to_string()),
            decimal_separator: env::var("APP_CURRENCY_DECIMAL_SEP")
                .unwrap_or_else(|_| ".".to_string()),
            symbol_before: env::var("APP_CURRENCY_SYMBOL_BEFORE")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            rates: Self::load_exchange_rates()?,
        };
        
        debug!("Currency configuration loaded: {} ({})", config.name, config.symbol);
        Ok(Self::new(config))
    }

    /// Load exchange rates from environment variables
    fn load_exchange_rates() -> AppResult<HashMap<String, Decimal>> {
        let mut rates = HashMap::new();
        
        // Load exchange rates from environment variables
        // Format: EXCHANGE_RATE_USD=74.50, EXCHANGE_RATE_EUR=88.20, etc.
        for (key, value) in env::vars() {
            if key.starts_with("EXCHANGE_RATE_") {
                if let Some(currency_code) = key.strip_prefix("EXCHANGE_RATE_") {
                    if let Ok(rate) = value.parse::<Decimal>() {
                        rates.insert(currency_code.to_string(), rate);
                        debug!("Loaded exchange rate: {} = {}", currency_code, rate);
                    }
                }
            }
        }
        
        info!("Loaded {} exchange rates from environment", rates.len());
        Ok(rates)
    }

    /// Format amount with currency symbol
    pub fn format(&self, amount: Decimal, currency_code: Option<&str>) -> String {
        let _currency_code = currency_code.unwrap_or(&self.config.code);
        let formatted_number = self.format_number(amount);
        
        if self.config.symbol_before {
            format!("{}{}", self.config.symbol, formatted_number)
        } else {
            format!("{}{}", formatted_number, self.config.symbol)
        }
    }

    /// Format amount without currency symbol
    pub fn format_number(&self, amount: Decimal) -> String {
        // Convert to string with specified decimal places
        let amount_str = format!("{:.1$}", amount, self.config.decimal_places as usize);
        
        // Split into integer and decimal parts
        let parts: Vec<&str> = amount_str.split('.').collect();
        let integer_part = parts[0];
        let decimal_part = if parts.len() > 1 { parts[1] } else { "" };
        
        // Add thousands separators to integer part
        let formatted_integer = self.add_thousands_separator(integer_part);
        
        // Combine integer and decimal parts
        if self.config.decimal_places > 0 && !decimal_part.is_empty() {
            format!("{}{}{}", formatted_integer, self.config.decimal_separator, decimal_part)
        } else {
            formatted_integer
        }
    }

    /// Add thousands separator to integer part
    fn add_thousands_separator(&self, integer_str: &str) -> String {
        let chars: Vec<char> = integer_str.chars().collect();
        let mut result = String::new();
        
        for (i, ch) in chars.iter().enumerate() {
            if i > 0 && (chars.len() - i) % 3 == 0 {
                result.push_str(&self.config.thousands_separator);
            }
            result.push(*ch);
        }
        
        result
    }

    /// Get currency symbol
    pub fn symbol(&self) -> &str {
        &self.config.symbol
    }

    /// Get currency code
    pub fn code(&self) -> &str {
        &self.config.code
    }

    /// Get currency name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Convert amount between currencies
    pub fn convert(&self, amount: Decimal, from: &str, to: &str) -> Result<Decimal, CurrencyError> {
        if from == to {
            return Ok(amount);
        }

        let from_rate = self.config.rates.get(from)
            .ok_or_else(|| CurrencyError::ExchangeRateNotFound(from.to_string()))?;
        
        let to_rate = self.config.rates.get(to)
            .ok_or_else(|| CurrencyError::ExchangeRateNotFound(to.to_string()))?;

        // Convert to base currency first, then to target currency
        let base_amount = amount / from_rate;
        Ok(base_amount * to_rate)
    }

    /// Get all supported currencies
    pub fn supported_currencies(&self) -> Vec<CurrencyInfo> {
        let mut currencies = vec![
            CurrencyInfo {
                code: self.config.code.clone(),
                symbol: self.config.symbol.clone(),
                name: self.config.name.clone(),
                is_default: true,
            }
        ];

        for (code, _rate) in &self.config.rates {
            if code != &self.config.code {
                currencies.push(CurrencyInfo {
                    code: code.clone(),
                    symbol: self.get_symbol_for_code(code),
                    name: self.get_name_for_code(code),
                    is_default: false,
                });
            }
        }

        currencies
    }

    /// Get symbol for currency code (could be extended with a lookup table)
    fn get_symbol_for_code(&self, code: &str) -> String {
        match code {
            "USD" => "$".to_string(),
            "EUR" => "€".to_string(),
            "GBP" => "£".to_string(),
            "JPY" => "¥".to_string(),
            "INR" => "₹".to_string(),
            "CNY" => "¥".to_string(),
            "AUD" => "A$".to_string(),
            "CAD" => "C$".to_string(),
            "CHF" => "CHF".to_string(),
            "SGD" => "S$".to_string(),
            _ => code.to_string(),
        }
    }

    /// Get name for currency code (could be extended with a lookup table)
    fn get_name_for_code(&self, code: &str) -> String {
        match code {
            "USD" => "US Dollar".to_string(),
            "EUR" => "Euro".to_string(),
            "GBP" => "British Pound".to_string(),
            "JPY" => "Japanese Yen".to_string(),
            "INR" => "Indian Rupee".to_string(),
            "CNY" => "Chinese Yuan".to_string(),
            "AUD" => "Australian Dollar".to_string(),
            "CAD" => "Canadian Dollar".to_string(),
            "CHF" => "Swiss Franc".to_string(),
            "SGD" => "Singapore Dollar".to_string(),
            _ => format!("{} Currency", code),
        }
    }

    /// Format price range (e.g., "₹100 - ₹500")
    pub fn format_range(&self, min_amount: Decimal, max_amount: Decimal) -> String {
        format!("{} - {}", self.format(min_amount, None), self.format(max_amount, None))
    }

    /// Parse formatted currency string back to Decimal
    pub fn parse(&self, formatted_amount: &str) -> Result<Decimal, CurrencyError> {
        let cleaned = formatted_amount
            .replace(&self.config.symbol, "")
            .replace(&self.config.thousands_separator, "")
            .replace(&self.config.decimal_separator, ".")
            .trim()
            .to_string();

        cleaned.parse::<Decimal>()
            .map_err(|e| CurrencyError::ParseError(e.to_string()))
    }

    /// Calculate percentage of amount
    pub fn calculate_percentage(&self, amount: Decimal, percentage: Decimal) -> Decimal {
        amount * percentage / Decimal::from(100)
    }

    /// Add percentage to amount
    pub fn add_percentage(&self, amount: Decimal, percentage: Decimal) -> Decimal {
        amount + self.calculate_percentage(amount, percentage)
    }

    /// Subtract percentage from amount
    pub fn subtract_percentage(&self, amount: Decimal, percentage: Decimal) -> Decimal {
        amount - self.calculate_percentage(amount, percentage)
    }

    /// Round amount to currency's decimal places
    pub fn round(&self, amount: Decimal) -> Decimal {
        let scale = self.config.decimal_places as u32;
        amount.round_dp(scale)
    }

    /// Check if amount is zero
    pub fn is_zero(&self, amount: Decimal) -> bool {
        amount.is_zero()
    }

    /// Check if amount is positive
    pub fn is_positive(&self, amount: Decimal) -> bool {
        amount.is_sign_positive() && !amount.is_zero()
    }

    /// Check if amount is negative
    pub fn is_negative(&self, amount: Decimal) -> bool {
        amount.is_sign_negative()
    }

    /// Get absolute value of amount
    pub fn abs(&self, amount: Decimal) -> Decimal {
        amount.abs()
    }

    /// Compare two amounts with currency precision
    pub fn equals(&self, amount1: Decimal, amount2: Decimal) -> bool {
        let rounded1 = self.round(amount1);
        let rounded2 = self.round(amount2);
        rounded1 == rounded2
    }
}

/// Currency information for API responses
#[derive(Debug, Serialize, Deserialize)]
pub struct CurrencyInfo {
    pub code: String,
    pub symbol: String,
    pub name: String,
    pub is_default: bool,
}

/// Currency-related errors
#[derive(Debug, thiserror::Error)]
pub enum CurrencyError {
    #[error("Exchange rate not found for currency: {0}")]
    ExchangeRateNotFound(String),
    
    #[error("Failed to parse currency amount: {0}")]
    ParseError(String),
    
    #[error("Invalid currency code: {0}")]
    InvalidCurrencyCode(String),
    
    #[error("Currency conversion failed: {0}")]
    ConversionError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_currency_formatting() {
        let helper = CurrencyHelper::from_env().unwrap();
        
        assert_eq!(helper.format(dec!(1234.56), None), "₹1,234.56");
        assert_eq!(helper.format_number(dec!(1000)), "1,000.00");
        assert_eq!(helper.symbol(), "₹");
        assert_eq!(helper.code(), "INR");
        assert_eq!(helper.name(), "Rupees");
    }

    #[test]
    fn test_thousands_separator() {
        let helper = CurrencyHelper::from_env().unwrap();
        
        assert_eq!(helper.format_number(dec!(1000)), "1,000.00");
        assert_eq!(helper.format_number(dec!(1000000)), "1,000,000.00");
        assert_eq!(helper.format_number(dec!(1234567.89)), "1,234,567.89");
    }

    #[test]
    fn test_currency_parsing() {
        let helper = CurrencyHelper::from_env().unwrap();
        
        assert_eq!(helper.parse("₹1,234.56").unwrap(), dec!(1234.56));
        assert_eq!(helper.parse("1,000.00").unwrap(), dec!(1000.00));
        assert_eq!(helper.parse("₹ 500").unwrap(), dec!(500));
    }

    #[test]
    fn test_price_range_formatting() {
        let helper = CurrencyHelper::from_env().unwrap();
        
        assert_eq!(helper.format_range(dec!(100), dec!(500)), "₹100.00 - ₹500.00");
    }

    #[test]
    fn test_percentage_calculations() {
        let helper = CurrencyHelper::from_env().unwrap();
        
        assert_eq!(helper.calculate_percentage(dec!(1000), dec!(10)), dec!(100));
        assert_eq!(helper.add_percentage(dec!(1000), dec!(10)), dec!(1100));
        assert_eq!(helper.subtract_percentage(dec!(1000), dec!(10)), dec!(900));
    }

    #[test]
    fn test_currency_rounding() {
        let helper = CurrencyHelper::from_env().unwrap();
        
        assert_eq!(helper.round(dec!(123.456)), dec!(123.46));
        assert_eq!(helper.round(dec!(123.454)), dec!(123.45));
    }

    #[test]
    fn test_currency_comparisons() {
        let helper = CurrencyHelper::from_env().unwrap();
        
        assert!(helper.is_positive(dec!(100)));
        assert!(helper.is_negative(dec!(-100)));
        assert!(helper.is_zero(dec!(0)));
        assert_eq!(helper.abs(dec!(-100)), dec!(100));
        assert!(helper.equals(dec!(123.45), dec!(123.45))); // Should be equal
        assert!(helper.equals(dec!(123.456), dec!(123.456))); // Should be equal (same value)
        assert!(!helper.equals(dec!(123.45), dec!(123.46))); // Should not be equal
    }

    #[test]
    fn test_custom_currency_config() {
        let config = CurrencyConfig {
            code: "USD".to_string(),
            symbol: "$".to_string(),
            name: "US Dollar".to_string(),
            decimal_places: 2,
            thousands_separator: ",".to_string(),
            decimal_separator: ".".to_string(),
            symbol_before: true,
            rates: HashMap::new(),
        };
        
        let helper = CurrencyHelper::new(config);
        
        assert_eq!(helper.format(dec!(1234.56), None), "$1,234.56");
        assert_eq!(helper.code(), "USD");
        assert_eq!(helper.name(), "US Dollar");
    }

    #[test]
    fn test_currency_conversion() {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), dec!(74.50)); // 1 USD = 74.50 INR
        rates.insert("EUR".to_string(), dec!(88.20)); // 1 EUR = 88.20 INR
        
        let config = CurrencyConfig {
            rates,
            ..Default::default()
        };
        
        let helper = CurrencyHelper::new(config);
        
        // Convert 100 USD to EUR
        let result = helper.convert(dec!(100), "USD", "EUR").unwrap();
        let expected = dec!(100) / dec!(74.50) * dec!(88.20); // Convert via base currency
        assert_eq!(result.round_dp(2), expected.round_dp(2));
    }

    #[test]
    fn test_supported_currencies() {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), dec!(74.50));
        rates.insert("EUR".to_string(), dec!(88.20));
        
        let config = CurrencyConfig {
            rates,
            ..Default::default()
        };
        
        let helper = CurrencyHelper::new(config);
        let currencies = helper.supported_currencies();
        
        assert_eq!(currencies.len(), 3); // INR (default) + USD + EUR
        assert!(currencies.iter().any(|c| c.code == "INR" && c.is_default));
        assert!(currencies.iter().any(|c| c.code == "USD" && !c.is_default));
        assert!(currencies.iter().any(|c| c.code == "EUR" && !c.is_default));
    }
}